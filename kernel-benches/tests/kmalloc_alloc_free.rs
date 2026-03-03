use kernel::memory::alloc::{kmalloc::KernelAllocator, palloc::PageAllocator};
use kernel_benches::{ALLOC_SIZE, VecDirectMap, prepare_alloc_workload, run_alloc_free_scenario};

#[test]
fn kmalloc_alloc_free_matches_benchmark_flow() {
    let object_count = 2048;
    let (first_cycle_sizes, second_cycle_sizes, memory_limit) =
        prepare_alloc_workload(object_count);

    let page_alloc = PageAllocator::with_memory_limit(2 * memory_limit);
    let dm = VecDirectMap::new(ALLOC_SIZE);
    let kmalloc = KernelAllocator::new(&dm, &page_alloc);

    run_alloc_free_scenario(&kmalloc, &first_cycle_sizes, &second_cycle_sizes).unwrap();

    let final_palloc = page_alloc.get_stats();
    let overhead_pct =
        ((final_palloc.peak_memory_usage as f64 - memory_limit as f64) / memory_limit as f64)
            * 100.0;
    println!(
        "palloc used: {} memory_limit: {} kmalloc overhead: {:.2}%",
        final_palloc.peak_memory_usage, memory_limit, overhead_pct
    );
}
