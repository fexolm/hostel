use crate::memory::{
    address::{AddressError, PhysicalAddr, VirtualAddr},
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
    ) -> Result<(), AddressError> {
        // todo ensure safety guarantees
        let pml4 = PageTable::current_pml4_mut()?;
        let pde = pml4.get(vaddr)?;
        if pde.is_present() {
            todo!(); // error
        }
        pde.set_paddr(paddr);

        Ok(())
    }
}
