use crate::memory::{
    address::{PhysicalAddr, VirtualAddr},
    errors::{MemoryError, Result},
    pagetable::PageTable,
};

pub struct Vmm {}

impl Vmm {
    pub fn new() -> Self {
        Self {}
    }

    pub fn map_user_memory(
        &mut self,
        paddr: PhysicalAddr,
        vaddr: VirtualAddr,
    ) -> Result<()> {
        // todo ensure safety guarantees
        let pml4 = PageTable::current_pml4_mut()?;
        let pde = pml4.get(vaddr)?;
        if pde.is_present() {
            return Err(MemoryError::AlreadyMapped {
                addr: vaddr.as_u64(),
            });
        }
        pde.set_paddr(paddr);

        Ok(())
    }
}
