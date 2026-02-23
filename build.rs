use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn gen_linker_script(linker_script_path: &PathBuf) {
    let linker_script_content = format!(
        r#"
        ENTRY(_start)
        MEMORY
        {{
            phys (rx) : ORIGIN = {phys:#x}, LENGTH = 1M
            virt (rw) : ORIGIN = {virt:#x}, LENGTH = 1M
        }}

        PHDRS
        {{
            text PT_LOAD FLAGS(5);    /* RX - Read + Execute */
            data PT_LOAD FLAGS(6);    /* RW - Read + Write */
        }}

        SECTIONS {{
            .text : ALIGN(4K) {{
                *(.text .text.*)
            }} > virt AT > phys :text

            .rodata : ALIGN(4K) {{
                *(.rodata .rodata.*) 
            }} > virt AT > phys :text

                .data : ALIGN(4K) {{
                    *(.data .data.*) 
            }} > virt AT > phys :data

                .bss : ALIGN(4K) {{
                    *(.bss .bss.*) 
                    *(COMMON)
            }} > virt :data
        }}
        "#,
        virt = kernel::constants::KERNEL_CODE_VIRT.0,
        phys = kernel::constants::KERNEL_CODE_PHYS.0,
    );

    let mut f = File::create(linker_script_path).unwrap();
    f.write_all(linker_script_content.as_bytes()).unwrap();
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let kernel_dir = env::current_dir().unwrap().join("kernel");
    let linker_script_path = out_dir.join("linker.ld");

    gen_linker_script(&linker_script_path);

    let rustflags = format!(
        "-C link-arg=-T{} -C relocation-model=static -C code-model=kernel",
        linker_script_path.display()
    );

    let status = Command::new("cargo")
        .env("RUSTFLAGS", rustflags)
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .args([
            "build",
            "--release",
            "--target",
            "x86_64-unknown-none",
            "--target-dir",
            out_dir.join("kernel-target").to_str().unwrap(),
        ])
        .current_dir(&kernel_dir)
        .status()
        .expect("Failed to run cargo build for kernel");

    if !status.success() {
        panic!("compiling kernel crate failed");
    }

    let elf_path = out_dir.join("kernel-target/x86_64-unknown-none/release/kernel");

    println!("cargo:rustc-env=KERNEL_BIN={}", elf_path.display());

    println!("cargo:rerun-if-changed=kernel");
}
