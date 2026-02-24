use core::arch::global_asm;

use crate::memory::{
    address::PhysicalAddr,
    alloc::palloc::palloc,
    constants::PAGE_SIZE,
};

const MAX_PROCESSES: usize = 8;
const PROCESS_STACK_PAGES: u64 = 1;
const NO_PROCESS: usize = usize::MAX;

pub type ProcessFn = fn();

#[repr(C)]
#[derive(Clone, Copy)]
struct Context {
    rsp: u64,
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
}

impl Context {
    const fn empty() -> Self {
        Self {
            rsp: 0,
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbx: 0,
            rbp: 0,
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
    _stack_pages: u64,
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

        let stack_base = palloc(PROCESS_STACK_PAGES);
        let stack_top = stack_base
            .to_virtual()
            .expect("process stack must be direct-map address")
            .add(PAGE_SIZE * PROCESS_STACK_PAGES);

        let initial_rsp = stack_top.as_u64() - core::mem::size_of::<u64>() as u64;
        unsafe {
            *(initial_rsp as usize as *mut u64) = process_trampoline as *const () as usize as u64;
        }

        let pid = self.next_pid;
        self.next_pid += 1;

        self.processes[slot] = Process {
            id: pid,
            state: State::Ready,
            context: Context {
                rsp: initial_rsp,
                ..Context::empty()
            },
            entry: Some(entry),
            _stack_base: stack_base,
            _stack_pages: PROCESS_STACK_PAGES,
        };

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

        self.processes[current].state = State::Exited;
        self.processes[current].entry = None;

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
        let mut i = 0;
        while i < MAX_PROCESSES {
            let idx = if current == NO_PROCESS {
                i
            } else {
                (current + i + 1) % MAX_PROCESSES
            };
            if self.processes[idx].state == State::Ready {
                return Some(idx);
            }
            i += 1;
        }
        None
    }
}

static SCHEDULER: spin::Mutex<Scheduler> = spin::Mutex::new(Scheduler::new());

global_asm!(
    r#"
    .global __context_switch
__context_switch:
    mov [rdi + 0], rsp
    mov [rdi + 8], r15
    mov [rdi + 16], r14
    mov [rdi + 24], r13
    mov [rdi + 32], r12
    mov [rdi + 40], rbx
    mov [rdi + 48], rbp

    mov rsp, [rsi + 0]
    mov r15, [rsi + 8]
    mov r14, [rsi + 16]
    mov r13, [rsi + 24]
    mov r12, [rsi + 32]
    mov rbx, [rsi + 40]
    mov rbp, [rsi + 48]
    ret
"#
);

unsafe extern "C" {
    fn __context_switch(old: *mut Context, new: *const Context);
}

#[inline(always)]
unsafe fn switch_context(old: *mut Context, new: *const Context) {
    __context_switch(old, new);
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
            switch_context(plan.old, plan.new);
        }
    }
}

pub fn run() -> ! {
    loop {
        let plan = { SCHEDULER.lock().plan_kernel_to_first() };
        match plan {
            Some(plan) => unsafe {
                switch_context(plan.old, plan.new);
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
        switch_context(plan.old, plan.new);
    }
    unreachable!("exit_current should never return");
}

pub fn current_pid() -> usize {
    let sched = SCHEDULER.lock();
    if sched.current == NO_PROCESS {
        0
    } else {
        sched.processes[sched.current].id
    }
}
