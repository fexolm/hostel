use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

use kernel::memory::alloc::{palloc::PageAllocator, kmalloc::KernelAllocator};

pub fn bench_kmalloc_small(c: &mut Criterion) {
    let page_alloc = PageAllocator::new();
    let kmalloc = KernelAllocator::new(&page_alloc);
    let mut group = c.benchmark_group("kmalloc_small");
    group.sample_size(10);
    group.warm_up_time(std::time::Duration::from_millis(10));

    for &size in [1 << 22, 1 << 23].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let ptr = kmalloc.alloc(black_box(size)).unwrap();
                kmalloc.free(ptr).unwrap();
            });
        });
    }

    group.finish();
}


criterion_group!(
    benches,
    bench_kmalloc_small
);

criterion_main!(benches);
