use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let boot_dir = env::current_dir().unwrap().join("boot");

    let status = Command::new("cargo")
        .env_remove("RUSTFLAGS")
        .env_remove("RUSTC_WORKSPACE_WRAPPER") // Убирает конфликты с sccache/clippy
        .args([
            "build",
            "--release",
            "--target",
            "x86_64-unknown-none",
            "--target-dir",
            out_dir.join("boot-target").to_str().unwrap(),
        ])
        .current_dir(&boot_dir)
        .status()
        .expect("Failed to run cargo build for boot");

    if !status.success() {
        panic!("compiling boot crate failed");
    }

    let elf_path = out_dir.join("boot-target/x86_64-unknown-none/release/boot");

    println!("cargo:rustc-env=BOOT_ELF_PATH={}", elf_path.display());

    println!("cargo:rerun-if-changed=boot/src/main.rs");
}
