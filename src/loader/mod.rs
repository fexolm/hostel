pub mod error;
pub mod module;
mod vm;

pub use error::{Error, Result};

use crate::loader::vm::Vm;

pub struct Loader {
    vm: Vm,
}

impl Loader {
    pub fn new() -> Result<Self> {
        Ok(Self { vm: Vm::new()? })
    }
}
