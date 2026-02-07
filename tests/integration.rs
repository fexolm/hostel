use std::{fs, process::Command};

fn compile_rust_binary(source: &str, output: &str, extra_args: &[&str]) {
    let mut args = vec![source, "-o", output];
    args.extend(extra_args);

    let status = Command::new("rustc")
        .args(&args)
        .status()
        .expect("Failed to execute rustc");

    assert!(status.success(), "Compilation of {} failed", source);
}

fn analyze_binary(path: &str) -> hostel::AnalysisResult {
    let buffer = fs::read(path).expect("Failed to read compiled binary");
    hostel::analyze(&buffer).expect("Analysis failed")
}

#[test]
fn write_syscall() {
    let bin_path = "tests/bins/write_syscall_bin";

    // Compile the test binary
    compile_rust_binary("tests/bins/write_syscall.rs", bin_path, &[]);

    // Analyze the binary
    let result = analyze_binary(bin_path);
    let text_syscall_count = result
        .text_syscalls
        .iter()
        .filter(|sec| !sec.syscalls.is_empty())
        .count();
    assert!(text_syscall_count == 1, "Expected one text syscall, found {}", text_syscall_count);
    
    let dynamic_syscall_count = result
        .dyn_syscalls
        .iter()
        .filter(|s| s.name.contains("syscall"))
        .count();
    // Call noinline syscall
    assert!(dynamic_syscall_count == 1);
}

#[test]
fn get_pid_syscall() {
    let bin_path = "tests/bins/get_pid_syscall_bin";

    // Compile the inline syscall binary with no optimizations
    compile_rust_binary("tests/bins/get_pid_syscall.rs", bin_path, &["-C", "opt-level=0"]);

    // Analyze the binary
    let result = analyze_binary(bin_path);

    let text_syscall_count = result
        .text_syscalls
        .iter()
        .filter(|sec| !sec.syscalls.is_empty())
        .count();
    assert!(text_syscall_count == 1, "Expected one text syscall, found {}", text_syscall_count);
    
    let dynamic_syscall_count = result
        .dyn_syscalls
        .iter()
        .filter(|s| s.name.contains("syscall"))
        .count();
    // Call noinline syscall
    assert!(dynamic_syscall_count == 1);
}
