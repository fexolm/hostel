use thiserror::Error as ThisError;

use crate::memory::errors::MemoryError;

#[derive(ThisError, Debug)]
pub enum Error {
    #[error("memory error: {0}")]
    Memory(MemoryError),
}
