use std::collections::HashMap;
use std::env;
use std::fs;

/// Representation of a compiled function.
#[derive(Clone)]
struct Function {
    params: Vec<String>,
    address: usize,
}

/// Value type for the VM stack.
#[derive(Clone)]
enum Value {
    Int(i64),
    Str(String),
    Bool(bool),
    List(Vec<Value>),
}

impl Value {
    fn as_int(&self) -> i64 {
        match self {
            Value::Int(i) => *i,
            Value::Str(s) => s.parse::<i64>().unwrap_or(0),
            Value::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            Value::List(l) => l.len() as i64,
        }
    }
    fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
        }
    }
    fn to_string(&self) -> String {
        match self {
            Value::Int(i) => i.to_string(),
            Value::Str(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::List(list) => {
                let inner: Vec<String> = list.iter().map(|v| v.to_string()).collect();
                format!("[{}]", inner.join(", "))
            }
        }
    }
}

/// Instruction set for the OMG stack VM.
enum Instr {
    PushInt(i64),
    PushStr(String),
    PushBool(bool),
    BuildList(usize),
    Load(String),
    Store(String),
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    Neg,
    Index,
    Slice,
    Jump(usize),
    JumpIfFalse(usize),
    Call(String),
    TailCall(String),
    CallBuiltin(String, usize),
    Pop,
    Ret,
    Emit,
    Halt,
}

/// Parse a textual bytecode file into instructions.
fn parse_bytecode(src: &str) -> (Vec<Instr>, HashMap<String, Function>) {
    let mut code = Vec::new();
    let mut funcs: HashMap<String, Function> = HashMap::new();
    for line in src.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("FUNC ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 3 {
                let name = parts[0].to_string();
                let param_count: usize = parts[1].parse().unwrap_or(0);
                let params = parts[2..2 + param_count]
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect::<Vec<_>>();
                let addr_idx = 2 + param_count;
                let address: usize = parts[addr_idx].parse().unwrap_or(0);
                funcs.insert(name, Function { params, address });
            }
        } else if let Some(rest) = trimmed.strip_prefix("PUSH_INT ") {
            if let Ok(v) = rest.parse::<i64>() {
                code.push(Instr::PushInt(v));
            }
        } else if let Some(rest) = trimmed.strip_prefix("PUSH_STR ") {
            code.push(Instr::PushStr(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("PUSH_BOOL ") {
            let b = rest.trim() == "1" || rest.trim().eq_ignore_ascii_case("true");
            code.push(Instr::PushBool(b));
        } else if let Some(rest) = trimmed.strip_prefix("BUILD_LIST ") {
            if let Ok(n) = rest.parse::<usize>() {
                code.push(Instr::BuildList(n));
            }
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
        } else if trimmed == "MOD" {
            code.push(Instr::Mod);
        } else if trimmed == "EQ" {
            code.push(Instr::Eq);
        } else if trimmed == "NE" {
            code.push(Instr::Ne);
        } else if trimmed == "LT" {
            code.push(Instr::Lt);
        } else if trimmed == "LE" {
            code.push(Instr::Le);
        } else if trimmed == "GT" {
            code.push(Instr::Gt);
        } else if trimmed == "GE" {
            code.push(Instr::Ge);
        } else if trimmed == "AND" {
            code.push(Instr::And);
        } else if trimmed == "OR" {
            code.push(Instr::Or);
        } else if trimmed == "NOT" {
            code.push(Instr::Not);
        } else if trimmed == "NEG" {
            code.push(Instr::Neg);
        } else if trimmed == "INDEX" {
            code.push(Instr::Index);
        } else if trimmed == "SLICE" {
            code.push(Instr::Slice);
        } else if let Some(rest) = trimmed.strip_prefix("JUMP_IF_FALSE ") {
            if let Ok(t) = rest.parse::<usize>() {
                code.push(Instr::JumpIfFalse(t));
            }
        } else if let Some(rest) = trimmed.strip_prefix("JUMP ") {
            if let Ok(t) = rest.parse::<usize>() {
                code.push(Instr::Jump(t));
            }
        } else if let Some(rest) = trimmed.strip_prefix("CALL ") {
            code.push(Instr::Call(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("TCALL ") {
            code.push(Instr::TailCall(rest.to_string()));
        } else if let Some(rest) = trimmed.strip_prefix("BUILTIN ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 2 {
                if let Ok(argc) = parts[1].parse::<usize>() {
                    code.push(Instr::CallBuiltin(parts[0].to_string(), argc));
                }
            }
        } else if trimmed == "RET" {
            code.push(Instr::Ret);
        } else if trimmed == "EMIT" {
            code.push(Instr::Emit);
        } else if trimmed == "HALT" {
            code.push(Instr::Halt);
        } else if trimmed == "POP" {
            code.push(Instr::Pop);
        }
    }
    (code, funcs)
}

/// Execute bytecode on a stack-based virtual machine.
fn run(code: &[Instr], funcs: &HashMap<String, Function>) {
    let mut stack: Vec<Value> = Vec::new();
    let mut globals: HashMap<String, Value> = HashMap::new();
    let mut env: HashMap<String, Value> = HashMap::new();
    let mut env_stack: Vec<HashMap<String, Value>> = Vec::new();
    let mut ret_stack: Vec<usize> = Vec::new();
    let mut pc: usize = 0;
    while pc < code.len() {
        match &code[pc] {
            Instr::PushInt(v) => stack.push(Value::Int(*v)),
            Instr::PushStr(s) => stack.push(Value::Str(s.clone())),
            Instr::PushBool(b) => stack.push(Value::Bool(*b)),
            Instr::BuildList(n) => {
                let mut elements = Vec::new();
                for _ in 0..*n {
                    elements.push(stack.pop().unwrap());
                }
                elements.reverse();
                stack.push(Value::List(elements));
            }
            Instr::Load(name) => {
                if let Some(v) = env.get(name) {
                    stack.push(v.clone());
                } else if let Some(v) = globals.get(name) {
                    stack.push(v.clone());
                } else {
                    stack.push(Value::Int(0));
                }
            }
            Instr::Store(name) => {
                if let Some(v) = stack.pop() {
                    if env_stack.is_empty() {
                        globals.insert(name.clone(), v);
                    } else if env.contains_key(name) {
                        env.insert(name.clone(), v);
                    } else if globals.contains_key(name) {
                        globals.insert(name.clone(), v);
                    } else {
                        env.insert(name.clone(), v);
                    }
                }
            }
            Instr::Add => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                match (a, b) {
                    (Value::Str(sa), Value::Str(sb)) => stack.push(Value::Str(sa + &sb)),
                    (Value::Str(sa), v) => stack.push(Value::Str(sa + &v.to_string())),
                    (v, Value::Str(sb)) => stack.push(Value::Str(v.to_string() + &sb)),
                    (Value::List(mut la), Value::List(lb)) => {
                        la.extend(lb);
                        stack.push(Value::List(la));
                    }
                    (a, b) => stack.push(Value::Int(a.as_int() + b.as_int())),
                }
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
            Instr::Mod => {
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Int(a % b));
            }
            Instr::Eq => {
                let b = stack.pop().unwrap().to_string();
                let a = stack.pop().unwrap().to_string();
                stack.push(Value::Bool(a == b));
            }
            Instr::Ne => {
                let b = stack.pop().unwrap().to_string();
                let a = stack.pop().unwrap().to_string();
                stack.push(Value::Bool(a != b));
            }
            Instr::Lt => {
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Bool(a < b));
            }
            Instr::Le => {
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Bool(a <= b));
            }
            Instr::Gt => {
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Bool(a > b));
            }
            Instr::Ge => {
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Bool(a >= b));
            }
            Instr::And => {
                let b = stack.pop().unwrap().as_bool();
                let a = stack.pop().unwrap().as_bool();
                stack.push(Value::Bool(a && b));
            }
            Instr::Or => {
                let b = stack.pop().unwrap().as_bool();
                let a = stack.pop().unwrap().as_bool();
                stack.push(Value::Bool(a || b));
            }
            Instr::Not => {
                let v = stack.pop().unwrap().as_bool();
                stack.push(Value::Bool(!v));
            }
            Instr::Neg => {
                let v = stack.pop().unwrap().as_int();
                stack.push(Value::Int(-v));
            }
            Instr::Index => {
                let idx = stack.pop().unwrap().as_int() as usize;
                if let Value::List(list) = stack.pop().unwrap() {
                    stack.push(list[idx].clone());
                } else {
                    stack.push(Value::Int(0));
                }
            }
            Instr::Slice => {
                let end = stack.pop().unwrap().as_int() as usize;
                let start = stack.pop().unwrap().as_int() as usize;
                if let Value::List(list) = stack.pop().unwrap() {
                    let slice = list[start..end].to_vec();
                    stack.push(Value::List(slice));
                } else {
                    stack.push(Value::List(vec![]));
                }
            }
            Instr::Jump(target) => {
                pc = *target;
                continue;
            }
            Instr::JumpIfFalse(target) => {
                let cond = stack.pop().unwrap().as_bool();
                if !cond {
                    pc = *target;
                    continue;
                }
            }
            Instr::Call(name) => {
                if let Some(func) = funcs.get(name) {
                    let mut new_env = HashMap::new();
                    for param in func.params.iter().rev() {
                        let arg = stack.pop().unwrap();
                        new_env.insert(param.clone(), arg);
                    }
                    env_stack.push(env);
                    ret_stack.push(pc + 1);
                    env = new_env;
                    pc = func.address;
                    continue;
                } else {
                    panic!("Unknown function: {}", name);
                }
            }
            Instr::TailCall(name) => {
                if let Some(func) = funcs.get(name) {
                    let mut new_env = HashMap::new();
                    for param in func.params.iter().rev() {
                        let arg = stack.pop().unwrap();
                        new_env.insert(param.clone(), arg);
                    }
                    env = new_env;
                    pc = func.address;
                    continue;
                } else {
                    panic!("Unknown function: {}", name);
                }
            }
            Instr::CallBuiltin(name, argc) => {
                let mut args: Vec<Value> = Vec::new();
                for _ in 0..*argc {
                    args.push(stack.pop().unwrap());
                }
                args.reverse();
                let result = match name.as_str() {
                    "chr" => match args.as_slice() {
                        [Value::Int(i)] => Value::Str((*i as u8 as char).to_string()),
                        _ => panic!("chr() expects one integer"),
                    },
                    "ascii" => match args.as_slice() {
                        [Value::Str(s)] if s.chars().count() == 1 => {
                            Value::Int(s.chars().next().unwrap() as i64)
                        }
                        _ => panic!("ascii() expects a single character"),
                    },
                    "hex" => match args.as_slice() {
                        [Value::Int(i)] => Value::Str(format!("{:x}", i)),
                        _ => panic!("hex() expects one integer"),
                    },
                    "binary" => match args.as_slice() {
                        [Value::Int(n)] => Value::Str(format!("{:b}", n)),
                        [Value::Int(n), Value::Int(width)] => {
                            if *width <= 0 {
                                panic!("binary() width must be positive");
                            }
                            let mask = (1_i64 << width) - 1;
                            Value::Str(format!("{:0width$b}", n & mask, width = *width as usize))
                        }
                        _ => panic!("binary() expects one or two integers"),
                    },
                    "length" => match args.as_slice() {
                        [Value::List(list)] => Value::Int(list.len() as i64),
                        [Value::Str(s)] => Value::Int(s.chars().count() as i64),
                        _ => panic!("length() expects a list or string"),
                    },
                    "read_file" => match args.as_slice() {
                        [Value::Str(path)] => {
                            let content = fs::read_to_string(path).expect("failed to read file");
                            Value::Str(content)
                        }
                        _ => panic!("read_file() expects a file path"),
                    },
                    _ => panic!("unknown builtin: {}", name),
                };
                stack.push(result);
            }
            Instr::Pop => {
                stack.pop();
            }
            Instr::Ret => {
                let ret_val = stack.pop().unwrap_or(Value::Int(0));
                pc = ret_stack.pop().unwrap();
                env = env_stack.pop().unwrap();
                stack.push(ret_val);
                continue;
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
    let (code, funcs) = parse_bytecode(&src);
    run(&code, &funcs);
}
