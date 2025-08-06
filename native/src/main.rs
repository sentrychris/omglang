use std::env;
use std::fs;

/// Simple instruction set for the OMG stack VM.
enum Instr {
    PushConst(String),
    Emit,
    Halt,
}

/// Parse a textual bytecode file into instructions.
fn parse_bytecode(src: &str) -> Vec<Instr> {
    let mut code = Vec::new();
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("PUSH_CONST ") {
            code.push(Instr::PushConst(rest.to_string()));
        } else if trimmed == "EMIT" {
            code.push(Instr::Emit);
        } else if trimmed == "HALT" {
            code.push(Instr::Halt);
        }
    }
    code
}

/// Execute bytecode on a stack-based virtual machine.
fn run(code: &[Instr]) {
    let mut stack: Vec<String> = Vec::new();
    for instr in code {
        match instr {
            Instr::PushConst(v) => stack.push(v.clone()),
            Instr::Emit => {
                if let Some(v) = stack.pop() {
                    println!("{}", v);
                }
            }
            Instr::Halt => break,
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: omg_native <bytecode_file>");
        std::process::exit(1);
    }
    let src = fs::read_to_string(&args[1]).expect("failed to read bytecode file");
    let code = parse_bytecode(&src);
    run(&code);
}

