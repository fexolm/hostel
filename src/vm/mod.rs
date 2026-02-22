pub mod error;

pub use self::error::{Error, Result};

use kernel::constants::{
    KERNEL_PD_OFFSET, KERNEL_PDPT_INDEX, KERNEL_PDPT_OFFSET, KERNEL_PML4_IDX, KERNEL_PML4_OFFSET,
    KERNEL_STACK,
};
use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::{Kvm, VmFd};
use vm_memory::{Bytes, GuestAddress, GuestMemoryBackend, GuestMemoryMmap};

// goblin is already a dependency of the workspace; we reuse it here to parse ELF
use goblin::elf::Elf;
use goblin::elf::program_header::PT_LOAD;

const MEM_SIZE: usize = 2 * 1024 * 1024 * 1024 * 1024;
const GUEST_BASE: GuestAddress = GuestAddress(0);
const PML4_ADDR: GuestAddress = GuestAddress(KERNEL_PML4_OFFSET);
const KERNEL_PML4_ADDR: GuestAddress = GuestAddress(KERNEL_PML4_OFFSET + KERNEL_PML4_IDX * 8);
const PDPT_ADDR: GuestAddress = GuestAddress(KERNEL_PDPT_OFFSET);
const KERNEL_PDPT_ADDR: GuestAddress = GuestAddress(KERNEL_PDPT_OFFSET + KERNEL_PDPT_INDEX * 8);
const PD_ADDR: GuestAddress = GuestAddress(KERNEL_PD_OFFSET);

// Page-table / PTE flag bits
const PTE_PRESENT: u64 = 0x1;
const PTE_RW: u64 = 0x2;
const PTE_PS: u64 = 0x80;
const PML4_ENTRY_FLAGS: u64 = PTE_PRESENT | PTE_RW; // present + read/write
const PD_2M_ENTRY_FLAGS: u64 = PTE_PRESENT | PTE_RW | PTE_PS; // 2MB page entry

// Control-register / system constants
const CR4_PAE: u64 = 1 << 5;
const EFER_LME: u64 = 1 << 8;
const EFER_LMA: u64 = 1 << 10;
const CR0_PE: u64 = 1 << 0;
const CR0_NE: u64 = 1 << 5;
const CR0_PG: u64 = 1 << 31;
const RFLAGS_RESERVED: u64 = 2;

// Segment selectors / descriptor types
const CS_SELECTOR: u16 = 0x8;
const SS_SELECTOR: u16 = 0x10;
const CS_TYPE: u8 = 0xB;
const SS_TYPE: u8 = 0x3;

pub struct Vm {
    _kvm: Kvm,
    _vm: VmFd,
    vcpus: Vec<kvm_ioctls::VcpuFd>,
    boot_mem: GuestMemoryMmap<()>,
}

fn init_x64(vm: &VmFd, vcpus: &[kvm_ioctls::VcpuFd], boot_mem: &GuestMemoryMmap<()>) -> Result<()> {
    let pml4_entry: u64 = PDPT_ADDR.0 | PML4_ENTRY_FLAGS; // PML4[KERNEL_PML4_IDX] -> PDPT
    let pdpt_entry: u64 = PD_ADDR.0 | PML4_ENTRY_FLAGS; // PDPT[0] -> PD
    let pd_entry: u64 = GUEST_BASE.0 | PD_2M_ENTRY_FLAGS; // PD[0] -> 2M pages

    boot_mem.write_slice(&pml4_entry.to_le_bytes(), KERNEL_PML4_ADDR)?;
    boot_mem.write_slice(&pdpt_entry.to_le_bytes(), KERNEL_PDPT_ADDR)?;
    boot_mem.write_slice(&pd_entry.to_le_bytes(), PD_ADDR)?;

    // Register the guest memory region with KVM.
    unsafe {
        vm.set_user_memory_region(kvm_userspace_memory_region {
            slot: 0,
            guest_phys_addr: GUEST_BASE.0,
            memory_size: MEM_SIZE as u64,
            userspace_addr: boot_mem.get_host_address(GUEST_BASE).unwrap() as u64,
            flags: 0,
        })?;
    }

    // General purpose registers:
    // - RIP: instruction pointer where the guest will start executing
    // - RSP: stack pointer inside guest memory
    // - RFLAGS: set the reserved bit required by x86
    let mut regs = vcpus[0].get_regs()?;
    regs.rsp = KERNEL_STACK.0; // initial stack pointer 
    regs.rflags = RFLAGS_RESERVED; // required reserved bit
    vcpus[0].set_regs(&regs)?;

    // Special registers (control & segment registers) for entering long mode.
    let mut sregs = vcpus[0].get_sregs()?;
    sregs.cr3 = PML4_ADDR.0; // CR3 = physical address of the PML4 (page-table root)

    // CR4.PAE must be set to enable physical-address-extension paging required
    // by 64-bit mode page tables.
    sregs.cr4 |= CR4_PAE;

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
    sregs.cr0 |= CR0_PG | CR0_PE; // paging + protected mode
    sregs.cr0 |= CR0_NE; // numeric error

    vcpus[0].set_sregs(&sregs)?;

    Ok(())
}

impl Vm {
    pub fn new() -> Result<Self> {
        let kvm = Kvm::new()?;
        let vm = kvm.create_vm()?;
        let vcpus = vec![vm.create_vcpu(0)?];

        let boot_mem: GuestMemoryMmap<()> =
            GuestMemoryMmap::from_ranges(&[(GUEST_BASE, MEM_SIZE)])?;

        init_x64(&vm, &vcpus, &boot_mem)?;

        Ok(Self {
            _kvm: kvm,
            _vm: vm,
            vcpus,
            boot_mem,
        })
    }

    /// Load an executable ELF blob into the guest memory and adjust the entry
    /// point accordingly.  The loader expects that the guest memory has already
    /// been registered with KVM (done in `Vm::new`).
    pub fn load_elf(&mut self, data: &[u8]) -> Result<()> {
        let elf = Elf::parse(data)?;

        for ph in &elf.program_headers {
            if ph.p_type != PT_LOAD {
                continue;
            }

            let file_offset = ph.p_offset as usize;
            let filesz = ph.p_filesz as usize;
            let memsz = ph.p_memsz as usize;

            // copy the initialized data from the file
            self.boot_mem.write_slice(
                &data[file_offset..file_offset + filesz],
                GuestAddress(ph.p_paddr),
            )?;

            // zero the remainder of the segment if any
            if memsz > filesz {
                let zero_addr = GuestAddress(ph.p_paddr + filesz as u64);
                let zero_buf = vec![0u8; memsz - filesz];
                self.boot_mem.write_slice(&zero_buf, zero_addr)?;
            }
        }

        // update the guest RIP to the ELF entry point
        let mut regs = self.vcpus[0].get_regs()?;
        regs.rip = elf.entry;
        self.vcpus[0].set_regs(&regs)?;

        Ok(())
    }

    /// Run the single vCPU until it exits.  Returns an error if the exit type is
    /// unexpected (anything other than `Hlt`).
    pub fn run(&mut self) -> Result<()> {
        use kvm_ioctls::VcpuExit;

        match self.vcpus[0].run()? {
            VcpuExit::Hlt => Ok(()),
            other => Err(Error::UnexpectedExit(format!("{:?}", other))),
        }
    }

    /// Return a reference to the guest physical memory.  This is primarily used
    /// by tests so that they can inspect memory after the VM has executed.
    pub fn guest_memory(&self) -> &GuestMemoryMmap<()> {
        &self.boot_mem
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_loads_kernel_elf_from_build_script() {
        // the build script emits the path via the KERNEL_BIN environment variable
        let path = env!("KERNEL_BIN");
        let data = std::fs::read(path).expect("read kernel elf");

        let mut vm = Vm::new().expect("create vm");
        vm.load_elf(&data).expect("load elf");
        vm.run().expect("run guest");
    }
}
