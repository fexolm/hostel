use core::arch::global_asm;
use core::ptr::{null, null_mut};

use crate::Kernel;
use crate::memory::{
    address::PhysicalAddr, constants::PAGE_SIZE, errors::Result as MemoryResult, vmm::Vmm,
};
use crate::scheduler::{Context, ExitPlan, MAX_PROCESSES, Scheduler, SwitchPlan};

const PROCESS_STACK_PAGES: usize = 1;

pub type ProcessFn = fn();

struct Process<'i> {
    vmm: Vmm<'i>,
    stack_base: PhysicalAddr,
    stack_pages: usize,
}

pub struct ProcessState<'i> {
    inner: spin::Mutex<ProcessStateInner<'i>>,
}

struct ProcessStateInner<'i> {
    scheduler: Scheduler,
    processes: [Option<Process<'i>>; MAX_PROCESSES],
}

impl<'i> ProcessState<'i> {
    pub fn new() -> Self {
        Self {
            inner: spin::Mutex::new(ProcessStateInner {
                scheduler: Scheduler::new(),
                processes: core::array::from_fn(|_| None),
            }),
        }
    }

    fn spawn(&self, kernel: &Kernel<'i>, entry: ProcessFn) -> usize {
        let vmm = Vmm::new(kernel.page_table, kernel.kalloc).expect("create vmm");
        let stack_base = kernel
            .palloc
            .alloc(PROCESS_STACK_PAGES)
            .expect("allocate process stack");

        let stack_top = stack_base.to_virtual().add(PAGE_SIZE * PROCESS_STACK_PAGES);

        // Keep SysV stack alignment for first frame (entry sees RSP % 16 == 8).
        let initial_rsp = stack_top.as_usize() - 2 * core::mem::size_of::<u64>();
        unsafe {
            *(initial_rsp as *mut u64) = process_trampoline as *const () as usize as u64;
        }

        let mut inner = self.inner.lock();
        let spawn = inner
            .scheduler
            .spawn(entry, initial_rsp as u64, vmm.root().as_u64());
        inner.processes[spawn.slot] = Some(Process {
            vmm,
            stack_base,
            stack_pages: PROCESS_STACK_PAGES,
        });
        spawn.pid
    }

    fn plan_kernel_to_first(&self) -> Option<SwitchPlan> {
        self.inner.lock().scheduler.plan_kernel_to_first()
    }

    fn plan_yield(&self) -> Option<SwitchPlan> {
        self.inner.lock().scheduler.plan_yield()
    }

    fn plan_exit_current(&self) -> (SwitchPlan, Process<'i>) {
        let mut inner = self.inner.lock();
        let ExitPlan {
            switch,
            exited_slot,
        } = inner.scheduler.plan_exit_current();
        let process = inner.processes[exited_slot]
            .take()
            .expect("exited process slot must be populated");
        (switch, process)
    }

    fn current_entry(&self) -> ProcessFn {
        self.inner.lock().scheduler.current_entry()
    }

    fn current_pid(&self) -> usize {
        self.inner.lock().scheduler.current_pid()
    }

    fn has_pid(&self, pid: usize) -> bool {
        self.inner.lock().scheduler.has_pid(pid)
    }

    fn with_current_process_mut<T>(
        &self,
        f: impl FnOnce(&mut Process<'i>) -> MemoryResult<T>,
    ) -> MemoryResult<T> {
        let mut inner = self.inner.lock();
        let current = inner.scheduler.current_slot().expect("no running process");
        let process = inner.processes[current]
            .as_mut()
            .expect("running process slot must be populated");
        f(process)
    }
}

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
    let kernel = crate::active_kernel();
    let entry = kernel.process.current_entry();
    entry();
    terminate_current(kernel);
}

pub fn spawn(kernel: &Kernel<'_>, entry: ProcessFn) -> usize {
    kernel.process.spawn(kernel, entry)
}

pub fn yield_now(kernel: &Kernel<'_>) {
    let plan = kernel.process.plan_yield();
    if let Some(plan) = plan {
        unsafe {
            switch_context(plan);
        }
    }
}

pub fn run(kernel: &Kernel<'_>) -> ! {
    loop {
        match kernel.process.plan_kernel_to_first() {
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

fn exit_current(kernel: &Kernel<'_>) -> ! {
    let (switch, process) = kernel.process.plan_exit_current();
    cleanup_process(kernel, process);

    unsafe {
        switch_context(switch);
    }
    unreachable!("exit_current should never return");
}

fn cleanup_process(kernel: &Kernel<'_>, process: Process<'_>) {
    drop(process.vmm);

    for page in 0..process.stack_pages {
        kernel
            .palloc
            .free(process.stack_base.add(PAGE_SIZE * page))
            .expect("free process stack");
    }
}

pub fn terminate_current(kernel: &Kernel<'_>) -> ! {
    exit_current(kernel)
}

pub fn current_pid(kernel: &Kernel<'_>) -> usize {
    kernel.process.current_pid()
}

pub fn has_pid(kernel: &Kernel<'_>, pid: usize) -> bool {
    kernel.process.has_pid(pid)
}

pub fn brk(kernel: &Kernel<'_>, requested: usize) -> MemoryResult<usize> {
    kernel
        .process
        .with_current_process_mut(|proc| proc.vmm.brk(requested))
}

pub fn mmap(kernel: &Kernel<'_>, hint: usize, len: usize, flags: u64) -> MemoryResult<usize> {
    kernel
        .process
        .with_current_process_mut(|proc| proc.vmm.mmap(hint, len, flags))
}
