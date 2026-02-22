use crate::types::VirtualAddr;

pub const PAGE_SIZE: u64 = 2 << 20;

pub const KERNEL_BASE: VirtualAddr = VirtualAddr(0xFFFF_FFFF_8000_0000);
pub const KERNEL_PML4_IDX: u64 = 511;
pub const KERNEL_PDPT_INDEX: u64 = 510;

pub const KERNEL_PML4_OFFSET: u64 = 0x0;
pub const KERNEL_PDPT_OFFSET: u64 = KERNEL_PML4_OFFSET + 0x1000;
pub const KERNEL_PD_OFFSET: u64 = KERNEL_PDPT_OFFSET + 0x1000;
pub const KERNEL_CODE_OFFSET: u64 = KERNEL_PD_OFFSET + 0x1000 + 0x1000;
pub const KERNEL_STACK_OFFSET: u64 = PAGE_SIZE - 0x1000;

pub const KERNEL_PML4: VirtualAddr = KERNEL_BASE.add(KERNEL_PML4_OFFSET);
pub const KERNEL_PDPT: VirtualAddr = KERNEL_BASE.add(KERNEL_PDPT_OFFSET);
pub const KERNEL_PD: VirtualAddr = KERNEL_BASE.add(KERNEL_PD_OFFSET);
pub const KERNEL_CODE: VirtualAddr = KERNEL_BASE.add(KERNEL_CODE_OFFSET);
pub const KERNEL_STACK: VirtualAddr = KERNEL_BASE.add(KERNEL_STACK_OFFSET);
