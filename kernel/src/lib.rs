#![cfg_attr(not(test), no_std)]

pub mod constants;
pub mod page_alloc;
pub mod pagetable;
pub mod process;
pub mod types;
pub mod vmm;

pub struct Kernel {
    pub pagetable_alloc: spin::Mutex<pagetable::PageTableAlloc>,
}

impl Kernel {
    pub fn new() -> Self {
        Self {
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
