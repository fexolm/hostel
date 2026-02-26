use core::arch::asm;

mod handlers;

pub const SYS_WRITE: u64 = 1;
pub const SYS_SCHED_YIELD: u64 = 24;
pub const SYS_GETPID: u64 = 39;
pub const SYS_EXIT: u64 = 60;
pub const SYS_EXIT_GROUP: u64 = 231;

pub fn init() {
    handlers::install();
}

#[inline]
pub fn syscall6(nr: u64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    let ret: i64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr as i64 => ret,
            in("rdi") a0 as i64,
            in("rsi") a1 as i64,
            in("rdx") a2 as i64,
            in("r10") a3 as i64,
            in("r8") a4 as i64,
            in("r9") a5 as i64,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack),
        );
    }
    ret
}

pub fn write(fd: u64, buf: &[u8]) -> i64 {
    syscall6(SYS_WRITE, fd, buf.as_ptr() as u64, buf.len() as u64, 0, 0, 0)
}

pub fn getpid() -> i64 {
    syscall6(SYS_GETPID, 0, 0, 0, 0, 0, 0)
}

pub fn sched_yield() -> i64 {
    syscall6(SYS_SCHED_YIELD, 0, 0, 0, 0, 0, 0)
}

pub fn exit(status: i32) -> ! {
    let _ = syscall6(SYS_EXIT, status as u64, 0, 0, 0, 0, 0);
    unreachable!("sys_exit should never return");
}
