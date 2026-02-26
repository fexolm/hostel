#![no_std]
#![no_main]

use kernel::{process, syscall};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    kernel::console::init();
    syscall::init();
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
    let mut i = 0;
    while i < 5 {
        kernel::println!("task A (pid={}): tick {}", syscall::getpid(), i);
        i += 1;
        let _ = syscall::sched_yield();
    }
    let _ = syscall::write(1, b"task A: done via SYS_write\n");
}

fn task_b() {
    let mut i = 0;
    while i < 5 {
        kernel::println!("task B (pid={}): tick {}", syscall::getpid(), i);
        i += 1;
        let _ = syscall::sched_yield();
    }
    let _ = syscall::write(1, b"task B: done via SYS_write\n");
}
