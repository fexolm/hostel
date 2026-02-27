use crate::memory::{
    address::{PhysicalAddr, VirtualAddr},
    alloc::kmalloc::KernelAllocator,
    constants::PAGE_SIZE,
    errors::{MemoryError, Result},
    pagetable::RootPageTable,
};

const USER_HEAP_BASE: usize = 0x0000_0001_0000_0000;
const USER_MMAP_BASE: usize = 0x0000_0004_0000_0000;
const USER_MMAP_LIMIT: usize = 0x0000_7000_0000_0000;
const MAP_FIXED: u64 = 0x10;

pub struct Vmm<'i> {
    heap_base: usize,
    brk: usize,
    brk_mapped_end: usize,
    mmap_base: usize,
    mmap_next: usize,
    kalloc: &'i KernelAllocator<'i>,
    page_table: RootPageTable<'i>,
}

impl<'i> Vmm<'i> {
    pub fn new(
        kernel_page_table: &'i RootPageTable<'i>,
        kalloc: &'i KernelAllocator<'i>,
    ) -> Result<Self> {
        Ok(Self {
            heap_base: USER_HEAP_BASE,
            brk: USER_HEAP_BASE,
            brk_mapped_end: USER_HEAP_BASE,
            mmap_base: USER_MMAP_BASE,
            mmap_next: USER_MMAP_BASE,
            kalloc,
            page_table: RootPageTable::new(kernel_page_table, kalloc)?,
        })
    }

    pub fn root(&self) -> PhysicalAddr {
        self.page_table.addr()
    }

    fn map_user_memory(&mut self, paddr: PhysicalAddr, vaddr: VirtualAddr) -> Result<()> {
        let pde = self.page_table.get(vaddr)?;
        if pde.is_present() {
            return Err(MemoryError::AlreadyMapped {
                addr: vaddr.as_usize(),
            });
        }
        pde.set_paddr(paddr);

        Ok(())
    }

    pub fn brk(&mut self, requested: usize) -> Result<usize> {
        if requested == 0 {
            return Ok(self.brk);
        }
        if requested < self.heap_base || requested >= self.mmap_base {
            return Err(MemoryError::VirtualToPhysical { addr: requested });
        }

        let target_mapped_end = align_up(requested, PAGE_SIZE).ok_or(MemoryError::OutOfMemory)?;
        while self.brk_mapped_end < target_mapped_end {
            self.map_user_page(self.brk_mapped_end)?;
            self.brk_mapped_end += PAGE_SIZE;
        }

        self.brk = requested;
        Ok(requested)
    }

    pub fn mmap(&mut self, hint: usize, len: usize, flags: u64) -> Result<usize> {
        if len == 0 {
            return Err(MemoryError::InvalidPageCount { pages: 0 });
        }

        let len_aligned = align_up(len, PAGE_SIZE).ok_or(MemoryError::OutOfMemory)?;
        let brk_limit = align_up(self.brk, PAGE_SIZE).ok_or(MemoryError::OutOfMemory)?;

        if flags & MAP_FIXED != 0 {
            if hint == 0 || hint % PAGE_SIZE != 0 {
                return Err(MemoryError::VirtualToPhysical { addr: hint });
            }
            let start = hint;
            let end = start
                .checked_add(len_aligned)
                .ok_or(MemoryError::OutOfMemory)?;
            if start < self.mmap_base || start < brk_limit || end > USER_MMAP_LIMIT {
                return Err(MemoryError::OutOfMemory);
            }
            if !self.range_is_unmapped(start, end)? {
                return Err(MemoryError::AlreadyMapped { addr: start });
            }
            self.map_user_range(start, end)?;
            return Ok(start);
        }

        let mut start = self.mmap_next.max(self.mmap_base);
        if hint != 0 {
            let hinted = align_up(hint, PAGE_SIZE).ok_or(MemoryError::OutOfMemory)?;
            if hinted > start {
                start = hinted;
            }
        }
        if start < brk_limit {
            start = brk_limit;
        }

        loop {
            let end = start
                .checked_add(len_aligned)
                .ok_or(MemoryError::OutOfMemory)?;
            if end > USER_MMAP_LIMIT {
                return Err(MemoryError::OutOfMemory);
            }

            if self.range_is_unmapped(start, end)? {
                self.map_user_range(start, end)?;
                self.mmap_next = end;
                return Ok(start);
            }

            start = start
                .checked_add(PAGE_SIZE)
                .ok_or(MemoryError::OutOfMemory)?;
        }
    }

    fn range_is_unmapped(&mut self, start: usize, end: usize) -> Result<bool> {
        let mut vaddr = start;
        while vaddr < end {
            let entry = self.page_table.get_if_present(VirtualAddr::new(vaddr))?;
            if entry.is_some_and(|e| e.is_present()) {
                return Ok(false);
            }
            vaddr += PAGE_SIZE;
        }
        Ok(true)
    }

    fn map_user_range(&mut self, start: usize, end: usize) -> Result<()> {
        let mut vaddr = start;
        while vaddr < end {
            self.map_user_page(vaddr)?;
            vaddr += PAGE_SIZE;
        }
        Ok(())
    }

    fn map_user_page(&mut self, vaddr: usize) -> Result<()> {
        let paddr = self.kalloc.alloc(PAGE_SIZE)?;
        if let Err(err) = self.map_user_memory(paddr, VirtualAddr::new(vaddr)) {
            self.kalloc.free(paddr)?;
            return Err(err);
        }
        Ok(())
    }
}

fn align_up(value: usize, align: usize) -> Option<usize> {
    if align == 0 || !align.is_power_of_two() {
        return None;
    }
    value.checked_add(align - 1).map(|v| v & !(align - 1))
}
