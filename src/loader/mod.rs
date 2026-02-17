use crate::loader::executable::Executable;

mod arch;
mod code_buffer;
pub mod executable;

pub struct Loader {}

impl Loader {
    pub fn new() -> Self {
        Self {}
    }

    pub fn load(&self, filename: &str) -> Executable {
        // read elf binary
        // allocate code buffer
        // patch code buffer
        // make executable
        todo!();
    }
}
