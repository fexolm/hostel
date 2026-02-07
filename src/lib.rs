pub mod types;
pub mod text;
pub mod dynsym;

use goblin::elf::Elf;
#[allow(unused_imports)]
use types::{TextSectionInfo, DynSyscallInfo};

pub struct AnalysisResult {
    pub text_syscalls: Vec<types::TextSectionInfo>,
    pub dyn_syscalls: Vec<types::DynSyscallInfo>,
}

pub fn analyze(buffer: &[u8]) -> Result<AnalysisResult, goblin::error::Error> {
    match Elf::parse(&buffer) {
        Ok(binary) => {
                Ok(AnalysisResult {
                    text_syscalls: text::find_text_syscalls(&binary, buffer),
                    dyn_syscalls: dynsym::find_dyn_syscalls(&binary),
                })
        }
        Err(e) => Err(e),
    }
}