use std::{env, fs, path::Path};

use goblin::elf::Elf;

fn process_elf_binary(binary: &Elf) {

    for h in &binary.section_headers {
        println!("{}", binary.shdr_strtab.get_at(h.sh_name).unwrap());
    }
}

fn main() {
    let len = env::args().len();

    if len != 2 {
        println!("usage: hostel <path to binary>");
        return;
    }

    let path_str = env::args().last().unwrap();

    let path = Path::new(&path_str);

    let buffer = fs::read(path).unwrap();

    match Elf::parse(&buffer) {
        Ok(binary) => {
            process_elf_binary(&binary);
        }
        Err(_) => (),
    }
}
