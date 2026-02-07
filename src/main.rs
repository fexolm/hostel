use std::{env, fs};

fn main() {
    let len = env::args().len();

    if len != 2 {
        println!("usage: hostel <path to binary>");
        return;
    }

    let path = env::args().nth(1).expect("usage: hostel <elf>");
    let buffer = fs::read(&path).unwrap();

    let result = hostel::analyze(&buffer).expect("analysis failed");
    let text_syscall_count = result
        .text_syscalls
        .iter()
        .filter(|sec| !sec.syscalls.is_empty())
        .count();
    for (i, sec) in result.text_syscalls.iter().filter(|sec| !sec.syscalls.is_empty()).enumerate() {
        println!("{}: {} at 0x{:x} with syscalls: {:?}", i, sec.name, sec.virtual_addr, sec.syscalls);
    }
    let dynamic_syscall_count = result
        .dyn_syscalls
        .iter()
        .filter(|s| s.name.contains("syscall"))
        .count();
    for (i, s) in result.dyn_syscalls.iter().filter(|s| s.name.contains("syscall")).enumerate() {
        println!("{}: {} at 0x{:x}", i, s.name, s.virtual_addr);
    }
    println!("Dynamic syscalls found: {}, Text syscalls found: {}", dynamic_syscall_count, text_syscall_count);
}
