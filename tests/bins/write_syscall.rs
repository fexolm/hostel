use std::arch::asm;

fn write_syscall(msg: &[u8]) {
    unsafe {
        let ret: i64;
        asm!(
            "syscall",
            in("rax") 1,
            in("rdi") 1,
            in("rsi") msg.as_ptr(),
            in("rdx") msg.len(),
            lateout("rax") ret,
            out("rcx") _, out("r11") _,
        );

        assert!(ret >= 0, "write syscall failed: {}", ret);
    }
}

fn main() {
    let message = b"Hello from dynamic syscall!\n";
    write_syscall(message);
}
