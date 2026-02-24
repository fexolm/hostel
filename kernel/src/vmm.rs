use core::arch::asm;

use crate::{
    address::{AddressError, PhysicalAddr, VirtualAddr},
    constants::PAGE_TABLE_ENTRIES,
    pagetable::PageTableEntry,
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
        let pd = unsafe { self.lookup_pd(vaddr)? };

        if pd.is_present() {
            todo!(); // error
        }

        pd.set_paddr(paddr);

        Ok(())
    }

    fn get_pml4() -> PhysicalAddr {
        let cr3_value: u64;

        unsafe {
            asm!("mov {}, cr3", out(reg) cr3_value);
        }

        PhysicalAddr::new(cr3_value)
    }

    unsafe fn lookup_pd(
        &mut self,
        vaddr: VirtualAddr,
    ) -> Result<&mut PageTableEntry, AddressError> {
        let pml4 = Self::get_pml4()
            .to_virtual()?
            .as_ref_mut::<[PageTableEntry; PAGE_TABLE_ENTRIES as usize]>();

        let pml4_entry = &mut pml4[vaddr.pml4_index() as usize];

        if !pml4_entry.is_present() {
            todo!(); // pml4_entry.set_table(...);
        }

        let pdpt = pml4_entry
            .addr()
            .to_virtual()?
            .as_ref_mut::<[PageTableEntry; PAGE_TABLE_ENTRIES as usize]>();

        let pdpt_entry = &mut pdpt[vaddr.pdpt_index() as usize];

        if !pdpt_entry.is_present() {
            todo!(); // pdpt_entry.set_table(...);
        }

        let pd = pdpt_entry
            .addr()
            .to_virtual()?
            .as_ref_mut::<[PageTableEntry; PAGE_TABLE_ENTRIES as usize]>();

        Ok(&mut pd[vaddr.pd_index() as usize])
    }
}
