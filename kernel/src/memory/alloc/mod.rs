pub mod kmalloc;
pub mod palloc;

#[cfg(test)]
pub(crate) static ALLOC_TEST_LOCK: spin::Mutex<()> = spin::Mutex::new(());
