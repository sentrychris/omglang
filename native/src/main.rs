use std::collections::HashMap;
use std::env;
use std::fs;

/// Value type for the VM stack.
#[derive(Clone)]
enum Value {
    Int(i64),
    Str(String),
}

impl Value {
    fn as_int(&self) -> i64 {
        match self {
            Value::Int(i) => *i,
            Value::Str(s) => s.parse::<i64>().unwrap_or(0),
        }
    }
    fn to_string(&self) -> String {
        match self {
            Value::Int(i) => i.to_string(),
            Value::Str(s) => s.clone(),
        }
    }
}

/// Instruction set for the OMG stack VM.
enum Instr {
    PushInt(i64),
    PushStr(String),
    Load(String),
    Store(String),
    Add,
    Sub,
    Mul,
    Div,
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
        if let Some(rest) = trimmed.strip_prefix("PUSH_INT ") {
            if let Ok(v) = rest.parse::<i64>() {
                code.push(Instr::PushInt(v));
            }
        } else if let Some(rest) = trimmed.strip_prefix("PUSH_STR ") {
            code.push(Instr::PushStr(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("LOAD ") {
            code.push(Instr::Load(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("STORE ") {
            code.push(Instr::Store(rest.to_string()));
        } else if trimmed == "ADD" {
            code.push(Instr::Add);
        } else if trimmed == "SUB" {
            code.push(Instr::Sub);
        } else if trimmed == "MUL" {
            code.push(Instr::Mul);
        } else if trimmed == "DIV" {
            code.push(Instr::Div);
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
    let mut stack: Vec<Value> = Vec::new();
    let mut env: HashMap<String, Value> = HashMap::new();
    let mut pc: usize = 0;
    while pc < code.len() {
        match &code[pc] {
            Instr::PushInt(v) => stack.push(Value::Int(*v)),
            Instr::PushStr(s) => stack.push(Value::Str(s.clone())),
            Instr::Load(name) => {
                if let Some(v) = env.get(name) {
                    stack.push(v.clone());
                } else {
                    stack.push(Value::Int(0));
                }
            }
            Instr::Store(name) => {
                if let Some(v) = stack.pop() {
                    env.insert(name.clone(), v);
                }
            }
            Instr::Add => {
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Int(a + b));
            }
            Instr::Sub => {
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Int(a - b));
            }
            Instr::Mul => {
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Int(a * b));
            }
            Instr::Div => {
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Int(a / b));
            }
            Instr::Emit => {
                if let Some(v) = stack.pop() {
                    println!("{}", v.to_string());
                }
            }
            Instr::Halt => break,
        }
        pc += 1;
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

