use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Rebuild if either the bootstrap source or the compiler changes
    println!("cargo:rerun-if-changed=bootstrap/interpreter.omg");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let src = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("bootstrap/interpreter.omg");
    let out_bc = out_dir.join("interpreter.omgb");

    let status = Command::new("python")
        .arg("-m")
        .arg("omglang.compiler")
        .arg(&src)
        .arg(&out_bc)
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
}
