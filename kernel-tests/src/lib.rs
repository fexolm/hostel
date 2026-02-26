#![no_std]

extern crate self as kernel_tests;

mod api;
mod test_process;

pub use kernel_tests_macros::KernelTest;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TestName {
    ptr: *const u8,
    len: usize,
}

impl TestName {
    pub const fn new(name: &'static str) -> Self {
        Self {
            ptr: name.as_ptr(),
            len: name.len(),
        }
    }

    pub fn as_str(self) -> &'static str {
        unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(self.ptr, self.len)) }
    }
}

unsafe impl Sync for TestName {}

#[repr(C)]
pub struct TestRegistration {
    pub name: TestName,
    pub run: extern "C" fn(),
}

#[cfg(target_os = "none")]
unsafe extern "C" {
    static __start_kernel_tests: TestRegistration;
    static __stop_kernel_tests: TestRegistration;
}

pub fn run() -> ! {
    for test in registered_tests() {
        let _ = test.name.as_str();
        (test.run)();
    }
    api::signal_success()
}

fn registered_tests() -> &'static [TestRegistration] {
    #[cfg(not(target_os = "none"))]
    {
        &[]
    }

    #[cfg(target_os = "none")]
    unsafe {
        let start = core::ptr::addr_of!(__start_kernel_tests);
        let stop = core::ptr::addr_of!(__stop_kernel_tests);
        let bytes = (stop as usize).saturating_sub(start as usize);
        let len = bytes / core::mem::size_of::<TestRegistration>();
        core::slice::from_raw_parts(start, len)
    }
}
