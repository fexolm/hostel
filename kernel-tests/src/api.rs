#[cfg(target_os = "none")]
unsafe extern "C" {
    fn kt_spawn(entry: usize) -> usize;
    fn kt_has_pid(pid: usize) -> bool;
    fn kt_yield_now();
    fn kt_mmap_anonymous(len: usize) -> i64;
    fn kt_exit(status: i32) -> !;
    fn kt_signal_success() -> !;
    fn kt_signal_failure() -> !;
}

#[cfg(not(target_os = "none"))]
unsafe extern "C" fn kt_spawn(_entry: usize) -> usize {
    panic!("kernel test API is unavailable outside kernel target");
}

#[cfg(not(target_os = "none"))]
unsafe extern "C" fn kt_has_pid(_pid: usize) -> bool {
    panic!("kernel test API is unavailable outside kernel target");
}

#[cfg(not(target_os = "none"))]
unsafe extern "C" fn kt_yield_now() {
    panic!("kernel test API is unavailable outside kernel target");
}

#[cfg(not(target_os = "none"))]
unsafe extern "C" fn kt_mmap_anonymous(_len: usize) -> i64 {
    panic!("kernel test API is unavailable outside kernel target");
}

#[cfg(not(target_os = "none"))]
unsafe extern "C" fn kt_exit(_status: i32) -> ! {
    panic!("kernel test API is unavailable outside kernel target");
}

#[cfg(not(target_os = "none"))]
unsafe extern "C" fn kt_signal_success() -> ! {
    panic!("kernel test API is unavailable outside kernel target");
}

#[cfg(not(target_os = "none"))]
unsafe extern "C" fn kt_signal_failure() -> ! {
    panic!("kernel test API is unavailable outside kernel target");
}

pub fn spawn(entry: fn()) -> usize {
    unsafe { kt_spawn(entry as usize) }
}

pub fn has_pid(pid: usize) -> bool {
    unsafe { kt_has_pid(pid) }
}

pub fn yield_now() {
    unsafe { kt_yield_now() }
}

pub fn mmap_anonymous(len: usize) -> i64 {
    unsafe { kt_mmap_anonymous(len) }
}

pub fn exit(status: i32) -> ! {
    unsafe { kt_exit(status) }
}

pub fn signal_success() -> ! {
    unsafe { kt_signal_success() }
}

#[allow(dead_code)]
pub fn signal_failure() -> ! {
    unsafe { kt_signal_failure() }
}
