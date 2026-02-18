use std::{io::Write, sync::Arc};

use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::{Kvm, VmFd};
pub mod error;
pub mod module;

pub use error::{Error, Result};
use vm_memory::{Bytes, GuestAddress, GuestMemoryBackend, GuestMemoryMmap};

use crate::loader::module::Module;

pub struct Loader {
    kvm: Kvm,
    vm: VmFd,
    vcpus: Vec<kvm_ioctls::VcpuFd>,
}

impl Loader {
    pub fn new() -> Result<Self> {
        let kvm = Kvm::new()?;
        let vm = kvm.create_vm()?;
        let mut vcpus = Vec::new();
        vcpus.push(vm.create_vcpu(0)?);
        Ok(Self { kvm, vm, vcpus })
    }

    pub fn load(&mut self, filename: &str) -> Result<Arc<Module>> {
        let addr = GuestAddress(0x0);
        let len = 4096u64;
        let mem: GuestMemoryMmap<()> = GuestMemoryMmap::from_ranges(&[(addr, len as usize)])?;

        mem.write_slice(
            &[0xba, 0xf8, 0x03, 0x00, 0xd8, 0x04, b'0', 0xee, 0xf4],
            addr,
        )?;

        unsafe {
            self.vm
                .set_user_memory_region(kvm_userspace_memory_region {
                    slot: 0,
                    guest_phys_addr: addr.0,
                    memory_size: len,
                    userspace_addr: mem.get_host_address(addr).unwrap() as u64,
                    flags: 0,
                })?;
        }

        let mut regs = self.vcpus[0].get_regs()?;
        regs.rip = addr.0;
        regs.rax = 2;
        regs.rbx = 2;
        self.vcpus[0].set_regs(&regs)?;

        let mut sregs = self.vcpus[0].get_sregs()?;
        sregs.cs.base = 0;
        sregs.cs.selector = 0;
        // Для уверенности обнуляем DS (data segment)
        sregs.ds.base = 0;
        sregs.ds.selector = 0;
        self.vcpus[0].set_sregs(&sregs)?;

        loop {
            match self.vcpus[0].run() {
                Ok(kvm_ioctls::VcpuExit::Hlt) => {
                    println!("Guest halted");
                    break;
                }
                Ok(kvm_ioctls::VcpuExit::IoOut(0x3f8, data)) => {
                    std::io::stdout().write_all(data).unwrap();
                    break;
                }
                Ok(exit_reason) => {
                    println!("Unexpected exit reason: {:?}", exit_reason);
                    break;
                }
                Err(e) => {
                    println!("Error running vCPU: {}", e);
                    break;
                }
            }
        }

        let module = Module::new(vec![mem], Vec::new());
        Ok(Arc::new(module))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_new_and_load() {
        let mut loader = Loader::new().unwrap();

        let module = loader.load("kek").unwrap();
    }
}
