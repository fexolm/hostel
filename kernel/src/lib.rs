#![cfg_attr(not(test), no_std)]

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::memory::{
    alloc::{kmalloc::KernelAllocator, palloc::PageAllocator},
    pagetable::RootPageTable,
};

pub mod boot;
pub mod console;
pub mod error;
pub mod memory;
pub mod process;
mod scheduler;
pub mod syscall;

static ACTIVE_KERNEL: AtomicUsize = AtomicUsize::new(0);

pub struct Kernel<'i> {
    pub palloc: &'i PageAllocator,
    pub kalloc: &'i KernelAllocator<'i>,
    pub page_table: &'i RootPageTable<'i>,
    pub process: process::ProcessState<'i>,
}

impl<'i> Kernel<'i> {
    pub fn new(
        palloc: &'i PageAllocator,
        kalloc: &'i KernelAllocator<'i>,
        page_table: &'i RootPageTable<'i>,
    ) -> Self {
        Self {
            palloc,
            kalloc,
            page_table,
            process: process::ProcessState::new(),
        }
    }
}

pub fn set_active_kernel(kernel: &Kernel<'_>) {
    let ptr = kernel as *const Kernel<'_> as usize;
    ACTIVE_KERNEL.store(ptr, Ordering::SeqCst);
}

pub fn active_kernel<'i>() -> &'i Kernel<'i> {
    let ptr = ACTIVE_KERNEL.load(Ordering::SeqCst);
    assert!(ptr != 0, "active kernel is not initialized");
    unsafe { &*(ptr as *const Kernel<'i>) }
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
