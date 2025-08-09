use std::env;
use std::fs;

mod bytecode;
mod repl;
mod value;
mod vm;

use bytecode::parse_bytecode;
use repl::interpret;
use vm::run;

/// Embedded interpreter bytecode generated at build time.
const INTERPRETER_BC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/interpreter.omgb"));

const VERSION: &str = "0.1.1";

fn usage() -> String {
    format!(
        r#"OMG Language Runtime v{0}

Usage:
    omg <script.omg>

Arguments:
    <script.omg>
        Path to an OMG language source file to execute. The file must
        include the required header ';;;omg' on the first non-empty line.

Example:
    omg hello.omg

Options:
    -h, --help
        Show this help message and exit.
    --version
        Show runtime version."#,
        VERSION
    )
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() == 1 {
        interpret();
        return;
    }
    if args[1] == "-h" || args[1] == "--help" {
        println!("{}", usage());
        return;
    }
    if args[1] == "--version" {
        println!(
            "omg-runtime-build-{}-{}: v{}",
            env::consts::OS,
            env::consts::ARCH,
            VERSION
        );
        return;
    }
    if args[1].ends_with(".omgb") {
        let bc_path = &args[1];
        let program_args: &[String] = if args.len() > 2 {
            if args[2] == "--" {
                &args[3..]
            } else {
                &args[2..]
            }
        } else {
            &[]
        };
        let src = fs::read(bc_path).expect("failed to read bytecode file");
        let (code, funcs) = parse_bytecode(&src);
        if let Err(e) = run(&code, &funcs, program_args) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    } else {
        let prog_path = &args[1];
        let program_args_slice: &[String] = if args.len() > 2 {
            if args[2] == "--" {
                &args[3..]
            } else {
                &args[2..]
            }
        } else {
            &[]
        };
        let mut full_args = Vec::with_capacity(program_args_slice.len() + 1);
        full_args.push(prog_path.clone());
        full_args.extend_from_slice(program_args_slice);
        let (code, funcs) = parse_bytecode(INTERPRETER_BC);
        if let Err(e) = run(&code, &funcs, &full_args) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
