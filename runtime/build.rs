use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out_bc = out_dir.join("interpreter.omgb");

    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .expect("CARGO_MANIFEST_DIR has no parent")
        .to_path_buf();

    let status = Command::new("python")
        .arg("-m")
        .arg("omglang.compiler")
        .arg("bootstrap/interpreter.omg")
        .arg(&out_bc)
        .current_dir(&root)
        .status()
        .expect("failed to run compiler");

    if !status.success() {
        panic!("compilation failed");
    }

    if env::var("DUMP_OMGB").map(|v| v == "1").unwrap_or(false) {
        let cwd = env::current_dir().expect("failed to get current dir");
        let dest = cwd.join("interpreter.omgb");
        fs::copy(&out_bc, &dest).expect("failed to copy .omgb file");
        println!("Copied {} -> {}", out_bc.display(), dest.display());
    }

    println!(
        "cargo:rerun-if-changed={}",
        root.join("bootstrap/interpreter.omg").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        root.join("omglang/compiler.py").display()
    );
}
