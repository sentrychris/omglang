//! Build script for the OMG runtime.
//!
//! Bootstraps two stage-1 OMG-language artifacts into bytecode the runtime
//! ships embedded:
//!
//! * `bootstrap/compiler.omg` → `bootstrap/compiler.omgb` — the OMG-written
//!   OMG compiler (used by the default `omg <script>` execution path and
//!   by `--self-hosted-compile` / `--verify-self-hosted`).
//! * `bootstrap/vm.omg` → `bootstrap/vm.omgb` — the OMG-written OMG VM
//!   (used by `--verify-omg-vm` for the triple-meta fixed-point check).
//!
//! Both are compiled by the runtime's own lexer/parser/compiler — no
//! Python, no external tools. The Rust frontend is the "stage-0" that
//! bootstraps the "stage-1" OMG-in-OMG implementations. Subsequent
//! self-hosted runs (and the `verify-*` flags) confirm fixed points:
//! stage-1 compiling its own source produces the same `.omgb` bytes as
//! stage-0 did, and the OMG VM running stage-1 produces the same
//! bytes as either.

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
    println!("cargo:rerun-if-changed=bootstrap/vm.omg");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let project_root = PathBuf::from(&manifest_dir)
        .parent()
        .expect("project root")
        .to_path_buf();

    bootstrap_one(&project_root, "bootstrap/compiler.omg", "bootstrap/compiler.omgb");
    bootstrap_one(&project_root, "bootstrap/vm.omg", "bootstrap/vm.omgb");
}

fn bootstrap_one(project_root: &PathBuf, src_rel: &str, out_rel: &str) {
    let src_path = project_root.join(src_rel);
    let out_path = project_root.join(out_rel);
    let src = fs::read_to_string(&src_path)
        .unwrap_or_else(|e| panic!("read {}: {}", src_path.display(), e));
    let program = compiler::compile_source(&src, &src_path)
        .unwrap_or_else(|e| panic!("compile {}: {}", src_path.display(), e));
    let bytes = bytecode::write_bytecode(&program.code, &program.funcs);
    fs::write(&out_path, &bytes)
        .unwrap_or_else(|e| panic!("write {}: {}", out_path.display(), e));
}
