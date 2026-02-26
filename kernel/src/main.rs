#![no_std]
#![no_main]

use kernel::{boot, process, syscall};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    kernel::console::init();
    syscall::init();
    let run_flags = kernel::boot::read_run_flags();

    if run_flags.run_tests() {
        kernel::println!("kernel: boot (integration-tests)");
        kernel_tests::run();
    }

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

    if kernel::boot::read_run_flags().run_tests() {
        kernel::boot::signal_kernel_tests_failure();
    }

    kernel::boot::halt_forever()
}

#[unsafe(no_mangle)]
extern "C" fn kt_spawn(entry: usize) -> usize {
    let entry_fn: process::ProcessFn = unsafe { core::mem::transmute(entry) };
    process::spawn(entry_fn)
}

#[unsafe(no_mangle)]
extern "C" fn kt_has_pid(pid: usize) -> bool {
    process::has_pid(pid)
}

#[unsafe(no_mangle)]
extern "C" fn kt_yield_now() {
    process::yield_now()
}

#[unsafe(no_mangle)]
extern "C" fn kt_mmap_anonymous(len: usize) -> i64 {
    syscall::mmap_anonymous(len)
}

#[unsafe(no_mangle)]
extern "C" fn kt_exit(status: i32) -> ! {
    syscall::exit(status)
}

#[unsafe(no_mangle)]
extern "C" fn kt_signal_success() -> ! {
    boot::signal_kernel_tests_success()
}

#[unsafe(no_mangle)]
extern "C" fn kt_signal_failure() -> ! {
    boot::signal_kernel_tests_failure()
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
