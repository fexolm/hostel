use core::arch::global_asm;
use core::ptr::{null, null_mut};

use crate::memory::{
    address::PhysicalAddr,
    alloc::palloc::{palloc, pfree},
    constants::PAGE_SIZE,
    errors::Result as MemoryResult,
    pagetable::PageTable,
    vmm::Vmm,
};
use crate::scheduler::{self, Context, ExitPlan, SwitchPlan, MAX_PROCESSES};

const PROCESS_STACK_PAGES: usize = 1;

pub type ProcessFn = fn();

#[derive(Clone, Copy)]
struct Process {
    vmm: Vmm,
    stack_base: PhysicalAddr,
    stack_pages: usize,
}

impl Process {
    const fn empty() -> Self {
        Self {
            vmm: Vmm::empty(),
            stack_base: PhysicalAddr::new(0),
            stack_pages: 0,
        }
    }
}

static PROCESSES: spin::Mutex<[Process; MAX_PROCESSES]> =
    spin::Mutex::new([Process::empty(); MAX_PROCESSES]);

#[unsafe(no_mangle)]
static mut SWITCH_OLD_CTX: *mut Context = null_mut();
#[unsafe(no_mangle)]
static mut SWITCH_NEW_CTX: *const Context = null();

global_asm!(
    r#"
    .global __context_switch
__context_switch:
    push rax
    push rdx

    mov rax, [rip + SWITCH_OLD_CTX]

    mov [rax + 8], rbx
    mov [rax + 16], rcx
    mov [rax + 32], rsi
    mov [rax + 40], rdi
    mov [rax + 48], rbp
    mov [rax + 56], r8
    mov [rax + 64], r9
    mov [rax + 72], r10
    mov [rax + 80], r11
    mov [rax + 88], r12
    mov [rax + 96], r13
    mov [rax + 104], r14
    mov [rax + 112], r15

    lea rdx, [rsp + 16]
    mov [rax + 120], rdx

    pushfq
    pop qword ptr [rax + 128]

    mov rdx, cr3
    mov [rax + 136], rdx

    fxsave64 [rax + 144]

    mov rdx, [rsp]
    mov [rax + 24], rdx
    mov rdx, [rsp + 8]
    mov [rax + 0], rdx

    add rsp, 16

    mov r8, [rip + SWITCH_NEW_CTX]

    mov rcx, [r8 + 136]
    mov cr3, rcx

    fxrstor64 [r8 + 144]

    mov r15, [r8 + 112]
    mov r14, [r8 + 104]
    mov r13, [r8 + 96]
    mov r12, [r8 + 88]
    mov r11, [r8 + 80]
    mov r10, [r8 + 72]
    mov r9, [r8 + 64]
    mov rdi, [r8 + 40]
    mov rsi, [r8 + 32]
    mov rbp, [r8 + 48]
    mov rbx, [r8 + 8]

    mov rsp, [r8 + 120]

    push qword ptr [r8 + 128]
    popfq

    mov rdx, [r8 + 24]
    mov rcx, [r8 + 16]
    mov rax, [r8 + 0]
    mov r8, [r8 + 56]

    ret
"#
);

unsafe extern "C" {
    fn __context_switch();
}

#[inline(always)]
unsafe fn switch_context(plan: SwitchPlan) {
    unsafe {
        SWITCH_OLD_CTX = plan.old;
    }
    unsafe {
        SWITCH_NEW_CTX = plan.new;
    }
    unsafe {
        __context_switch();
    }
}

extern "C" fn process_trampoline() -> ! {
    let entry = scheduler::current_entry();
    entry();
    exit_current();
}

pub fn spawn(entry: ProcessFn) -> usize {
    let pml4_base = PageTable::alloc_user_pml4().expect("allocate user pml4");
    let stack_base = palloc(PROCESS_STACK_PAGES).expect("allocate process stack");
    let stack_top = stack_base
        .to_virtual()
        .expect("process stack must be direct-map address")
        .add(PAGE_SIZE * PROCESS_STACK_PAGES);

    // Keep SysV stack alignment for first frame (entry sees RSP % 16 == 8).
    let initial_rsp = stack_top.as_usize() - 2 * core::mem::size_of::<u64>();
    unsafe {
        *(initial_rsp as *mut u64) = process_trampoline as *const () as usize as u64;
    }

    let spawn = scheduler::spawn(entry, initial_rsp as u64, pml4_base.as_u64());
    PROCESSES.lock()[spawn.slot] = Process {
        vmm: Vmm::new(pml4_base),
        stack_base,
        stack_pages: PROCESS_STACK_PAGES,
    };

    spawn.pid
}

pub fn yield_now() {
    let plan = scheduler::plan_yield();
    if let Some(plan) = plan {
        unsafe {
            switch_context(plan);
        }
    }
}

pub fn run() -> ! {
    loop {
        match scheduler::plan_kernel_to_first() {
            Some(plan) => unsafe {
                switch_context(plan);
            },
            None => loop {
                unsafe {
                    core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
                }
            },
        }
    }
}

fn exit_current() -> ! {
    let ExitPlan { switch, exited_slot } = scheduler::plan_exit_current();

    let process = {
        let mut processes = PROCESSES.lock();
        let process = processes[exited_slot];
        processes[exited_slot] = Process::empty();
        process
    };

    cleanup_process(process);

    unsafe {
        switch_context(switch);
    }
    unreachable!("exit_current should never return");
}

fn cleanup_process(process: Process) {
    let user_root = process.vmm.root();
    if user_root.as_u64() != 0 {
        let root = PageTable::from_paddr_mut(user_root).expect("valid user root page table");
        root.free().expect("free user page table tree");
    }

    for page in 0..process.stack_pages {
        pfree(process.stack_base.add(PAGE_SIZE * page)).expect("free process stack");
    }
}

pub fn terminate_current() -> ! {
    exit_current()
}

pub fn current_pid() -> usize {
    scheduler::current_pid()
}

fn with_current_process_mut<T>(f: impl FnOnce(&mut Process) -> MemoryResult<T>) -> MemoryResult<T> {
    let mut processes = PROCESSES.lock();
    let current = scheduler::current_slot().expect("no running process");
    f(&mut processes[current])
}

pub fn brk(requested: usize) -> MemoryResult<usize> {
    with_current_process_mut(|proc| proc.vmm.brk(requested))
}

pub fn mmap(hint: usize, len: usize, flags: u64) -> MemoryResult<usize> {
    with_current_process_mut(|proc| proc.vmm.mmap(hint, len, flags))
}
