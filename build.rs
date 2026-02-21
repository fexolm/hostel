use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let kernel_dir = env::current_dir().unwrap().join("kernel");

    let status = Command::new("cargo")
        .env_remove("RUSTFLAGS")
        .env_remove("RUSTC_WORKSPACE_WRAPPER") // Убирает конфликты с sccache/clippy
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

    println!("cargo:rerun-if-changed=kernel/src/main.rs");
}
