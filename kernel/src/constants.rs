use crate::address::{PhysicalAddr, VirtualAddr};

pub const PAGE_SIZE: u64 = 2 << 20;
pub const MAX_PHYSICAL_ADDR: u64 = 0x0000_00FF_FFFF_FFFF;

pub const PAGE_TABLE_ENTRIES: u64 = 512;
pub const PAGE_TABLE_SIZE: u64 = 8 * PAGE_TABLE_ENTRIES;

// offset of kernel code in virtual address space; the kernel is linked to run at this address
pub const KERNEL_CODE_VIRT: VirtualAddr = VirtualAddr(0xFFFF_FFFF_8000_0000);

// direct map
pub const DIRECT_MAP_OFFSET: VirtualAddr = VirtualAddr(0xFFFF_8880_0000_0000);

pub const DIRECT_MAP_PML4: PhysicalAddr = PhysicalAddr(0x0);

// the PML4 entry index for the direct map region; this is used to set up the initial page tables
pub const DIRECT_MAP_PML4_OFFSET: u64 = DIRECT_MAP_OFFSET.pml4_index() as u64 * 8;
pub const DIRECT_MAP_PML4_ENTRIES_COUNT: u64 =
    (DIRECT_MAP_PDPT_COUNT + PAGE_TABLE_ENTRIES - 1) / PAGE_TABLE_ENTRIES; // number of PML4 entries needed to cover the direct map region

pub const DIRECT_MAP_PDPT: PhysicalAddr = DIRECT_MAP_PML4.add(PAGE_TABLE_SIZE);
pub const DIRECT_MAP_PDPT_COUNT: u64 =
    MAX_PHYSICAL_ADDR / (PAGE_SIZE * PAGE_TABLE_ENTRIES * PAGE_TABLE_ENTRIES);
pub const DIRECT_MAP_PD: PhysicalAddr =
    DIRECT_MAP_PDPT.add(DIRECT_MAP_PDPT_COUNT * PAGE_TABLE_SIZE);
pub const DIRECT_MAP_PD_COUNT: u64 = MAX_PHYSICAL_ADDR / (PAGE_SIZE * PAGE_TABLE_ENTRIES);

// pdpd and pd for the kernel code (we need to reserver 2gb of virtual address space for kernel code, for code-model=kernel)
pub const KERNEL_CODE_PDPD: PhysicalAddr = DIRECT_MAP_PD.add(DIRECT_MAP_PD_COUNT * PAGE_TABLE_SIZE);
pub const KERNEL_CODE_PD: PhysicalAddr = KERNEL_CODE_PDPD.add(PAGE_TABLE_SIZE);

const KERNEL_STACK_SIZE: u64 = 0x1000 * 8; // 32KB stack
pub const KERNEL_STACK: PhysicalAddr = KERNEL_CODE_PD
    .add(PAGE_TABLE_SIZE + KERNEL_STACK_SIZE)
    .align_up(PAGE_SIZE);

pub const KERNEL_CODE_PHYS: PhysicalAddr = KERNEL_STACK; // stack will grow down from this point, code will grow up
pub const KERNEL_CODE_SIZE: u64 = PAGE_SIZE;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_layout_consistency() {
        assert_eq!(DIRECT_MAP_PML4.0 % 4096, 0, "PML4 must be 4KB aligned");
        assert_eq!(DIRECT_MAP_PDPT.0 % 4096, 0, "PDPT must be 4KB aligned");
        assert_eq!(DIRECT_MAP_PD.0 % 4096, 0, "PD must be 4KB aligned");
        assert_eq!(
            KERNEL_CODE_PDPD.0 % 4096,
            0,
            "Kernel PDPT must be 4KB aligned"
        );
        assert_eq!(KERNEL_CODE_PD.0 % 4096, 0, "Kernel PD must be 4KB aligned");

        assert_eq!(
            KERNEL_CODE_PHYS.0 % (2 << 20),
            0,
            "KERNEL_CODE_PHYS must be 2MB aligned for Huge Pages (PTE_PS)"
        );

        let dm_pd_end = DIRECT_MAP_PD.0 + (DIRECT_MAP_PD_COUNT * 8);
        assert!(
            dm_pd_end <= KERNEL_CODE_PDPD.0,
            "Direct Map PD tables overlap with Kernel PDPT! End: {:#x}, Next: {:#x}",
            dm_pd_end,
            KERNEL_CODE_PDPD.0
        );

        let kernel_pd_end = KERNEL_CODE_PD.0 + (PAGE_TABLE_ENTRIES * 8);
        assert!(
            kernel_pd_end <= KERNEL_STACK.0 || KERNEL_STACK.0 < KERNEL_CODE_PD.0,
            "Kernel PD tables overlap with Stack! End: {:#x}, Stack: {:#x}",
            kernel_pd_end,
            KERNEL_STACK.0
        );

        assert_eq!(KERNEL_CODE_VIRT.pml4_index(), PAGE_TABLE_ENTRIES - 1);

        assert!(KERNEL_CODE_VIRT.pdpt_index() == PAGE_TABLE_ENTRIES - 2);
    }
}
