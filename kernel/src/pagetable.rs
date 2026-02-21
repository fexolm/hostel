#![allow(unused)]
use crate::{
    constants::PAGE_SIZE,
    page_alloc::{palloc, pfree},
    types::PhysicalAddr,
};

const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER_ACCESSIBLE: u64 = 1 << 2;
const WRITE_THROUGH: u64 = 1 << 3;
const NO_CACHE: u64 = 1 << 4;
const ACCESSED: u64 = 1 << 5;
const DIRTY: u64 = 1 << 6;
const HUGE_PAGE: u64 = 1 << 7;
const GLOBAL: u64 = 1 << 8;
const NO_EXECUTE: u64 = 1 << 63;

#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub fn set_table(&mut self, addr: PhysicalAddr) {
        self.0 = addr.0 | PRESENT | WRITABLE | USER_ACCESSIBLE;
    }

    pub fn set_page(&mut self, addr: PhysicalAddr) {
        self.0 = addr.0 | PRESENT | WRITABLE | USER_ACCESSIBLE | HUGE_PAGE;
    }

    pub fn is_present(&self) -> bool {
        (self.0 & PRESENT) != 0
    }

    pub fn addr(&self) -> u64 {
        self.0 & 0x000F_FFFF_FFFF_F000
    }
}

#[repr(align(4096))]
#[repr(C)]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

const PAGE_TABLES_PER_PAGE: usize = PAGE_SIZE as usize / size_of::<PageTable>();

#[derive(Clone, Copy)]
pub struct PageTableBitmap {
    bitmap: [u64; PAGE_TABLES_PER_PAGE / 64],
}

impl PageTableBitmap {
    pub fn new() -> Self {
        Self {
            bitmap: [0; PAGE_TABLES_PER_PAGE / 64],
        }
    }

    pub fn alloc(&mut self) -> Option<usize> {
        for (i, word) in self.bitmap.iter_mut().enumerate() {
            let free_bits = !*word;

            if free_bits != 0 {
                let j = free_bits.trailing_zeros() as usize;

                *word |= 1 << j;
                return Some(i * 64 + j);
            }
        }
        None
    }

    pub fn free(&mut self, index: usize) {
        let word_index = index / 64;
        let bit_index = index % 64;
        self.bitmap[word_index] &= !(1 << bit_index);
    }
}

pub struct PageTableAlloc {
    pages: [PhysicalAddr; 64],
    bitmap: [PageTableBitmap; 64],
    count: usize,
}

impl PageTableAlloc {
    pub fn new() -> Self {
        Self {
            pages: [PhysicalAddr(0); 64],
            bitmap: [PageTableBitmap::new(); 64],
            count: 0,
        }
    }

    pub fn alloc(&mut self) -> Option<u64> {
        for i in 0..self.count {
            if let Some(index) = self.bitmap[i].alloc() {
                return Some(self.pages[i].0 + (index as u64 * size_of::<PageTable>() as u64));
            }
        }
        if self.count < self.pages.len() {
            let page = palloc();
            self.pages[self.count] = page;
            self.count += 1;
            return self.alloc();
        }
        panic!("Out of page table memory");
    }

    pub fn free(&mut self, addr: u64) {
        for i in 0..self.count {
            if addr >= self.pages[i].0 && addr < self.pages[i].0 + PAGE_SIZE {
                let index = ((addr - self.pages[i].0) / size_of::<PageTable>() as u64) as usize;
                self.bitmap[i].free(index);
                if self.bitmap[i].bitmap.iter().all(|&b| b == 0) {
                    pfree(self.pages[i]);

                    self.pages.swap(i, self.count - 1);
                    self.bitmap.swap(i, self.count - 1);
                    self.count -= 1;
                }
                return;
            }
        }
        panic!("Invalid page table address");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::zeroed;

    fn make_dummy_kernel() -> &'static Kernel {
        let boxed: Box<Kernel> = Box::new(unsafe { zeroed() });
        Box::leak(boxed)
    }

    #[test]
    fn bitmap_alloc_free_cycle() {
        let mut bm = PageTableBitmap::new();
        let cap = PAGE_TABLES_PER_PAGE;

        // fresh bitmap contains no bits set
        for &w in bm.bitmap.iter() {
            assert_eq!(w, 0);
        }

        // allocate every entry we can
        let mut allocated = Vec::new();
        for _ in 0..cap {
            let idx = bm.alloc().expect("should be able to allocate");
            allocated.push(idx);
        }
        assert!(
            bm.alloc().is_none(),
            "no more slots after capacity is reached"
        );

        // free them all
        for &i in &allocated {
            bm.free(i);
        }

        // and make sure we can allocate the same sequence again
        for &expected in &allocated {
            let idx = bm.alloc().expect("re‑allocation should succeed");
            assert_eq!(idx, expected);
        }
        assert!(bm.alloc().is_none());
    }

    #[test]
    fn pagetablealloc_alloc_and_free() {
        let k = make_dummy_kernel();

        let mut alloc = PageTableAlloc::new();

        // pretend we already have one page and don't touch the kernel
        alloc.pages[0] = PhysicalAddr(0x1000);
        alloc.count = 1;

        let addr1 = alloc.alloc().unwrap();
        assert_eq!(addr1, 0x1000);

        let step = core::mem::size_of::<PageTable>() as u64;
        let addr2 = alloc.alloc().unwrap();
        assert_eq!(addr2, 0x1000 + step);

        // free the first slot; page is not empty, so kernel.free() is never
        // invoked.
        alloc.free(addr1);

        // the freed slot should be reused
        let addr3 = alloc.alloc().unwrap();
        assert_eq!(addr3, addr1);
    }
}
