/// Entry point executed by the guest VM. When running under KVM this function
/// will be invoked <strong>after</strong> the loader has set up a 64-bit
/// environment (paging/long mode).
#[no_mangle]
pub extern "C" fn _start() -> ! {
    // write a magic value into guest memory at the automatically reserved
    // `DATA_ADDR` (0x4000) from the loader so that the unit tests can observe
    // that the guest actually executed our code. This mirrors what the
    // existing `vm` unit test does.
    unsafe {
        core::ptr::write_volatile(0x4000 as *mut u64, 0x1234_5678_9ABC_DEF0);
    }

    // halt forever
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

// required by `#![no_std]` because otherwise the linker expects a symbol
// for unwinding.  Simply hang the CPU.
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
