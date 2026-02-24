#![no_std]
#![no_main]

use kernel::memory::alloc::palloc::palloc;

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    let _ = palloc(1);
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
