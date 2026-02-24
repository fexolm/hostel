pub mod error;
mod serial;
mod x64;

pub use self::error::{Error, Result};
use kernel::memory::constants::{KERNEL_CODE_SIZE, KERNEL_CODE_VIRT, MAX_PHYSICAL_ADDR};
use kvm_bindings::KVM_MAX_CPUID_ENTRIES;
use kvm_ioctls::{Kvm, VmFd};
use vm_memory::{Bytes, GuestAddress, GuestMemoryMmap};
use x64::{GUEST_BASE, init_x64};

// goblin is already a dependency of the workspace; we reuse it here to parse ELF
use goblin::elf::Elf;
use goblin::elf::program_header::PT_LOAD;
use serial::SerialConsole16550;

const MEM_SIZE: usize = MAX_PHYSICAL_ADDR as usize + 1;

pub struct Vm {
    _kvm: Kvm,
    _vm: VmFd,
    vcpus: Vec<kvm_ioctls::VcpuFd>,
    boot_mem: GuestMemoryMmap<()>,
    serial: SerialConsole16550,
}

impl Vm {
    pub fn new() -> Result<Self> {
        let kvm = Kvm::new()?;
        let vm = kvm.create_vm()?;
        let vcpu = vm.create_vcpu(0)?;
        let cpuid = kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES)?;
        vcpu.set_cpuid2(&cpuid)?;
        let vcpus = vec![vcpu];

        let boot_mem: GuestMemoryMmap<()> =
            GuestMemoryMmap::from_ranges(&[(GUEST_BASE, MEM_SIZE)])?;

        init_x64(&vm, &vcpus, &boot_mem, MEM_SIZE)?;

        Ok(Self {
            _kvm: kvm,
            _vm: vm,
            vcpus,
            boot_mem,
            serial: SerialConsole16550::new(),
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

            if ph.p_vaddr < KERNEL_CODE_VIRT.as_u64()
                || ph.p_vaddr + memsz as u64 > KERNEL_CODE_VIRT.as_u64() + KERNEL_CODE_SIZE as u64
            {
                return Err(Error::Parsing(goblin::error::Error::Malformed(format!(
                    "Program header with p_vaddr {:#x} and memsz {:#x} is out of bounds",
                    ph.p_vaddr, memsz
                ))));
            }

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

    /// Run the single vCPU until it halts.
    pub fn run(&mut self) -> Result<()> {
        use kvm_ioctls::VcpuExit;

        loop {
            match self.vcpus[0].run()? {
                VcpuExit::Hlt => {
                    self.serial.flush()?;
                    return Ok(());
                }
                VcpuExit::IoOut(port, data) => {
                    if self.serial.handles_range(port, data.len()) {
                        self.serial.io_out(port, data)?;
                    } else {
                        return Err(Error::UnexpectedExit(format!(
                            "unhandled IoOut on port {port:#x} with {} byte(s)",
                            data.len()
                        )));
                    }
                }
                VcpuExit::IoIn(port, data) => {
                    if self.serial.handles_range(port, data.len()) {
                        self.serial.io_in(port, data);
                    } else {
                        return Err(Error::UnexpectedExit(format!(
                            "unhandled IoIn on port {port:#x} with {} byte(s)",
                            data.len()
                        )));
                    }
                }
                other => return Err(Error::UnexpectedExit(format!("{:?}", other))),
            }
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
    use crate::vm::Vm;

    #[test]
    fn vm_loads_kernel_elf_from_build_script() {
        // the build script emits the path via the KERNEL_BIN environment variable
        let path = env!("KERNEL_BIN");
        let data = std::fs::read(path).expect("read kernel elf");

        let mut vm = Vm::new().unwrap();
        vm.load_elf(&data).expect("load elf");
        vm.run().expect("run guest");
    }
}
