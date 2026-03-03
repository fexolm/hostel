use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

use kernel::memory::alloc::{kmalloc::KernelAllocator, palloc::PageAllocator};
use kernel_benches::{ALLOC_SIZE, VecDirectMap, prepare_alloc_workload, run_alloc_free_scenario};

pub fn bench_kmalloc_alloc_free(c: &mut Criterion) {
    let object_count = 2048;
    let (first_cycle_sizes, second_cycle_sizes, memory_limit) =
        prepare_alloc_workload(object_count);

    let page_alloc = PageAllocator::with_memory_limit(2 * memory_limit);
    let dm = VecDirectMap::new(ALLOC_SIZE);
    let kmalloc = KernelAllocator::new(&dm, &page_alloc);

    let mut group = c.benchmark_group("kmalloc_alloc_free");
    group.bench_function("kmalloc_alloc_free", |b| {
        b.iter(|| {
            run_alloc_free_scenario(&kmalloc, &first_cycle_sizes, &second_cycle_sizes).unwrap();
            black_box(&kmalloc);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_kmalloc_alloc_free);
criterion_main!(benches);
