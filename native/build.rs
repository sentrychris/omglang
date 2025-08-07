use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out_bc = out_dir.join("interpreter.bc");

    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .expect("CARGO_MANIFEST_DIR has no parent")
        .to_path_buf();

    let status = Command::new("python")
        .arg("-m")
        .arg("omglang.bytecode")
        .arg("bootstrap/interpreter.omg")
        .arg(&out_bc)
        .current_dir(&root)
        .status()
        .expect("failed to run bytecode compiler");

    if !status.success() {
        panic!("bytecode compilation failed");
    }

    println!("cargo:rerun-if-changed={}", root.join("bootstrap/interpreter.omg").display());
    println!("cargo:rerun-if-changed={}", root.join("omglang/bytecode.py").display());
}
