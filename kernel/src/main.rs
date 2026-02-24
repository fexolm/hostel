#![no_std]
#![no_main]

use kernel::memory::alloc::palloc::palloc;

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    kernel::console::init();
    kernel::println!("kernel: boot");

    let _ = palloc(1);
    kernel::println!("kernel: palloc(1) ok");

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
fn panic(info: &core::panic::PanicInfo) -> ! {
    kernel::console::init();
    kernel::println!("kernel panic: {}", info);

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
