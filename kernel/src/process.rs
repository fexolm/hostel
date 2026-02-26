use core::arch::{asm, global_asm};
use core::ptr::{null, null_mut};

use crate::memory::{
    address::PhysicalAddr,
    alloc::palloc::{palloc, pfree},
    constants::PAGE_SIZE,
    pagetable::PageTable,
};

const MAX_PROCESSES: usize = 8;
const PROCESS_STACK_PAGES: usize = 1;
const NO_PROCESS: usize = usize::MAX;
pub type ProcessFn = fn();

#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct Context {
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rsp: u64,
    rflags: u64,
    cr3: u64,
    fxstate: [u8; 512],
}

impl Context {
    const fn empty() -> Self {
        Self {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rsp: 0,
            rflags: 0x2,
            cr3: 0,
            fxstate: [0; 512],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Empty,
    Ready,
    Running,
    Exited,
}

#[derive(Clone, Copy)]
struct Process {
    id: usize,
    state: State,
    context: Context,
    entry: Option<ProcessFn>,
    _stack_base: PhysicalAddr,
    _stack_pages: usize,
}

impl Process {
    const fn empty() -> Self {
        Self {
            id: 0,
            state: State::Empty,
            context: Context::empty(),
            entry: None,
            _stack_base: PhysicalAddr::new(0),
            _stack_pages: 0,
        }
    }
}

struct SwitchPlan {
    old: *mut Context,
    new: *const Context,
}

struct Scheduler {
    kernel_context: Context,
    processes: [Process; MAX_PROCESSES],
    current: usize,
    next_pid: usize,
}

impl Scheduler {
    const fn new() -> Self {
        Self {
            kernel_context: Context::empty(),
            processes: [Process::empty(); MAX_PROCESSES],
            current: NO_PROCESS,
            next_pid: 1,
        }
    }

    fn spawn(&mut self, entry: ProcessFn) -> usize {
        let slot = self
            .processes
            .iter()
            .position(|proc| proc.state == State::Empty || proc.state == State::Exited)
            .expect("process table is full");

        let pml4_base = PageTable::alloc_user_pml4().expect("allocate user pml4");
        let stack_base = palloc(PROCESS_STACK_PAGES).expect("allocate process stack");
        let stack_top = stack_base
            .to_virtual()
            .expect("process stack must be direct-map address")
            .add(PAGE_SIZE * PROCESS_STACK_PAGES);

        let initial_rsp = stack_top.as_usize() - core::mem::size_of::<u64>();
        unsafe {
            *(initial_rsp as *mut u64) = process_trampoline as *const () as usize as u64;
        }

        let pid = self.next_pid;
        self.next_pid += 1;

        self.processes[slot] = Process {
            id: pid,
            state: State::Ready,
            context: Context {
                rsp: initial_rsp as u64,
                cr3: pml4_base.as_u64(),
                ..Context::empty()
            },
            entry: Some(entry),
            _stack_base: stack_base,
            _stack_pages: PROCESS_STACK_PAGES,
        };
        save_current_fxstate(&mut self.processes[slot].context);

        pid
    }

    fn plan_kernel_to_first(&mut self) -> Option<SwitchPlan> {
        let next = self.find_next_ready(NO_PROCESS)?;
        self.processes[next].state = State::Running;
        self.current = next;
        Some(SwitchPlan {
            old: &mut self.kernel_context as *mut Context,
            new: &self.processes[next].context as *const Context,
        })
    }

    fn plan_yield(&mut self) -> Option<SwitchPlan> {
        if self.current == NO_PROCESS {
            return self.plan_kernel_to_first();
        }

        let current = self.current;
        let next = self.find_next_ready(current)?;
        if next == current {
            return None;
        }

        if self.processes[current].state == State::Running {
            self.processes[current].state = State::Ready;
        }
        self.processes[next].state = State::Running;
        self.current = next;

        Some(SwitchPlan {
            old: &mut self.processes[current].context as *mut Context,
            new: &self.processes[next].context as *const Context,
        })
    }

    fn plan_exit_current(&mut self) -> SwitchPlan {
        let current = self.current;
        assert!(current != NO_PROCESS, "no running process to exit");
        let user_root = PhysicalAddr::new(self.processes[current].context.cr3 as usize);
        let stack_base = self.processes[current]._stack_base;
        let stack_pages = self.processes[current]._stack_pages;

        self.processes[current].state = State::Exited;
        self.processes[current].entry = None;
        self.processes[current].context.cr3 = 0;
        self.processes[current]._stack_base = PhysicalAddr::new(0);
        self.processes[current]._stack_pages = 0;

        if user_root.as_u64() != 0 {
            let root = PageTable::from_paddr_mut(user_root).expect("valid user root page table");
            root.free().expect("free user page table tree");
        }

        for page in 0..stack_pages {
            pfree(stack_base.add(PAGE_SIZE * page)).expect("free process stack");
        }

        if let Some(next) = self.find_next_ready(current) {
            self.processes[next].state = State::Running;
            self.current = next;
            SwitchPlan {
                old: &mut self.processes[current].context as *mut Context,
                new: &self.processes[next].context as *const Context,
            }
        } else {
            self.current = NO_PROCESS;
            SwitchPlan {
                old: &mut self.processes[current].context as *mut Context,
                new: &self.kernel_context as *const Context,
            }
        }
    }

    fn current_entry(&self) -> ProcessFn {
        assert!(self.current != NO_PROCESS, "no running process");
        self.processes[self.current]
            .entry
            .expect("running process has no entry function")
    }

    fn find_next_ready(&self, current: usize) -> Option<usize> {
        for i in 0..MAX_PROCESSES {
            let idx = if current == NO_PROCESS {
                i
            } else {
                (current + i + 1) % MAX_PROCESSES
            };
            if self.processes[idx].state == State::Ready {
                return Some(idx);
            }
        }
        None
    }
}

static SCHEDULER: spin::Mutex<Scheduler> = spin::Mutex::new(Scheduler::new());
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

fn save_current_fxstate(context: &mut Context) {
    let fx_ptr = context.fxstate.as_mut_ptr();
    unsafe {
        asm!(
            "fxsave64 [{}]",
            in(reg) fx_ptr,
            options(nostack),
        );
    }
}

extern "C" fn process_trampoline() -> ! {
    let entry = { SCHEDULER.lock().current_entry() };
    entry();
    exit_current();
}

pub fn spawn(entry: ProcessFn) -> usize {
    SCHEDULER.lock().spawn(entry)
}

pub fn yield_now() {
    let plan = { SCHEDULER.lock().plan_yield() };
    if let Some(plan) = plan {
        unsafe {
            switch_context(plan);
        }
    }
}

pub fn run() -> ! {
    loop {
        let plan = { SCHEDULER.lock().plan_kernel_to_first() };
        match plan {
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
    let plan = { SCHEDULER.lock().plan_exit_current() };
    unsafe {
        switch_context(plan);
    }
    unreachable!("exit_current should never return");
}

pub fn terminate_current() -> ! {
    exit_current()
}

pub fn current_pid() -> usize {
    let sched = SCHEDULER.lock();
    if sched.current == NO_PROCESS {
        0
    } else {
        sched.processes[sched.current].id
    }
}
