use crate::vm::Result;
use kernel::memory::constants::{
    DIRECT_MAP_PD, DIRECT_MAP_PD_COUNT, DIRECT_MAP_PDPT, DIRECT_MAP_PDPT_COUNT, DIRECT_MAP_PML4,
    DIRECT_MAP_PML4_ENTRIES_COUNT, DIRECT_MAP_PML4_OFFSET, KERNEL_CODE_PD, KERNEL_CODE_PDPD,
    KERNEL_CODE_PHYS, KERNEL_CODE_VIRT, KERNEL_STACK, PAGE_SIZE, PAGE_TABLE_ENTRIES,
    PAGE_TABLE_SIZE,
};
use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::VmFd;
use vm_memory::{Bytes, GuestAddress, GuestMemoryBackend, GuestMemoryMmap};

// Page-table / PTE flag bits
const PTE_PRESENT: u64 = 0x1;
const PTE_RW: u64 = 0x2;
const PTE_PS: u64 = 0x80;

// Control-register / system constants
const CR4_PAE: u64 = 1 << 5;
const CR4_OSFXSR: u64 = 1 << 9;
const CR4_OSXMMEXCPT: u64 = 1 << 10;
const EFER_LME: u64 = 1 << 8;
const EFER_LMA: u64 = 1 << 10;
const CR0_PE: u64 = 1 << 0;
const CR0_MP: u64 = 1 << 1;
const CR0_EM: u64 = 1 << 2;
const CR0_TS: u64 = 1 << 3;
const CR0_NE: u64 = 1 << 5;
const CR0_PG: u64 = 1 << 31;
const RFLAGS_RESERVED: u64 = 2;

// Segment selectors / descriptor types
const CS_SELECTOR: u16 = 0x8;
const SS_SELECTOR: u16 = 0x10;
const CS_TYPE: u8 = 0xB;
const SS_TYPE: u8 = 0x3;

pub const GUEST_BASE: GuestAddress = GuestAddress(0);

fn u64_from_usize(value: usize) -> u64 {
    u64::try_from(value).expect("usize value fits u64")
}


pub fn init_x64(
    vm: &VmFd,
    vcpus: &[kvm_ioctls::VcpuFd],
    boot_mem: &GuestMemoryMmap<()>,
    mem_size: usize,
) -> Result<()> {
    // map direct map region
    for i in 0..DIRECT_MAP_PML4_ENTRIES_COUNT {
        let entry_val =
            (DIRECT_MAP_PDPT.as_u64() + u64_from_usize(i) * u64_from_usize(PAGE_TABLE_SIZE)) | PTE_PRESENT | PTE_RW;
        let entry_addr =
            GuestAddress(DIRECT_MAP_PML4.as_u64() + u64_from_usize((DIRECT_MAP_PML4_OFFSET + i) * 8));
        boot_mem.write_slice(&entry_val.to_le_bytes(), entry_addr)?;
    }

    for i in 0..DIRECT_MAP_PDPT_COUNT * PAGE_TABLE_ENTRIES {
        let pd_phys = DIRECT_MAP_PD.as_u64() + u64_from_usize(i) * u64_from_usize(PAGE_TABLE_SIZE);
        let entry_val = pd_phys | PTE_PRESENT | PTE_RW;
        let entry_addr = GuestAddress(DIRECT_MAP_PDPT.as_u64() + u64_from_usize(i * 8));
        boot_mem.write_slice(&entry_val.to_le_bytes(), entry_addr)?;
    }

    for i in 0..DIRECT_MAP_PD_COUNT * PAGE_TABLE_ENTRIES {
        let phys = u64_from_usize(i) * u64_from_usize(PAGE_SIZE);
        let entry_val = phys | PTE_PRESENT | PTE_RW | PTE_PS;
        let entry_addr = GuestAddress(DIRECT_MAP_PD.as_u64() + u64_from_usize(i * 8));
        boot_mem.write_slice(&entry_val.to_le_bytes(), entry_addr)?;
    }

    // map kernel code region
    let kernel_pml4_val = KERNEL_CODE_PDPD.as_u64() | PTE_PRESENT | PTE_RW;
    let kernel_pml4_addr =
        GuestAddress(DIRECT_MAP_PML4.as_u64() + u64_from_usize(KERNEL_CODE_VIRT.pml4_index() * 8));
    boot_mem.write_slice(&kernel_pml4_val.to_le_bytes(), kernel_pml4_addr)?;

    for i in 0..2 {
        let pd_phys = KERNEL_CODE_PD.as_u64() + (u64_from_usize(i) * u64_from_usize(PAGE_TABLE_SIZE));
        let entry_val = pd_phys | PTE_PRESENT | PTE_RW;
        let entry_addr =
            GuestAddress(KERNEL_CODE_PDPD.as_u64() + u64_from_usize((KERNEL_CODE_VIRT.pdpt_index() + i) * 8));
        boot_mem.write_slice(&entry_val.to_le_bytes(), entry_addr)?;
    }

    for i in 0..PAGE_TABLE_ENTRIES {
        let phys = KERNEL_CODE_PHYS.add(i * PAGE_SIZE).as_u64();
        let entry_val = phys | PTE_PRESENT | PTE_RW | PTE_PS;
        let entry_addr = GuestAddress(KERNEL_CODE_PD.as_u64() + u64_from_usize(i * 8));
        boot_mem.write_slice(&entry_val.to_le_bytes(), entry_addr)?;
    }

    // Register the guest memory region with KVM.
    unsafe {
        vm.set_user_memory_region(kvm_userspace_memory_region {
            slot: 0,
            guest_phys_addr: GUEST_BASE.0,
            memory_size: u64_from_usize(mem_size),
            userspace_addr: u64_from_usize(boot_mem.get_host_address(GUEST_BASE).unwrap() as usize),
            flags: 0,
        })?;
    }

    // General purpose registers:
    // - RIP: instruction pointer where the guest will start executing
    // - RSP: stack pointer inside guest memory
    // - RFLAGS: set the reserved bit required by x86
    let mut regs = vcpus[0].get_regs()?;
    regs.rsp = KERNEL_STACK.to_virtual().unwrap().as_u64(); // initial stack pointer
    regs.rflags = RFLAGS_RESERVED; // required reserved bit
    vcpus[0].set_regs(&regs)?;

    // Special registers (control & segment registers) for entering long mode.
    let mut sregs = vcpus[0].get_sregs()?;
    sregs.cr3 = DIRECT_MAP_PML4.as_u64(); // CR3 = physical address of the PML4 (page-table root)

    // CR4.PAE must be set to enable physical-address-extension paging required
    // by 64-bit mode page tables.
    sregs.cr4 |= CR4_PAE | CR4_OSFXSR | CR4_OSXMMEXCPT;

    // EFER.LME enables Long Mode; EFER.LMA indicates Long Mode Active.
    sregs.efer = EFER_LME | EFER_LMA;

    // Code segment descriptor: set as a 64-bit code segment.
    sregs.cs.l = 1; // L bit = 1 => 64-bit code segment
    sregs.cs.db = 0; // DB = 0 => default operand size is 32-bit (unused in 64-bit)
    sregs.cs.s = 1; // S = 1 => code/data descriptor (not system)
    sregs.cs.type_ = CS_TYPE; // executable, read, accessed
    sregs.cs.present = 1;
    sregs.cs.dpl = 0; // ring 0
    sregs.cs.selector = CS_SELECTOR;

    // Stack/data segment for the guest (selector points into the GDT).
    sregs.ss.s = 1;
    sregs.ss.type_ = SS_TYPE;
    sregs.ss.present = 1;
    sregs.ss.selector = SS_SELECTOR;

    // KVM allows zero-sized GDT/IDT here because we supply selectors directly.
    sregs.gdt.limit = 0;
    sregs.idt.limit = 0;

    // CR0: enable protected mode (PE) and paging (PG). Also enable NE (numeric
    // error) so x87 exceptions behave as expected.
    sregs.cr0 |= CR0_PG | CR0_PE | CR0_MP; // paging + protected mode + monitor coprocessor
    sregs.cr0 |= CR0_NE; // numeric error
    sregs.cr0 &= !CR0_EM; // enable x87/SSE instructions
    sregs.cr0 &= !CR0_TS; // allow immediate FPU/SSE use

    vcpus[0].set_sregs(&sregs)?;

    Ok(())
}
