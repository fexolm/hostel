use goblin::elf::Elf;
use iced_x86::{Decoder, DecoderOptions, Mnemonic};

use crate::types::{SyscallInfo, TextSectionInfo};

pub fn find_text_syscalls(binary: &Elf, buffer: &[u8]) -> Vec<TextSectionInfo> {
    let bitness = match binary.header.e_machine {
        goblin::elf::header::EM_X86_64 => 64,
        goblin::elf::header::EM_386 => 32,
        _ => {
            println!(
                "Unsupported architecture: e_machine = {}",
                binary.header.e_machine
            );
            return Vec::new();
        }
    };

    let mut text_sections: Vec<TextSectionInfo> = Vec::new();

    for sh in &binary.section_headers {
        let flags = sh.sh_flags as u32;
        if flags & goblin::elf::section_header::SHF_EXECINSTR == 0 {
            continue; // skip non-executable sections
        }
        let section_name = binary.shdr_strtab.get_at(sh.sh_name).unwrap_or("<unknown>");
        if section_name != ".text" {
            continue; // skip non-.text sections
        }
        let file_offset = sh.sh_offset as usize;
        let section_size = sh.sh_size as usize;
        let section_data = &buffer[file_offset..file_offset + section_size];
        let section_vaddr = sh.sh_addr;
        let mut syscalls: Vec<SyscallInfo> = Vec::new();
        let mut decoder = Decoder::with_ip(
            bitness, 
            section_data, 
            section_vaddr, 
            DecoderOptions::NONE,
        );

        while decoder.can_decode() {
            let instruction = decoder.decode();
            if instruction.mnemonic() == Mnemonic::Syscall {
                let offset = instruction.ip() - section_vaddr;
                syscalls.push(SyscallInfo {
                    offset,
                    virtual_addr: instruction.ip(),
                    section_name: section_name.to_string(),
                });
            }
        }

        let text_info = TextSectionInfo {
            name: section_name.to_string(),
            virtual_addr: section_vaddr,
            file_offset: sh.sh_offset,
            size: sh.sh_size,
            syscalls,
        };
        text_sections.push(text_info);
    }

    text_sections
}
