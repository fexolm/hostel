use goblin::elf::Elf;
use crate::types::DynSyscallInfo;

pub fn find_dyn_syscalls(binary: &Elf) -> Vec<DynSyscallInfo> {
    let mut result = Vec::new();

    for sym in &binary.dynsyms {
        let name = binary.dynstrtab.get_at(sym.st_name).unwrap_or("");

        // minimal, honest criterion
        if name == "syscall" || name.starts_with("syscall@") {
            result.push(DynSyscallInfo {
                name: name.to_string(),
                virtual_addr: sym.st_value,
            });
        }
    }

    result
}