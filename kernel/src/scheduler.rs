use core::arch::asm;

pub(crate) const MAX_PROCESSES: usize = 8;
const NO_PROCESS: usize = usize::MAX;

pub type ProcessFn = fn();

#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct Context {
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
}

impl Process {
    const fn empty() -> Self {
        Self {
            id: 0,
            state: State::Empty,
            context: Context::empty(),
            entry: None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SwitchPlan {
    pub old: *mut Context,
    pub new: *const Context,
}

pub struct SpawnPlan {
    pub slot: usize,
    pub pid: usize,
}

pub struct ExitPlan {
    pub switch: SwitchPlan,
    pub exited_slot: usize,
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

    fn spawn(&mut self, entry: ProcessFn, rsp: u64, cr3: u64) -> SpawnPlan {
        let slot = self
            .processes
            .iter()
            .position(|proc| proc.state == State::Empty || proc.state == State::Exited)
            .expect("process table is full");

        let pid = self.next_pid;
        self.next_pid += 1;

        self.processes[slot] = Process {
            id: pid,
            state: State::Ready,
            context: Context {
                rsp,
                cr3,
                ..Context::empty()
            },
            entry: Some(entry),
        };

        save_current_fxstate(&mut self.processes[slot].context);
        SpawnPlan { slot, pid }
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

    fn plan_exit_current(&mut self) -> ExitPlan {
        let current = self.current;
        assert!(current != NO_PROCESS, "no running process to exit");

        self.processes[current].state = State::Exited;
        self.processes[current].entry = None;
        self.processes[current].context.cr3 = 0;

        let switch = if let Some(next) = self.find_next_ready(current) {
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
        };

        ExitPlan {
            switch,
            exited_slot: current,
        }
    }

    fn current_entry(&self) -> ProcessFn {
        assert!(self.current != NO_PROCESS, "no running process");
        self.processes[self.current]
            .entry
            .expect("running process has no entry function")
    }

    fn current_pid(&self) -> usize {
        if self.current == NO_PROCESS {
            0
        } else {
            self.processes[self.current].id
        }
    }

    fn current_slot(&self) -> Option<usize> {
        if self.current == NO_PROCESS {
            None
        } else {
            Some(self.current)
        }
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

pub(crate) fn spawn(entry: ProcessFn, rsp: u64, cr3: u64) -> SpawnPlan {
    SCHEDULER.lock().spawn(entry, rsp, cr3)
}

pub(crate) fn plan_kernel_to_first() -> Option<SwitchPlan> {
    SCHEDULER.lock().plan_kernel_to_first()
}

pub(crate) fn plan_yield() -> Option<SwitchPlan> {
    SCHEDULER.lock().plan_yield()
}

pub(crate) fn plan_exit_current() -> ExitPlan {
    SCHEDULER.lock().plan_exit_current()
}

pub(crate) fn current_entry() -> ProcessFn {
    SCHEDULER.lock().current_entry()
}

pub(crate) fn current_pid() -> usize {
    SCHEDULER.lock().current_pid()
}

pub(crate) fn current_slot() -> Option<usize> {
    SCHEDULER.lock().current_slot()
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
