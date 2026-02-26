use crate::{
    boot::RunFlags,
    memory::address::{PhysicalAddr, VirtualAddr},
};

pub const PAGE_SIZE: usize = 2 << 20;
pub const MAX_PHYSICAL_ADDR: usize = 0x0000_00FF_FFFF_FFFF;

pub const PAGE_TABLE_ENTRIES: usize = 512;
pub const PAGE_TABLE_SIZE: usize = 8 * PAGE_TABLE_ENTRIES;

// offset of kernel code in virtual address space; the kernel is linked to run at this address
pub const KERNEL_CODE_VIRT: VirtualAddr = VirtualAddr::new(0xFFFF_FFFF_8000_0000);

// direct map
pub const DIRECT_MAP_OFFSET: VirtualAddr = VirtualAddr::new(0xFFFF_8880_0000_0000);

pub const DIRECT_MAP_PML4: PhysicalAddr = PhysicalAddr::new(0x0);

// the PML4 entry index for the direct map region; this is used to set up the initial page tables
pub const DIRECT_MAP_PML4_OFFSET: usize = DIRECT_MAP_OFFSET.pml4_index();
pub const DIRECT_MAP_PML4_ENTRIES_COUNT: usize = DIRECT_MAP_PDPT_COUNT.div_ceil(PAGE_TABLE_ENTRIES); // number of PML4 entries needed to cover the direct map region

pub const DIRECT_MAP_PDPT: PhysicalAddr = DIRECT_MAP_PML4.add(PAGE_TABLE_SIZE);
pub const DIRECT_MAP_PDPT_COUNT: usize =
    (MAX_PHYSICAL_ADDR + 1).div_ceil(PAGE_SIZE * PAGE_TABLE_ENTRIES * PAGE_TABLE_ENTRIES);
pub const DIRECT_MAP_PD: PhysicalAddr =
    DIRECT_MAP_PDPT.add(DIRECT_MAP_PDPT_COUNT * PAGE_TABLE_SIZE);
pub const DIRECT_MAP_PD_COUNT: usize =
    (MAX_PHYSICAL_ADDR + 1).div_ceil(PAGE_SIZE * PAGE_TABLE_ENTRIES);

// pdpd and pd for the kernel code (we need to reserver 2gb of virtual address space for kernel code, for code-model=kernel)
pub const KERNEL_CODE_PDPD: PhysicalAddr = DIRECT_MAP_PD.add(DIRECT_MAP_PD_COUNT * PAGE_TABLE_SIZE);
pub const KERNEL_CODE_PD: PhysicalAddr = KERNEL_CODE_PDPD.add(PAGE_TABLE_SIZE);

const KERNEL_STACK_SIZE: usize = 0x1000 * 8; // 32KB stack
pub const KERNEL_STACK: PhysicalAddr = KERNEL_CODE_PD
    .add(PAGE_TABLE_SIZE + KERNEL_STACK_SIZE)
    .align_up(PAGE_SIZE);

pub const KERNEL_CODE_PHYS: PhysicalAddr = KERNEL_STACK; // stack will grow down from this point, code will grow up
pub const KERNEL_CODE_SIZE: usize = PAGE_SIZE - RUN_FLAGS_SIZE;

// Boot-time flags written by VM before kernel starts.
pub const RUN_FLAGS_PHYS: PhysicalAddr = KERNEL_CODE_PHYS.add(KERNEL_CODE_SIZE);
pub const RUN_FLAGS_SIZE: usize = size_of::<RunFlags>();

pub const PALLOC_FIRST_PAGE: PhysicalAddr = RUN_FLAGS_PHYS.add(RUN_FLAGS_SIZE);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_layout_consistency() {
        assert_eq!(
            DIRECT_MAP_PML4.as_u64() % 4096,
            0,
            "PML4 must be 4KB aligned"
        );
        assert_eq!(
            DIRECT_MAP_PDPT.as_u64() % 4096,
            0,
            "PDPT must be 4KB aligned"
        );
        assert_eq!(DIRECT_MAP_PD.as_u64() % 4096, 0, "PD must be 4KB aligned");
        assert_eq!(
            KERNEL_CODE_PDPD.as_usize() % 4096,
            0,
            "Kernel PDPT must be 4KB aligned"
        );
        assert_eq!(
            KERNEL_CODE_PD.as_u64() % 4096,
            0,
            "Kernel PD must be 4KB aligned"
        );

        assert_eq!(
            KERNEL_CODE_PHYS.as_u64() % (2 << 20),
            0,
            "KERNEL_CODE_PHYS must be 2MB aligned for Huge Pages (PTE_PS)"
        );

        let dm_pd_end = DIRECT_MAP_PD.as_usize() + (DIRECT_MAP_PD_COUNT * 8);
        assert!(
            dm_pd_end <= KERNEL_CODE_PDPD.as_usize(),
            "Direct Map PD tables overlap with Kernel PDPT! End: {:#x}, Next: {:#x}",
            dm_pd_end,
            KERNEL_CODE_PDPD.as_usize()
        );

        let kernel_pd_end = KERNEL_CODE_PD.as_usize() + (PAGE_TABLE_ENTRIES * 8);
        assert!(
            kernel_pd_end <= KERNEL_STACK.as_usize()
                || KERNEL_STACK.as_usize() < KERNEL_CODE_PD.as_usize(),
            "Kernel PD tables overlap with Stack! End: {:#x}, Stack: {:#x}",
            kernel_pd_end,
            KERNEL_STACK.as_usize()
        );

        assert_eq!(KERNEL_CODE_VIRT.pml4_index(), PAGE_TABLE_ENTRIES - 1);

        assert!(KERNEL_CODE_VIRT.pdpt_index() == PAGE_TABLE_ENTRIES - 2);
    }
}
