use crate::{constants::PAGE_SIZE, address::PhysicalAddr};

const BITMAP_SIZE: usize = 8 * 1024; // can address up to 1tb of 2mb pages

#[repr(align(4096))]
#[repr(C)]
struct PageAllocator {
    bitmap: [u64; BITMAP_SIZE],
}

impl PageAllocator {
    const fn new() -> Self {
        let mut bitmap = [0; BITMAP_SIZE];
        bitmap[0] = 1; // occupy first page
        Self { bitmap }
    }

    fn alloc(&mut self) -> PhysicalAddr {
        for (i, word) in self.bitmap.iter_mut().enumerate() {
            let free_bits = !*word;

            if free_bits != 0 {
                let j = free_bits.trailing_zeros() as usize;

                *word |= 1 << j;

                let global_idx = (i * 64 + j) as u64;
                return PhysicalAddr(global_idx * PAGE_SIZE);
            }
        }
        panic!("Out of memory");
    }

    fn free(&mut self, addr: PhysicalAddr) {
        let page_index = addr.0 / PAGE_SIZE;
        let word_index = page_index / 64;
        let bit_index = page_index % 64;
        self.bitmap[word_index as usize] &= !(1 << bit_index);
    }
}

static PAGE_ALLOCATOR: spin::Mutex<PageAllocator> = spin::Mutex::new(PageAllocator::new());

pub fn palloc() -> PhysicalAddr {
    PAGE_ALLOCATOR.lock().alloc()
}

pub fn pfree(addr: PhysicalAddr) {
    PAGE_ALLOCATOR.lock().free(addr);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_allocator() {
        let mut allocator = PageAllocator::new();
        let addr1 = allocator.alloc();
        let addr2 = allocator.alloc();
        assert_eq!(addr1, PhysicalAddr(PAGE_SIZE));
        assert_eq!(addr2, PhysicalAddr(2 * PAGE_SIZE));
        allocator.free(addr1);
        let addr3 = allocator.alloc();
        assert_eq!(addr3, PhysicalAddr(PAGE_SIZE)); // should reuse the freed page
    }
}
