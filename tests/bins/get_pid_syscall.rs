use std::arch::asm;

fn get_pid() -> u64 {
    let pid: u64;
    unsafe {
        asm!(
            "syscall",
            in("rax") 39,       // number of sys_getpid
            lateout("rax") pid, // pid
            out("rcx") _, out("r11") _,
        );
    }
    pid
}

fn main() {
    let pid = get_pid();
    println!("PID from inline syscall = {}", pid);
}
