#![cfg_attr(not(test), no_std)]

pub mod error;
pub mod memory;
pub mod process;
pub mod vmm;

pub struct Kernel {
    pub pagetable_alloc: spin::Mutex<memory::pagetable::PageTableAlloc>,
}

impl Kernel {
    pub fn new() -> Self {
        Self {
            pagetable_alloc: spin::Mutex::new(memory::pagetable::PageTableAlloc::new()),
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
