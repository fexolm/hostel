#![no_std]
#![no_main]

use kernel::process;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    kernel::console::init();
    kernel::println!("kernel: boot");
    let p1 = process::spawn(task_a);
    let p2 = process::spawn(task_b);
    kernel::println!("kernel: spawned pid={} pid={}", p1, p2);
    process::run()
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

fn task_a() {
    let mut i = 0u64;
    while i < 5 {
        kernel::println!("task A (pid={}): tick {}", process::current_pid(), i);
        i += 1;
        process::yield_now();
    }
    kernel::println!("task A: done");
}

fn task_b() {
    let mut i = 0u64;
    while i < 5 {
        kernel::println!("task B (pid={}): tick {}", process::current_pid(), i);
        i += 1;
        process::yield_now();
    }
    kernel::println!("task B: done");
}
