use thiserror::Error as ThisError;
use vm_memory::{GuestMemoryError, mmap::FromRangesError};

#[derive(ThisError, Debug)]
pub enum Error {
    #[error("kvm error: {0}")]
    Kvm(#[from] kvm_ioctls::Error),

    #[error("guest memory error: {0}")]
    GuestMemory(#[from] GuestMemoryError),

    #[error("from ranges error: {0}")]
    FromRanges(#[from] FromRangesError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("elf parse error: {0}")]
    Parsing(#[from] goblin::error::Error),

    #[error("unexpected vCPU exit: {0}")]
    UnexpectedExit(String),

    #[error("kernel integration tests failed")]
    KernelTestsFailed,
}

pub type Result<T> = std::result::Result<T, Error>;
