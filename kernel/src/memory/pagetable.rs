use core::arch::asm;
use core::ptr::copy_nonoverlapping;
use core::ptr::write_bytes;

use crate::memory::{
    address::{PhysicalAddr, VirtualAddr},
    alloc::{
        kmalloc::{kfree, kmalloc},
        palloc::pfree,
    },
    constants::{DIRECT_MAP_OFFSET, DIRECT_MAP_PML4, PAGE_TABLE_ENTRIES, PAGE_TABLE_SIZE},
    errors::{MemoryError, Result},
};

const PRESENT: usize = 1 << 0;
const WRITABLE: usize = 1 << 1;
const USER_ACCESSIBLE: usize = 1 << 2;
const HUGE_PAGE: usize = 1 << 7;
const ADDR_MASK: usize = 0x000F_FFFF_FFFF_F000;
const USER_PML4_LIMIT: usize = DIRECT_MAP_OFFSET.pml4_index();

#[derive(Clone, Copy)]
pub struct PageTableEntry(usize);

impl PageTableEntry {
    pub fn set_table(&mut self, addr: PhysicalAddr) {
        self.0 = addr.as_usize() | PRESENT | WRITABLE | USER_ACCESSIBLE;
    }

    pub fn set_paddr(&mut self, addr: PhysicalAddr) {
        self.0 = addr.as_usize() | PRESENT | WRITABLE | USER_ACCESSIBLE | HUGE_PAGE;
    }

    pub fn is_present(&self) -> bool {
        (self.0 & PRESENT) != 0
    }

    pub fn addr(&self) -> PhysicalAddr {
        PhysicalAddr::new(self.0 & ADDR_MASK)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PageTableLevel {
    Pml4,
    Pdpt,
    Pd,
}

impl PageTableLevel {
    fn next(self) -> Option<Self> {
        match self {
            Self::Pml4 => Some(Self::Pdpt),
            Self::Pdpt => Some(Self::Pd),
            Self::Pd => None,
        }
    }
}

#[repr(C, align(4096))]
pub struct PageTable {
    entries: [PageTableEntry; PAGE_TABLE_ENTRIES],
}

impl PageTable {
    pub fn current_pml4_mut() -> Result<&'static mut Self> {
        Self::from_paddr_mut(read_cr3())
    }

    pub fn from_paddr(paddr: PhysicalAddr) -> Result<&'static Self> {
        let vaddr = paddr.to_virtual()?;
        Ok(unsafe { &*vaddr.as_ptr::<Self>() })
    }

    pub fn from_paddr_mut(paddr: PhysicalAddr) -> Result<&'static mut Self> {
        Ok(unsafe { paddr.to_virtual()?.as_ref_mut::<Self>() })
    }

    pub fn alloc_user_pml4() -> Result<PhysicalAddr> {
        let paddr = alloc_zeroed_table()?;
        let pml4 = Self::from_paddr_mut(paddr)?;
        let kernel = Self::from_paddr_mut(DIRECT_MAP_PML4)?;

        unsafe {
            copy_nonoverlapping(
                kernel.entries.as_ptr().add(USER_PML4_LIMIT),
                pml4.entries.as_mut_ptr().add(USER_PML4_LIMIT),
                PAGE_TABLE_ENTRIES - USER_PML4_LIMIT,
            );
        }

        Ok(paddr)
    }

    pub fn get(&mut self, vaddr: VirtualAddr) -> Result<&mut PageTableEntry> {
        self.get_level(vaddr, PageTableLevel::Pml4)
    }

    fn get_level(
        &mut self,
        vaddr: VirtualAddr,
        level: PageTableLevel,
    ) -> Result<&mut PageTableEntry> {
        if level == PageTableLevel::Pd {
            return Ok(&mut self.entries[index_for(level, vaddr)]);
        }

        let entry = &mut self.entries[index_for(level, vaddr)];
        if !entry.is_present() {
            entry.set_table(alloc_zeroed_table()?);
        }

        let Some(next) = level.next() else {
            return Err(MemoryError::VirtualToPhysical {
                addr: vaddr.as_usize(),
            });
        };
        let child = Self::from_paddr_mut(entry.addr())?;
        child.get_level(vaddr, next)
    }

    pub fn free(&mut self) -> Result<()> {
        self.free_level(PageTableLevel::Pml4)
    }

    fn free_level(&mut self, level: PageTableLevel) -> Result<()> {
        let end = if level == PageTableLevel::Pml4 {
            USER_PML4_LIMIT
        } else {
            PAGE_TABLE_ENTRIES
        };

        if let Some(next) = level.next() {
            for i in 0..end {
                let entry = self.entries[i];
                if entry.is_present() {
                    let child = Self::from_paddr_mut(entry.addr())?;
                    child.free_level(next)?;
                }
            }
        } else {
            for i in 0..end {
                let entry = self.entries[i];
                if entry.is_present() {
                    pfree(entry.addr())?;
                }
            }
        }

        kfree(self.self_vaddr())?;
        Ok(())
    }

    fn self_vaddr(&self) -> VirtualAddr {
        VirtualAddr::new(self as *const Self as usize)
    }
}

fn alloc_zeroed_table() -> Result<PhysicalAddr> {
    let vaddr = kmalloc(PAGE_TABLE_SIZE)?;
    unsafe {
        write_bytes(vaddr.as_ptr::<u8>(), 0, PAGE_TABLE_SIZE);
    }
    vaddr
        .to_physical()
        .map_err(|_| MemoryError::PointerNotInDirectMap {
            addr: vaddr.as_usize(),
        })
}

fn read_cr3() -> PhysicalAddr {
    let value: u64;
    unsafe {
        asm!("mov {}, cr3", out(reg) value, options(nostack, preserves_flags));
    }
    PhysicalAddr::new(value as usize)
}

fn index_for(level: PageTableLevel, vaddr: VirtualAddr) -> usize {
    match level {
        PageTableLevel::Pml4 => vaddr.pml4_index(),
        PageTableLevel::Pdpt => vaddr.pdpt_index(),
        PageTableLevel::Pd => vaddr.pd_index(),
    }
}
