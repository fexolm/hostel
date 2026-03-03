use rand::SeedableRng;
use rand::rngs::StdRng;
use rand_distr::{Distribution, LogNormal};

use kernel::memory::address::{DirectMap, PhysicalAddr, VirtualAddr};
use kernel::memory::alloc::kmalloc::KernelAllocator;
use kernel::memory::errors::{MemoryError, Result};

pub const ALLOC_SIZE: usize = 1 << 32; // 4 GiB
pub const LOGNORMAL_MU: f64 = 9.8;
pub const LOGNORMAL_SIGMA: f64 = 1.8;
pub const RNG_SEED: u64 = 15;
const MAX_ALLOC_SIZE: usize = 1 << 24; // 16 MiB, matches kernel kmalloc limit

pub struct VecDirectMap {
    mem: Vec<u8>,
}

impl VecDirectMap {
    pub fn new(size: usize) -> Self {
        Self {
            mem: vec![0u8; size],
        }
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
            return Err(MemoryError::VirtualToPhysical {
                addr: vaddr.as_usize(),
            });
        }
        Ok(PhysicalAddr::new(virt - self.as_ptr() as usize))
    }
}

#[inline(always)]
fn sample_alloc_size(rng: &mut StdRng, log_normal: &LogNormal<f64>) -> usize {
    let size = log_normal.sample(rng).round();
    size.clamp(1.0, MAX_ALLOC_SIZE as f64) as usize
}

pub fn build_alloc_sizes(
    object_count: usize,
    rng: &mut StdRng,
    log_normal: &LogNormal<f64>,
) -> Vec<usize> {
    (0..object_count)
        .map(|_| sample_alloc_size(rng, log_normal))
        .collect()
}

pub fn calc_peak_memory_usage(first_cycle_sizes: &[usize], second_cycle_sizes: &[usize]) -> usize {
    assert_eq!(
        first_cycle_sizes.len(),
        second_cycle_sizes.len(),
        "cycle size arrays must have same length",
    );

    let mut peak_live_bytes = 0usize;
    let mut current_live_bytes = 0usize;

    for &size in first_cycle_sizes {
        current_live_bytes += size;
    }

    for i in 0..first_cycle_sizes.len() {
        peak_live_bytes = peak_live_bytes.max(current_live_bytes);

        current_live_bytes -= first_cycle_sizes[i];
        current_live_bytes += second_cycle_sizes[i];
    }

    peak_live_bytes = peak_live_bytes.max(current_live_bytes);

    peak_live_bytes
}

pub fn prepare_alloc_workload(object_count: usize) -> (Vec<usize>, Vec<usize>, usize) {
    let mut rng: StdRng = StdRng::seed_from_u64(RNG_SEED);
    let log_normal = LogNormal::new(LOGNORMAL_MU, LOGNORMAL_SIGMA)
        .expect("log-normal distribution params must be valid");
    let first_cycle_sizes = build_alloc_sizes(object_count, &mut rng, &log_normal);
    let second_cycle_sizes = build_alloc_sizes(object_count, &mut rng, &log_normal);
    let memory_usage = calc_peak_memory_usage(&first_cycle_sizes, &second_cycle_sizes);

    (first_cycle_sizes, second_cycle_sizes, memory_usage)
}

pub fn run_alloc_free_scenario<DM: DirectMap>(
    kmalloc: &KernelAllocator<'_, DM>,
    first_cycle_sizes: &[usize],
    second_cycle_sizes: &[usize],
) -> Result<()> {
    assert_eq!(
        first_cycle_sizes.len(),
        second_cycle_sizes.len(),
        "cycle size arrays must have same length",
    );

    let object_count = first_cycle_sizes.len();
    let mut ptr_vec: Vec<(usize, usize)> = vec![(0, 0); object_count];

    for i in 0..object_count {
        let size = first_cycle_sizes[i];
        ptr_vec[i] = (kmalloc.alloc(size)?.as_usize(), size);
    }

    for i in 0..object_count {
        let (ptr, size) = ptr_vec[i];
        kmalloc.free(PhysicalAddr::new(ptr), size)?;

        let next_size = second_cycle_sizes[i];
        ptr_vec[i] = (kmalloc.alloc(next_size)?.as_usize(), next_size);
    }

    for i in 0..object_count {
        let (ptr, size) = ptr_vec[i];
        kmalloc.free(PhysicalAddr::new(ptr), size)?;
    }

    Ok(())
}
