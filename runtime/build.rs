//! Build script for the OMG runtime.
//!
//! Bootstraps `bootstrap/compiler.omg` (the OMG-written OMG compiler) into
//! `bootstrap/compiler.omgb` so the runtime can ship it embedded for
//! `--self-hosted` mode.
//!
//! The bootstrap is performed by directly using the runtime's *own* lexer,
//! parser and compiler — no Python, no external tools. This is the
//! "stage-0" Rust compiler bootstrapping the "stage-1" OMG-in-OMG compiler.
//! Subsequent self-hosted runs (and the `verify-self-hosted` make target)
//! confirm the fixed point: stage-1 compiling its own source produces the
//! exact same `.omgb` bytes as stage-0 did.

use std::env;
use std::fs;
use std::path::PathBuf;

// These modules are shared with the runtime binary via `#[path]`. The build
// script only needs a subset (parsing + write_bytecode); the binary uses the
// rest. Suppress dead-code warnings here so the *binary's* live code isn't
// flagged from the build script's narrower view.
#[allow(dead_code)]
#[path = "src/ast.rs"]
mod ast;
#[allow(dead_code)]
#[path = "src/error.rs"]
mod error;
#[allow(dead_code)]
#[path = "src/lexer.rs"]
mod lexer;
#[allow(dead_code)]
#[path = "src/parser.rs"]
mod parser;
#[allow(dead_code)]
#[path = "src/bytecode.rs"]
mod bytecode;
#[allow(dead_code)]
#[path = "src/compiler.rs"]
mod compiler;

fn main() {
    println!("cargo:rerun-if-changed=bootstrap/compiler.omg");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let project_root = PathBuf::from(&manifest_dir)
        .parent()
        .expect("project root")
        .to_path_buf();
    let src_path = project_root.join("bootstrap/compiler.omg");
    let out_path = project_root.join("bootstrap/compiler.omgb");

    let src = fs::read_to_string(&src_path).expect("read compiler.omg");
    let program = compiler::compile_source(&src, &src_path)
        .unwrap_or_else(|e| panic!("failed to compile compiler.omg: {}", e));
    let bytes = bytecode::write_bytecode(&program.code, &program.funcs);
    fs::write(&out_path, &bytes).expect("write compiler.omgb");
}
