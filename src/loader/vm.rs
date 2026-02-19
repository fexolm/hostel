use super::error::Result;
use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::{Kvm, VmFd};
use vm_memory::{Bytes, GuestAddress, GuestMemoryBackend, GuestMemoryMmap};

const MEM_SIZE: usize = 2 * 1024 * 1024;
const GUEST_BASE: GuestAddress = GuestAddress(0);
const PML4_ADDR: GuestAddress = GuestAddress(0x1000);
const PDPT_ADDR: GuestAddress = GuestAddress(0x2000);
const PD_ADDR: GuestAddress = GuestAddress(0x3000);
const DATA_ADDR: GuestAddress = GuestAddress(0x4000);
const CODE_ADDR: GuestAddress = GuestAddress(0x100000);
const STACK_TOP: u64 = (MEM_SIZE - 0x1000) as u64; // stack somewhere near the top

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

const BOOT_CODE: &[u8] = &[
    0xF4, // hlt
];

pub struct Vm {
    kvm: Kvm,
    vm: VmFd,
    vcpus: Vec<kvm_ioctls::VcpuFd>,
    boot_mem: GuestMemoryMmap<()>,
}

fn init_x64(
    kvm: &Kvm,
    vm: &VmFd,
    vcpus: &Vec<kvm_ioctls::VcpuFd>,
    boot_mem: &GuestMemoryMmap<()>,
    boot_code: &[u8],
) -> Result<()> {
    // Build minimal page tables: PML4 -> PDPT -> PD (2 MiB pages)
    // PML4[0] points to PDPT, PDPT[0] points to PD, PD[0] maps the first 2MiB.
    let pml4_entry: u64 = (PDPT_ADDR.0 as u64) | PML4_ENTRY_FLAGS; // PML4[0] -> PDPT
    let pdpt_entry: u64 = (PD_ADDR.0 as u64) | PML4_ENTRY_FLAGS; // PDPT[0] -> PD
    let pd_entry: u64 = (GUEST_BASE.0 as u64) | PD_2M_ENTRY_FLAGS; // PD[0] -> 2M pages

    boot_mem.write_slice(&pml4_entry.to_le_bytes(), PML4_ADDR)?;
    boot_mem.write_slice(&pdpt_entry.to_le_bytes(), PDPT_ADDR)?;
    boot_mem.write_slice(&pd_entry.to_le_bytes(), PD_ADDR)?;

    // Clear observable data area (guest will write a 64-bit value here)
    boot_mem.write_slice(&0u64.to_le_bytes(), DATA_ADDR)?;

    // Place the provided boot code at the expected entry point.
    boot_mem.write_slice(&boot_code, CODE_ADDR)?;

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
    regs.rip = CODE_ADDR.0; // entry point for payload
    regs.rsp = STACK_TOP; // initial stack pointer
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
        Self::with_boot_code(BOOT_CODE)
    }

    fn with_boot_code(boot_code: &[u8]) -> Result<Self> {
        let kvm = Kvm::new()?;
        let vm = kvm.create_vm()?;
        let mut vcpus = Vec::new();
        vcpus.push(vm.create_vcpu(0)?);

        let boot_mem: GuestMemoryMmap<()> =
            GuestMemoryMmap::from_ranges(&[(GUEST_BASE, MEM_SIZE)])?;

        init_x64(&kvm, &vm, &vcpus, &boot_mem, &boot_code)?;

        Ok(Self {
            kvm,
            vm,
            vcpus,
            boot_mem,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kvm_ioctls::VcpuExit;

    #[test]
    fn vm_runs_boot_code_and_operates_in_64bit() {
        const MAGIC: u64 = 0xdeadbeefcafebabeu64;

        // Build a tiny 64-bit payload:
        //   movabs rax, MAGIC
        //   mov [moffs64], rax
        //   hlt
        let mut code: Vec<u8> = Vec::new();
        code.extend_from_slice(&[0x48, 0xB8]); // movabs rax, imm64
        code.extend_from_slice(&MAGIC.to_le_bytes());
        code.extend_from_slice(&[0x48, 0xA3]); // mov [moffs64], rax
        code.extend_from_slice(&DATA_ADDR.0.to_le_bytes());
        code.push(0xF4); // hlt

        // Create a VM with our test boot code (sets up page-tables + long mode)
        let mut vm = Vm::with_boot_code(&code).expect("create vm");

        // Sanity: sregs should indicate long mode / 64-bit CS
        let sregs = vm.vcpus[0].get_sregs().expect("get sregs");
        assert!((sregs.efer & EFER_LMA) != 0, "expected LMA (long mode) enabled in EFER");
        assert_eq!(sregs.cs.l, 1);

        // Run the vCPU until it HLTs
        match vm.vcpus[0].run().expect("vcpu run failed") {
            VcpuExit::Hlt => {}
            other => panic!("unexpected vcpu exit: {:?}", other),
        }

        // Verify guest wrote the 64-bit value to DATA_ADDR (proves 64-bit execution)
        let mut buf = [0u8; 8];
        vm.boot_mem
            .read_slice(&mut buf, DATA_ADDR)
            .expect("read guest memory");
        assert_eq!(u64::from_le_bytes(buf), MAGIC);
    }
}
