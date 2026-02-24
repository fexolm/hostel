#![cfg_attr(not(test), no_std)]

pub mod console;
pub mod error;
pub mod memory;
pub mod process;

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

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        $crate::console::_print(core::format_args!($($arg)*));
    });
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(concat!($fmt, "\n"), $($arg)*));
}
