use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

use kernel::memory::alloc::{palloc::PageAllocator, kmalloc::KernelAllocator};
use kernel::memory::address::{DirectMap, PhysicalAddr, VirtualAddr};
use kernel::memory::errors::{MemoryError, Result};

pub struct VecDirectMap {
    mem: Vec<u8>,
}

impl VecDirectMap {
    pub fn new(size: usize) -> Self {
        Self { mem: vec![0u8; size] }
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.mem.as_ptr()
    }
}

impl DirectMap for VecDirectMap {
    fn p2v(&self, phys: PhysicalAddr) -> VirtualAddr {
        VirtualAddr::new(self.as_ptr() as usize + phys.as_usize())
    }

    fn v2p(&self, vaddr: VirtualAddr) -> Result<PhysicalAddr> {
        let virt = vaddr.as_usize();
        if virt < self.as_ptr() as usize {
            return Err(MemoryError::VirtualToPhysical { addr: vaddr.as_usize() });
        }
        Ok(PhysicalAddr::new(virt - self.as_ptr() as usize))
    }
    
}

pub const ALLOC_SIZE: usize = 1 << 32; // 4 GiB

pub fn bench_kmalloc_small(c: &mut Criterion) {
    let page_alloc = PageAllocator::new();
    let dm = VecDirectMap::new(ALLOC_SIZE);
    let kmalloc = KernelAllocator::new(&dm, &page_alloc);
    let mut group = c.benchmark_group("kmalloc_small");

    for &size in [8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096].iter() {
        let object_count = 2048;
        let mut ptr_vec: Vec<u32> = vec![0; object_count];
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                for i in 0..object_count {
                    ptr_vec[i] = kmalloc.alloc(black_box(size)).unwrap().as_usize() as u32;
                }
                for &ptr in ptr_vec.iter() {
                    kmalloc.free(PhysicalAddr::new(ptr as usize)).unwrap();
                }
            });
        });
    }

    group.finish();
}

pub fn bench_kmalloc_large(c: &mut Criterion) {
    let page_alloc = PageAllocator::new();
    let dm = VecDirectMap::new(ALLOC_SIZE);
    let kmalloc = KernelAllocator::new(&dm, &page_alloc);
    let mut group = c.benchmark_group("kmalloc_large");

    for &size in [16 * 1024, 32 * 1024, 64 * 1024, 128 * 1024, 256 * 1024].iter() { // 16 KiB to 256 KiB
        let object_count = 2048;
        let mut ptr_vec: Vec<u32> = vec![0; object_count];
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                for i in 0..object_count {
                    ptr_vec[i] = kmalloc.alloc(black_box(size)).unwrap().as_usize() as u32;
                }
                for &ptr in ptr_vec.iter() {
                    kmalloc.free(PhysicalAddr::new(ptr as usize)).unwrap();
                }
            });
        });
    }

    group.finish();
}

#[derive(Debug, Clone, Copy)]
struct XorShift32 {
    a: u32,
}

impl XorShift32 {
    fn new(seed: u32) -> Self {
        assert!(seed != 0, "Seed must be non-zero!");
        XorShift32 { a: seed }
    }

    fn next(&mut self) -> u32 {
        let mut x = self.a;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.a = x;
        x
    }
}

pub fn bench_kmalloc_mixed(c: &mut Criterion) {
    let mut rng = XorShift32::new(42);
    let page_alloc = PageAllocator::new();
    let dm = VecDirectMap::new(ALLOC_SIZE);
    let kmalloc = KernelAllocator::new(&dm, &page_alloc);
    let mut group = c.benchmark_group("kmalloc_mixed");
    let object_count = 2048;
    let mut ptr_vec: Vec<u32> = vec![0; object_count];
    group.bench_function("kmalloc_mixed", |b| {
        b.iter(|| {
            for i in 0..object_count {
                let size = rng.next() as usize;
                ptr_vec[i] = kmalloc.alloc(black_box(size)).unwrap().as_usize() as u32;
            }
            for &ptr in ptr_vec.iter() {
                kmalloc.free(PhysicalAddr::new(ptr as usize)).unwrap();
            }
        });
    });

    group.finish();
}

#[inline(always)]
fn pseudo_size(i: u64) -> u16 {
    let x = i.wrapping_mul(0x9E3779B97F4A7C15);
    let lz = x.leading_zeros() as u16; // 0..64

    1u16 << (lz % 16) // 1..32768
}

pub fn bench_kmalloc_lifecycle(c: &mut Criterion) {
    let page_alloc = PageAllocator::new();
    let dm = VecDirectMap::new(ALLOC_SIZE);
    let kmalloc = KernelAllocator::new(&dm, &page_alloc);
    let mut group = c.benchmark_group("kmalloc_lifecycle");
    let object_count = 2048;
    let mut ptr_vec: Vec<u32> = vec![0; object_count];
    group.bench_function("kmalloc_lifecycle", |b| {
        b.iter(|| {
            for i in 0..(object_count/2) {
                ptr_vec[i] = kmalloc.alloc(black_box(pseudo_size(i as u64)) as usize).unwrap().as_usize() as u32;
            }
            for i in 0..(object_count/4) {
                kmalloc.free(PhysicalAddr::new(ptr_vec[i] as usize)).unwrap();
            }
            for i in 0..(object_count/4) {
                ptr_vec[i] = kmalloc.alloc(black_box(pseudo_size(i as u64)) as usize).unwrap().as_usize() as u32;
            }
            for i in (object_count/2)..object_count {
                ptr_vec[i] = kmalloc.alloc(black_box(pseudo_size(i as u64)) as usize).unwrap().as_usize() as u32;
            }
            for i in 0..object_count {
                kmalloc.free(PhysicalAddr::new(ptr_vec[i] as usize)).unwrap();
            }
        });
    });

    group.finish();
}

pub fn bench_kmalloc_alloc_free(c: &mut Criterion) {
    let page_alloc = PageAllocator::new();
    let dm = VecDirectMap::new(ALLOC_SIZE);
    let kmalloc = KernelAllocator::new(&dm, &page_alloc);
    let mut group = c.benchmark_group("kmalloc_alloc_free");
    let object_count = 2048;
    let mut ptr_vec: Vec<u32> = vec![0; object_count];
    group.bench_function("kmalloc_alloc_free", |b| {
        b.iter(|| {
            for i in 0..object_count {
                ptr_vec[i] = kmalloc.alloc(black_box(pseudo_size(i as u64)) as usize).unwrap().as_usize() as u32;
            }
            for i in 0..object_count {
                kmalloc.free(PhysicalAddr::new(ptr_vec[i] as usize)).unwrap();
                ptr_vec[i] = kmalloc.alloc(black_box(pseudo_size(i as u64)) as usize).unwrap().as_usize() as u32;
            }
            for i in 0..object_count {
                kmalloc.free(PhysicalAddr::new(ptr_vec[i] as usize)).unwrap();
            }
        });
    });

    group.finish();
}


criterion_group!(
    benches,
    bench_kmalloc_small,
    bench_kmalloc_large,
    bench_kmalloc_mixed,
    bench_kmalloc_lifecycle,
    bench_kmalloc_alloc_free
);

criterion_main!(benches);
