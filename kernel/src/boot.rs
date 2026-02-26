use core::arch::asm;

use crate::memory::constants::RUN_FLAGS_PHYS;

pub const KERNEL_TEST_EXIT_PORT: u16 = 0xF4;
pub const KERNEL_TEST_EXIT_SUCCESS: u32 = 0x10;
pub const KERNEL_TEST_EXIT_FAILURE: u32 = 0x11;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunFlags {
    bits: u64,
}

impl RunFlags {
    const RUN_TESTS_BIT: u64 = 1 << 0;

    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn from_bits(bits: u64) -> Self {
        Self {
            bits: bits & Self::RUN_TESTS_BIT,
        }
    }

    pub const fn bits(self) -> u64 {
        self.bits
    }

    pub const fn with_run_tests(mut self, enabled: bool) -> Self {
        if enabled {
            self.bits |= Self::RUN_TESTS_BIT;
        } else {
            self.bits &= !Self::RUN_TESTS_BIT;
        }
        self
    }

    pub const fn run_tests(self) -> bool {
        (self.bits & Self::RUN_TESTS_BIT) != 0
    }
}

pub fn read_run_flags() -> RunFlags {
    let flags_addr = RUN_FLAGS_PHYS
        .to_virtual()
        .expect("run-flags physical address must be direct-map accessible");
    let raw = unsafe { core::ptr::read_volatile(flags_addr.as_ptr::<u64>() as *const u64) };
    RunFlags::from_bits(raw)
}

pub fn signal_kernel_tests_success() -> ! {
    write_test_exit_code(KERNEL_TEST_EXIT_SUCCESS);
    halt_forever()
}

pub fn signal_kernel_tests_failure() -> ! {
    write_test_exit_code(KERNEL_TEST_EXIT_FAILURE);
    halt_forever()
}

pub fn halt_forever() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

#[inline]
fn write_test_exit_code(code: u32) {
    unsafe {
        asm!(
            "out dx, eax",
            in("dx") KERNEL_TEST_EXIT_PORT,
            in("eax") code,
            options(nomem, nostack, preserves_flags),
        );
    }
}
