// src/types.rs

#[derive(Debug, Clone)]
pub struct SyscallInfo {
    pub offset: u64,          // Offset in section
    pub virtual_addr: u64,         // Virtual address of the syscall instruction
    pub section_name: String, // Section name (e.g., .text)
}

#[derive(Debug, Clone)]
pub struct TextSectionInfo {
    pub name: String,               // Section name (e.g., .text)
    pub virtual_addr: u64,          // Virtual address of the section
    pub file_offset: u64,           // Offset in the file where the section starts
    pub size: u64,                  // Size of the section
    pub syscalls: Vec<SyscallInfo>, // List of syscall instructions found in the section
}

#[derive(Debug, Clone)]
pub struct DynSyscallInfo {
    pub name: String, // Name of the syscall (e.g., "syscall" or "syscall@plt")
    pub virtual_addr: u64, // Virtual address of the syscall entry in .dynsym
}