use std::sync::Arc;

use vm_memory::GuestMemoryMmap;

/// Represents a loaded executable unit (binary or shared library) within the process address space.
///
/// A `Module` encapsulates one or more memory-mapped segments that have been parsed,
/// loaded, and patched (e.g., syscall redirection).
pub struct Module {
    /// Memory-mapped executable segments and read-only data.
    code: Vec<GuestMemoryMmap<()>>,
    /// Shared dependencies required by this module (e.g., loaded .so files).
    deps: Vec<Arc<Module>>,
}

impl Module {
    pub(crate) fn new(code: Vec<GuestMemoryMmap<()>>, deps: Vec<Arc<Module>>) -> Self {
        Self { code, deps }
    }
}

pub struct Executable {
    module: Arc<Module>,
}

impl Executable {
    fn new(module: Arc<Module>) -> Self {
        Self { module }
    }

    pub fn run(&self) {
        todo!()
    }
}
