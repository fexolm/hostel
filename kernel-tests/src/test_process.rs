use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::api;
use kernel_tests_macros::kernel_test;

const PAGE_SIZE: usize = 2 << 20;
const MAGIC_VALUE: u64 = 0xfeed_face_cafe_beef;

static PROCESS_DONE: AtomicBool = AtomicBool::new(false);
static PROCESS_READBACK: AtomicU64 = AtomicU64::new(0);

#[kernel_test]
fn process_mmap_write_read_and_exit() {
    PROCESS_DONE.store(false, Ordering::SeqCst);
    PROCESS_READBACK.store(0, Ordering::SeqCst);

    let pid = api::spawn(process_entry);
    assert!(api::has_pid(pid), "spawned process must be active");

    api::yield_now();

    assert!(
        PROCESS_DONE.load(Ordering::SeqCst),
        "process did not reach completion point"
    );
    assert_eq!(
        PROCESS_READBACK.load(Ordering::SeqCst),
        MAGIC_VALUE,
        "process must read back the value written into mmap"
    );
    assert!(
        !api::has_pid(pid),
        "process must be removed from active scheduler list after exit"
    );
}

fn process_entry() {
    let mapped = api::mmap_anonymous(PAGE_SIZE);
    assert!(mapped > 0, "mmap failed with return value {}", mapped);

    let ptr = mapped as usize as *mut u64;
    unsafe {
        ptr.write_volatile(MAGIC_VALUE);
        let readback = ptr.read_volatile();
        PROCESS_READBACK.store(readback, Ordering::SeqCst);
    }
    PROCESS_DONE.store(true, Ordering::SeqCst);

    api::exit(0);
}
