#![no_std]
#![no_main]

use kernel::{page_alloc::palloc, Kernel};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let _ = palloc();
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
    // let kernel = Kernel::new();
    // kernel.run()
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
