use crate::memory::{
    address::{PhysicalAddr, VirtualAddr},
    alloc::palloc::{palloc, pfree},
    constants::{DIRECT_MAP_OFFSET, PAGE_SIZE},
};

const MIN_SHIFT: u32 = 10; // 1 KiB
const MAX_SHIFT: u32 = 24; // 16 MiB
const MIN_ALLOC_SIZE: usize = 1usize << MIN_SHIFT;
const MAX_ALLOC_SIZE: usize = 1usize << MAX_SHIFT;
const SMALL_CLASS_COUNT: usize = (21 - MIN_SHIFT + 1) as usize; // 1 KiB .. 2 MiB
const MAX_SLABS_PER_CLASS: usize = 128;
const MAX_LARGE_ALLOCS: usize = 256;
const FREE_LIST_END: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct SmallSlab {
    in_use: bool,
    base: PhysicalAddr,
    block_size: u32,
    capacity: u32,
    free_count: u32,
    free_head: u32,
}

impl SmallSlab {
    const fn empty() -> Self {
        Self {
            in_use: false,
            base: PhysicalAddr::new(0),
            block_size: 0,
            capacity: 0,
            free_count: 0,
            free_head: FREE_LIST_END,
        }
    }
}

#[derive(Clone, Copy)]
struct SmallClass {
    block_size: u32,
    slabs: [SmallSlab; MAX_SLABS_PER_CLASS],
}

impl SmallClass {
    const fn new(block_size: u32) -> Self {
        Self {
            block_size,
            slabs: [SmallSlab::empty(); MAX_SLABS_PER_CLASS],
        }
    }
}

#[derive(Clone, Copy)]
struct LargeAlloc {
    in_use: bool,
    base: PhysicalAddr,
    pages: u64,
}

impl LargeAlloc {
    const fn empty() -> Self {
        Self {
            in_use: false,
            base: PhysicalAddr::new(0),
            pages: 0,
        }
    }
}

struct KmallocAllocator {
    small: [SmallClass; SMALL_CLASS_COUNT],
    large: [LargeAlloc; MAX_LARGE_ALLOCS],
}

impl KmallocAllocator {
    const fn new() -> Self {
        Self {
            small: build_small_classes(),
            large: [LargeAlloc::empty(); MAX_LARGE_ALLOCS],
        }
    }

    fn alloc(&mut self, size: usize) -> VirtualAddr {
        let class_size = size_to_class(size) as u64;

        if class_size <= PAGE_SIZE {
            self.alloc_small(class_size as u32)
        } else {
            self.alloc_large(class_size)
        }
    }

    fn free(&mut self, ptr: VirtualAddr) {
        let phys = ptr
            .to_physical()
            .expect("kfree expects pointer from direct map");

        if self.free_small(phys) {
            return;
        }

        self.free_large(phys);
    }

    fn alloc_small(&mut self, block_size: u32) -> VirtualAddr {
        let class_idx = (block_size.trailing_zeros() - MIN_SHIFT) as usize;
        let class = &mut self.small[class_idx];

        for slab in &mut class.slabs {
            if slab.in_use && slab.free_count > 0 {
                return alloc_from_small_slab(slab);
            }
        }

        for slab in &mut class.slabs {
            if !slab.in_use {
                init_small_slab(slab, class.block_size);
                return alloc_from_small_slab(slab);
            }
        }

        panic!("kmalloc: too many slabs for class {}", class.block_size);
    }

    fn free_small(&mut self, addr: PhysicalAddr) -> bool {
        let p = addr.as_u64();

        for class in &mut self.small {
            for slab in &mut class.slabs {
                if !slab.in_use {
                    continue;
                }

                let start = slab.base.as_u64();
                let end = start + PAGE_SIZE;
                if p < start || p >= end {
                    continue;
                }

                let block_size = slab.block_size as u64;
                let offset = p - start;
                assert!(
                    offset % block_size == 0,
                    "kfree: pointer does not match slab alignment"
                );

                let idx = (offset / block_size) as u32;
                unsafe {
                    *small_slab_link_ptr(slab, idx) = slab.free_head;
                }
                slab.free_head = idx;
                slab.free_count += 1;

                if slab.free_count == slab.capacity {
                    let base = slab.base;
                    *slab = SmallSlab::empty();
                    pfree(base, 1);
                }

                return true;
            }
        }

        false
    }

    fn alloc_large(&mut self, class_size: u64) -> VirtualAddr {
        let pages = class_size / PAGE_SIZE;
        let base = palloc(pages);

        for slot in &mut self.large {
            if !slot.in_use {
                *slot = LargeAlloc {
                    in_use: true,
                    base,
                    pages,
                };
                return base.to_virtual().expect("valid direct map conversion");
            }
        }

        pfree(base, pages);
        panic!("kmalloc: too many active large allocations");
    }

    fn free_large(&mut self, addr: PhysicalAddr) {
        for slot in &mut self.large {
            if slot.in_use && slot.base == addr {
                pfree(slot.base, slot.pages);
                *slot = LargeAlloc::empty();
                return;
            }
        }

        panic!("kfree: unknown allocation {}", addr);
    }
}

const fn build_small_classes() -> [SmallClass; SMALL_CLASS_COUNT] {
    let mut classes = [SmallClass::new(0); SMALL_CLASS_COUNT];
    let mut i = 0;
    while i < SMALL_CLASS_COUNT {
        classes[i] = SmallClass::new(1u32 << (MIN_SHIFT + i as u32));
        i += 1;
    }
    classes
}

fn size_to_class(size: usize) -> usize {
    let requested = if size == 0 { MIN_ALLOC_SIZE } else { size };
    assert!(
        requested <= MAX_ALLOC_SIZE,
        "kmalloc supports up to 16 MiB allocations"
    );

    let mut class_size = MIN_ALLOC_SIZE;
    while class_size < requested {
        class_size <<= 1;
    }
    class_size
}

fn init_small_slab(slab: &mut SmallSlab, block_size: u32) {
    let base = palloc(1);
    let capacity = (PAGE_SIZE as u32) / block_size;
    assert!(capacity > 0, "invalid slab capacity");

    *slab = SmallSlab {
        in_use: true,
        base,
        block_size,
        capacity,
        free_count: capacity,
        free_head: 0,
    };

    let mut i = 0;
    while i < capacity {
        let next = if i + 1 < capacity {
            i + 1
        } else {
            FREE_LIST_END
        };
        unsafe {
            *small_slab_link_ptr(slab, i) = next;
        }
        i += 1;
    }
}

fn alloc_from_small_slab(slab: &mut SmallSlab) -> VirtualAddr {
    let idx = slab.free_head;
    assert!(idx != FREE_LIST_END, "slab is empty");

    let next = unsafe { *small_slab_link_ptr(slab, idx) };
    slab.free_head = next;
    slab.free_count -= 1;

    let offset = idx as u64 * slab.block_size as u64;
    slab.base
        .add(offset)
        .to_virtual()
        .expect("invalid direct map conversion")
}

unsafe fn small_slab_link_ptr(slab: &SmallSlab, idx: u32) -> *mut u32 {
    let addr = slab.base.as_u64() + idx as u64 * slab.block_size as u64;
    VirtualAddr::new(DIRECT_MAP_OFFSET.as_u64() + addr).as_ptr::<u32>()
}

static KMALLOC: spin::Mutex<KmallocAllocator> = spin::Mutex::new(KmallocAllocator::new());

pub fn kmalloc(size: usize) -> VirtualAddr {
    KMALLOC.lock().alloc(size)
}

pub fn kfree(ptr: VirtualAddr) {
    KMALLOC.lock().free(ptr);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::alloc::ALLOC_TEST_LOCK;

    #[test]
    fn class_rounding_works() {
        let _guard = ALLOC_TEST_LOCK.lock();
        assert_eq!(size_to_class(0), 1024);
        assert_eq!(size_to_class(1024), 1024);
        assert_eq!(size_to_class(1025), 2048);
        assert_eq!(size_to_class((1 << 22) + 1), 1 << 23);
    }

    #[test]
    fn class_boundaries_are_powers_of_two() {
        let _guard = ALLOC_TEST_LOCK.lock();
        for shift in MIN_SHIFT..=MAX_SHIFT {
            let class = 1usize << shift;
            assert_eq!(size_to_class(class - 1), class);
            assert_eq!(size_to_class(class), class);
            if shift < MAX_SHIFT {
                assert_eq!(size_to_class(class + 1), class << 1);
            }
        }
    }

    #[test]
    #[should_panic(expected = "kmalloc supports up to 16 MiB allocations")]
    fn class_rounding_panics_above_limit() {
        let _guard = ALLOC_TEST_LOCK.lock();
        let _ = size_to_class(MAX_ALLOC_SIZE + 1);
    }

    #[test]
    fn kmalloc_large_is_contiguous_and_reused() {
        let _guard = ALLOC_TEST_LOCK.lock();
        let mut alloc = KmallocAllocator::new();

        let a = alloc.alloc((1 << 22) + 1); // rounds to 8 MiB
        let b = alloc.alloc(1 << 22); // 4 MiB

        assert_eq!(
            a.to_physical()
                .expect("kmalloc should return direct-map addresses")
                .as_u64()
                % PAGE_SIZE,
            0
        );
        assert_eq!(
            b.to_physical()
                .expect("kmalloc should return direct-map addresses")
                .as_u64()
                % PAGE_SIZE,
            0
        );

        alloc.free(a);
        let c = alloc.alloc(1 << 23);
        assert_eq!(c.as_u64(), a.as_u64());
    }

    #[test]
    fn kmalloc_large_allocations_do_not_overlap() {
        let _guard = ALLOC_TEST_LOCK.lock();
        let mut alloc = KmallocAllocator::new();

        let a = alloc.alloc(1 << 22); // 4 MiB
        let b = alloc.alloc(1 << 22); // 4 MiB

        let a_phys = a
            .to_physical()
            .expect("kmalloc should return direct-map addresses")
            .as_u64();
        let b_phys = b
            .to_physical()
            .expect("kmalloc should return direct-map addresses")
            .as_u64();

        assert_ne!(a_phys, b_phys);
        let diff = a_phys.abs_diff(b_phys);
        assert!(diff >= (1 << 22) as u64);
    }

    #[test]
    fn kmalloc_large_free_and_realloc_same_class_reuses_address() {
        let _guard = ALLOC_TEST_LOCK.lock();
        let mut alloc = KmallocAllocator::new();

        let a = alloc.alloc(1 << 24); // 16 MiB
        let b = alloc.alloc(1 << 24); // 16 MiB
        assert_ne!(a.as_u64(), b.as_u64());

        alloc.free(b);
        let c = alloc.alloc(1 << 24);
        assert_eq!(c.as_u64(), b.as_u64());
    }
}
