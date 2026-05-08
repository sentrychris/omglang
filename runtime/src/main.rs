//! OMG Language Runtime entry point.
//!
//! The runtime is fully self-hosted in Rust: it owns the lexer, parser,
//! compiler, bytecode reader/writer, and VM. There is no longer any
//! dependency on Python or on the embedded self-hosted `interpreter.omg`.
//!
//! ## Modes
//! - **No args** → interactive REPL.
//! - `-h` / `--help` / `-v` / `--version` → print and exit.
//! - `--compile <in.omg> [<out.omgb>]` → compile a source file to bytecode.
//!   When `<out.omgb>` is omitted, the bytes are written to stdout.
//! - `--disasm <file>` → print a textual disassembly of `.omg` or `.omgb`
//!   input to stdout (helpful for debugging).
//! - `<file.omg>` → compile in-process and execute.
//! - `<file.omgb>` → load bytecode and execute.
//!
//! ## Argument forwarding
//! Anything after the script path (optionally separated by `--`) is exposed
//! to the running program via the `args` global, with `module_file` and
//! `current_dir` derived from the script path.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

mod ast;
mod bytecode;
mod compiler;
mod error;
mod lexer;
mod parser;
mod repl;
mod value;
mod vm;

use bytecode::{parse_bytecode, write_bytecode, Function, Instr};
use compiler::compile_source;
use error::RuntimeError;
use repl::repl_interpret;
use vm::run;

/// Human-facing runtime version. Bump in lockstep with `Cargo.toml`.
const VERSION: &str = "0.2.0";

/// Embedded `bootstrap/compiler.omgb` — the OMG-written OMG compiler,
/// pre-compiled by the Rust frontend at build time. Loaded on demand by
/// `--self-hosted` and `--verify-self-hosted`.
const SELF_HOSTED_COMPILER: &[u8] =
    include_bytes!("../../bootstrap/compiler.omgb");

fn usage() -> String {
    format!(
        r#"OMG Language Runtime v{0}

Usage:
    omg [--rust] [<script>]
    omg --compile <in.omg> [<out.omgb>]
    omg --disasm <file>

Arguments:
    <script>
        A `.omg` source file or a precompiled `.omgb` file. By default,
        `.omg` sources are compiled by the embedded OMG-in-OMG compiler
        (stage-1) running on the VM. Pass `--rust` to use the Rust
        frontend instead — significantly faster, but bypasses the
        self-hosted toolchain.

Options:
    -h, --help               Show this help message and exit.
    -v, --version            Show runtime version.
        --rust               Use the Rust frontend (stage-0) to compile the
                             script instead of the embedded OMG-in-OMG
                             compiler. Faster for one-off runs.
        --compile            Compile a `.omg` file to `.omgb` (Rust frontend).
        --disasm             Disassemble a `.omg` or `.omgb` file.
        --self-hosted-compile <in.omg> [<out.omgb>]
                             Like --compile, but uses the OMG-in-OMG compiler.
                             Writes bytecode to <out.omgb> or stdout.
        --verify-self-hosted Run the fixed-point check: compile a `.omg` with
                             both the Rust and OMG-in-OMG compilers and
                             confirm the byte streams are identical.

Examples:
    omg hello.omg                    # self-hosted (default)
    omg --rust hello.omg             # Rust frontend
    omg hello.omgb -- arg1 arg2
    omg --compile hello.omg hello.omgb
    omg --verify-self-hosted bootstrap/compiler.omg"#,
        VERSION
    )
}

fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().collect();

    // Pull `--rust` off the front (if present) so the rest of dispatch
    // doesn't have to know it exists. It only affects the bare-script
    // execution path: `--compile` is already Rust-only, `--disasm` is
    // backend-agnostic, and the various `--self-hosted-*` commands
    // are explicit about which compiler they want.
    let mut use_rust = false;
    if args.get(1).map(|s| s.as_str()) == Some("--rust") {
        use_rust = true;
        args.remove(1);
    }
    // `--self-hosted` used to be the opt-in flag for the OMG-in-OMG
    // compiler; that is now the default, so the flag is a no-op alias.
    // We keep it for back-compat with anyone who has it in their muscle
    // memory or in scripts.
    if args.get(1).map(|s| s.as_str()) == Some("--self-hosted") {
        args.remove(1);
    }

    if args.len() == 1 {
        repl_interpret();
        return ExitCode::SUCCESS;
    }

    match args[1].as_str() {
        "-h" | "--help" => {
            println!("{}", usage());
            return ExitCode::SUCCESS;
        }
        "-v" | "--version" => {
            println!(
                "omg-runtime-build-{}-{}: v{}",
                env::consts::OS,
                env::consts::ARCH,
                VERSION
            );
            return ExitCode::SUCCESS;
        }
        "--compile" => return cmd_compile(&args[2..]),
        "--disasm" => return cmd_disasm(&args[2..]),
        "--self-hosted-compile" => return cmd_self_hosted_compile(&args[2..]),
        "--verify-self-hosted" => return cmd_verify_self_hosted(&args[2..]),
        _ => {}
    }

    let script = PathBuf::from(&args[1]);
    let program_args: Vec<String> = if args.len() > 2 {
        if args[2] == "--" {
            args[3..].to_vec()
        } else {
            args[2..].to_vec()
        }
    } else {
        Vec::new()
    };
    let mut full_args = Vec::with_capacity(program_args.len() + 1);
    full_args.push(script.to_string_lossy().to_string());
    full_args.extend_from_slice(&program_args);

    let is_omgb = script
        .extension()
        .map(|e| e == "omgb")
        .unwrap_or(false);

    // Precompiled bytecode: nothing to compile, both paths are identical.
    // Bare `.omg` source: route through the OMG-in-OMG compiler by
    // default, or the Rust frontend when the user passed `--rust`.
    let result = if is_omgb {
        run_omgb(&script, &full_args)
    } else if use_rust {
        run_omg(&script, &full_args)
    } else {
        run_omg_self_hosted(&script, &full_args)
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", e);
            ExitCode::FAILURE
        }
    }
}

fn run_omg(path: &PathBuf, full_args: &[String]) -> Result<(), RuntimeError> {
    let source = fs::read_to_string(path).map_err(|e| {
        RuntimeError::ModuleImportError(format!(
            "Cannot read script '{}': {}",
            path.display(),
            e
        ))
    })?;
    check_header(&source, path)?;
    let program = compile_source(&source, path)?;
    run(&program.code, &program.funcs, full_args)
}

/// Default execution path for a `.omg` file: compile it via the embedded
/// OMG-in-OMG compiler (running on the VM) and run the result. This is
/// what bare `omg <script>` invokes when `--rust` isn't passed.
fn run_omg_self_hosted(path: &PathBuf, full_args: &[String]) -> Result<(), RuntimeError> {
    let bytes = self_hosted_compile(path)?;
    let (code, funcs) = parse_bytecode(&bytes)?;
    run(&code, &funcs, full_args)
}

fn run_omgb(path: &PathBuf, full_args: &[String]) -> Result<(), RuntimeError> {
    let bytes = fs::read(path).map_err(|e| {
        RuntimeError::ModuleImportError(format!(
            "Cannot read bytecode '{}': {}",
            path.display(),
            e
        ))
    })?;
    let (code, funcs) = parse_bytecode(&bytes)?;
    run(&code, &funcs, full_args)
}

fn check_header(source: &str, path: &PathBuf) -> Result<(), RuntimeError> {
    for line in source.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t == ";;;omg" {
            return Ok(());
        }
        break;
    }
    Err(RuntimeError::SyntaxError(format!(
        "OMG script missing required header ';;;omg' in {}",
        path.display()
    )))
}

fn cmd_compile(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("--compile expects an input .omg path");
        return ExitCode::FAILURE;
    }
    let in_path = PathBuf::from(&args[0]);
    let out: Option<PathBuf> = args.get(1).map(PathBuf::from);
    let source = match fs::read_to_string(&in_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read '{}': {}", in_path.display(), e);
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = check_header(&source, &in_path) {
        eprintln!("{}", e);
        return ExitCode::FAILURE;
    }
    let program = match compile_source(&source, &in_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::FAILURE;
        }
    };
    let bytes = write_bytecode(&program.code, &program.funcs);
    match out {
        Some(p) => {
            if let Err(e) = fs::write(&p, &bytes) {
                eprintln!("cannot write '{}': {}", p.display(), e);
                return ExitCode::FAILURE;
            }
        }
        None => {
            use std::io::Write;
            if std::io::stdout().write_all(&bytes).is_err() {
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

/// Compile a `.omg` file with the embedded OMG-in-OMG compiler and write
/// the bytecode to disk — the self-hosted analogue of `--compile`.
fn cmd_self_hosted_compile(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("--self-hosted-compile expects <in.omg> [<out.omgb>]");
        return ExitCode::FAILURE;
    }
    let in_path = PathBuf::from(&args[0]);
    let out: Option<PathBuf> = args.get(1).map(PathBuf::from);
    let bytes = match self_hosted_compile(&in_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::FAILURE;
        }
    };
    match out {
        Some(p) => {
            if let Err(e) = fs::write(&p, &bytes) {
                eprintln!("cannot write '{}': {}", p.display(), e);
                return ExitCode::FAILURE;
            }
        }
        None => {
            use std::io::Write;
            if std::io::stdout().write_all(&bytes).is_err() {
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

/// Compile a `.omg` file with both the Rust frontend and the OMG-in-OMG
/// frontend, then assert the byte streams are identical. Used to verify
/// that the self-hosted compiler is a fixed point.
fn cmd_verify_self_hosted(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("--verify-self-hosted expects a .omg path");
        return ExitCode::FAILURE;
    }
    let in_path = PathBuf::from(&args[0]);
    let source = match fs::read_to_string(&in_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read '{}': {}", in_path.display(), e);
            return ExitCode::FAILURE;
        }
    };
    let rust_bytes = match compile_source(&source, &in_path) {
        Ok(p) => write_bytecode(&p.code, &p.funcs),
        Err(e) => {
            eprintln!("Rust frontend failed: {}", e);
            return ExitCode::FAILURE;
        }
    };
    let omg_bytes = match self_hosted_compile(&in_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("OMG frontend failed: {}", e);
            return ExitCode::FAILURE;
        }
    };
    if rust_bytes == omg_bytes {
        println!(
            "OK  {} ({} bytes) — self-hosted output matches Rust output",
            in_path.display(),
            rust_bytes.len()
        );
        ExitCode::SUCCESS
    } else {
        println!(
            "FAIL  {}: Rust={} bytes, OMG={} bytes — outputs differ",
            in_path.display(),
            rust_bytes.len(),
            omg_bytes.len()
        );
        ExitCode::FAILURE
    }
}

/// Drive the embedded OMG-written compiler against a source file and return
/// the resulting `.omgb` bytes. Uses a temp file to thread the result back
/// into the host process — the embedded compiler is a normal OMG program
/// that takes [in.omg, out.omgb] as args and writes the bytecode to disk.
fn self_hosted_compile(in_path: &PathBuf) -> Result<Vec<u8>, RuntimeError> {
    let (code, funcs) = parse_bytecode(SELF_HOSTED_COMPILER)?;
    let abs_in = fs::canonicalize(in_path).map_err(|e| {
        RuntimeError::ModuleImportError(format!(
            "cannot canonicalise '{}': {}",
            in_path.display(),
            e
        ))
    })?;
    let tmp = std::env::temp_dir().join(format!(
        "omg-stage1-{}-{}.omgb",
        std::process::id(),
        rand_suffix()
    ));
    let args = [
        "<embedded>".to_string(),
        abs_in.to_string_lossy().to_string(),
        tmp.to_string_lossy().to_string(),
    ];
    run(&code, &funcs, &args)?;
    let bytes = fs::read(&tmp).map_err(|e| {
        RuntimeError::ModuleImportError(format!(
            "cannot read self-hosted output '{}': {}",
            tmp.display(),
            e
        ))
    })?;
    let _ = fs::remove_file(&tmp);
    Ok(bytes)
}

fn rand_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn cmd_disasm(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("--disasm expects a file path");
        return ExitCode::FAILURE;
    }
    let path = PathBuf::from(&args[0]);
    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read '{}': {}", path.display(), e);
            return ExitCode::FAILURE;
        }
    };
    let (code, funcs): (Vec<Instr>, std::collections::HashMap<String, Function>) =
        if path.extension().map(|e| e == "omgb").unwrap_or(false) {
            match parse_bytecode(&bytes) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", e);
                    return ExitCode::FAILURE;
                }
            }
        } else {
            let source = String::from_utf8_lossy(&bytes).to_string();
            if let Err(e) = check_header(&source, &path) {
                eprintln!("{}", e);
                return ExitCode::FAILURE;
            }
            match compile_source(&source, &path) {
                Ok(p) => (p.code, p.funcs),
                Err(e) => {
                    eprintln!("{}", e);
                    return ExitCode::FAILURE;
                }
            }
        };
    println!("# functions");
    let mut names: Vec<&String> = funcs.keys().collect();
    names.sort();
    for name in names {
        let f = &funcs[name];
        println!(
            "FUNC {} ({}) @ {}",
            name,
            f.params.join(", "),
            f.address
        );
    }
    println!("# code");
    for (i, instr) in code.iter().enumerate() {
        println!("{:04}  {:?}", i, instr);
    }
    ExitCode::SUCCESS
}
