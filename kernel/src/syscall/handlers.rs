use core::arch::{asm, global_asm};

use crate::{console, memory::errors::MemoryError, process};

use super::{
    MAP_ANONYMOUS, MAP_PRIVATE, MAP_SHARED, SYS_BRK, SYS_EXIT, SYS_EXIT_GROUP, SYS_GETPID,
    SYS_MMAP, SYS_SCHED_YIELD, SYS_WRITE,
};

const STDOUT_FD: u64 = 1;
const STDERR_FD: u64 = 2;

const EBADF: i64 = 9;
const EFAULT: i64 = 14;
const EINVAL: i64 = 22;
const ENOMEM: i64 = 12;
const ENOSYS: i64 = 38;

const IA32_STAR: u32 = 0xC000_0081;
const IA32_LSTAR: u32 = 0xC000_0082;
const IA32_FMASK: u32 = 0xC000_0084;
const IA32_EFER: u32 = 0xC000_0080;
const EFER_SCE: u64 = 1 << 0;

// These selectors match VM x86 setup in src/vm/x64.rs.
const KERNEL_CS_SELECTOR: u64 = 0x8;
const USER_CS_SELECTOR: u64 = 0x1b;

#[inline]
const fn errno(code: i64) -> u64 {
    (-code) as u64
}

global_asm!(
    r#"
    .global __syscall_entry
__syscall_entry:
    // syscall saved return RIP -> RCX, old RFLAGS -> R11.
    push rcx
    push r11

    // Save original syscall argument registers.
    push r9
    push r8
    push rdx
    push rsi
    push rdi

    // Map Linux syscall ABI (rax,rdi,rsi,rdx,r10,r8,r9)
    // to SysV call ABI for __syscall_dispatch(nr,a0,a1,a2,a3,a4,a5).
    mov rdi, rax
    mov rsi, [rsp + 0]
    mov rdx, [rsp + 8]
    mov rcx, [rsp + 16]
    mov r8, r10
    mov r9, [rsp + 24]

    // 7th argument (a5) goes on stack for SysV.
    mov rax, [rsp + 32]
    sub rsp, 8
    mov [rsp], rax
    call __syscall_dispatch
    add rsp, 8

    // Drop saved args and restore return context.
    add rsp, 40
    pop r11
    pop rcx

    // Return to the original CPL0 caller without SYSRET.
    push r11
    popfq
    jmp rcx
"#
);

unsafe extern "C" {
    fn __syscall_entry();
}

pub(super) fn install() {
    let mut efer = rdmsr(IA32_EFER);
    efer |= EFER_SCE;
    wrmsr(IA32_EFER, efer);

    // STAR layout for SYSCALL/SYSRET. We only use SYSCALL path in ring0.
    let star = (KERNEL_CS_SELECTOR << 32) | (USER_CS_SELECTOR << 48);
    wrmsr(IA32_STAR, star);
    wrmsr(IA32_LSTAR, __syscall_entry as *const () as usize as u64);
    wrmsr(IA32_FMASK, 0);
}

#[unsafe(no_mangle)]
extern "C" fn __syscall_dispatch(
    nr: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
) -> u64 {
    match nr {
        SYS_WRITE => sys_write(arg0, arg1, arg2),
        SYS_BRK => sys_brk(arg0),
        SYS_MMAP => sys_mmap(arg0, arg1, arg2, arg3, arg4 as i64, arg5),
        SYS_GETPID => process::current_pid(crate::active_kernel()) as u64,
        SYS_SCHED_YIELD => {
            process::yield_now(crate::active_kernel());
            0
        }
        SYS_EXIT | SYS_EXIT_GROUP => {
            let _status = arg0 as i32;
            process::terminate_current(crate::active_kernel())
        }
        _ => errno(ENOSYS),
    }
}

fn sys_write(fd: u64, ptr: u64, len: u64) -> u64 {
    if fd != STDOUT_FD && fd != STDERR_FD {
        return errno(EBADF);
    }
    if len == 0 {
        return 0;
    }
    if ptr == 0 {
        return errno(EFAULT);
    }

    let Ok(len) = usize::try_from(len) else {
        return errno(EINVAL);
    };

    let bytes = unsafe { core::slice::from_raw_parts(ptr as *const u8, len) };
    console::write_bytes(bytes);
    len as u64
}

fn sys_brk(addr: u64) -> u64 {
    match process::brk(crate::active_kernel(), addr as usize) {
        Ok(cur) => cur as u64,
        Err(err) => errno(memory_errno(err)),
    }
}

fn sys_mmap(addr: u64, len: u64, _prot: u64, flags: u64, fd: i64, offset: u64) -> u64 {
    let Ok(len) = usize::try_from(len) else {
        return errno(EINVAL);
    };
    if len == 0 {
        return errno(EINVAL);
    }
    if offset != 0 {
        return errno(EINVAL);
    }

    let sharing = flags & (MAP_PRIVATE | MAP_SHARED);
    if sharing == 0 {
        return errno(EINVAL);
    }
    if (flags & MAP_ANONYMOUS) == 0 {
        return errno(ENOSYS);
    }
    if fd != -1 {
        return errno(EINVAL);
    }

    match process::mmap(crate::active_kernel(), addr as usize, len, flags) {
        Ok(mapped) => mapped as u64,
        Err(err) => errno(memory_errno(err)),
    }
}

const fn memory_errno(err: MemoryError) -> i64 {
    match err {
        MemoryError::OutOfMemory | MemoryError::TooManyLargeAllocations => ENOMEM,
        MemoryError::AlreadyMapped { .. } => ENOMEM,
        _ => EINVAL,
    }
}

#[inline]
fn wrmsr(msr: u32, value: u64) {
    let lo = value as u32;
    let hi = (value >> 32) as u32;
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") lo,
            in("edx") hi,
            options(nostack, preserves_flags),
        );
    }
}

#[inline]
fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") lo,
            out("edx") hi,
            options(nostack, preserves_flags),
        );
    }
    ((hi as u64) << 32) | lo as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_syscall_returns_enosys() {
        assert_eq!(__syscall_dispatch(0xdead, 0, 0, 0, 0, 0, 0) as i64, -ENOSYS);
    }

    #[test]
    fn write_rejects_unknown_fd() {
        assert_eq!(
            __syscall_dispatch(SYS_WRITE, 7, 0, 0, 0, 0, 0) as i64,
            -EBADF
        );
    }

    #[test]
    fn write_rejects_null_pointer_for_non_zero_len() {
        assert_eq!(
            __syscall_dispatch(SYS_WRITE, 1, 0, 1, 0, 0, 0) as i64,
            -EFAULT
        );
    }
}
