#![cfg_attr(not(test), no_std)]

pub mod constants;
pub mod page_alloc;
pub mod pagetable;
pub mod process;
pub mod types;
pub mod vmm;

use page_alloc::PageAllocator;

pub struct Kernel {
    pub page_alloc: spin::Mutex<PageAllocator>,
    pub pagetable_alloc: spin::Mutex<pagetable::PageTableAlloc>,
}

impl Kernel {
    pub fn new() -> Self {
        Self {
            page_alloc: spin::Mutex::new(PageAllocator::new()),
            pagetable_alloc: spin::Mutex::new(pagetable::PageTableAlloc::new()),
        }
    }

    pub fn run(&self) -> ! {
        loop {
            unsafe {
                core::arch::asm!("hlt");
            }
        }
    }
}
