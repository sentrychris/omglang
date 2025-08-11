use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::rc::Rc;

use crate::bytecode::{Function, Instr};
use crate::error::RuntimeError;
use crate::value::Value;

mod ops_arith;
mod ops_control;
mod ops_struct;

struct Block {
    handler: usize,
    stack_size: usize,
    env_depth: usize,
    ret_depth: usize,
}

fn pop(stack: &mut Vec<Value>) -> Result<Value, RuntimeError> {
    stack
        .pop()
        .ok_or_else(|| RuntimeError::VmInvariant("stack underflow".to_string()))
}

fn call_builtin(
    name: &str,
    args: &[Value],
    env: &HashMap<String, Value>,
    globals: &HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match name {
        "chr" => match args {
            [Value::Int(i)] => Ok(Value::Str((*i as u8 as char).to_string())),
            _ => Err(RuntimeError::TypeError(
                "chr() expects one integer".to_string(),
            )),
        },
        "ascii" => match args {
            [Value::Str(s)] if s.chars().count() == 1 => {
                Ok(Value::Int(s.chars().next().unwrap() as i64))
            }
            _ => Err(RuntimeError::TypeError(
                "ascii() expects a single character (arity mismatch)".to_string(),
            )),
        },
        "hex" => match args {
            [Value::Int(i)] => Ok(Value::Str(format!("{:x}", i))),
            _ => Err(RuntimeError::TypeError(
                "hex() expects one integer (arity mismatch)".to_string(),
            )),
        },
        "binary" => match args {
            [Value::Int(n)] => Ok(Value::Str(format!("{:b}", n))),
            [Value::Int(n), Value::Int(width)] => {
                if *width <= 0 {
                    Err(RuntimeError::ValueError(
                        "binary() width must be positive".to_string(),
                    ))
                } else {
                    let mask = (1_i64 << width) - 1;
                    Ok(Value::Str(format!(
                        "{:0width$b}",
                        n & mask,
                        width = *width as usize
                    )))
                }
            }
            _ => Err(RuntimeError::TypeError(
                "binary() expects one or two integers (arity mismatch)".to_string(),
            )),
        },
        "length" => {
            if args.len() != 1 {
                Err(RuntimeError::TypeError(
                    "length() expects one positional argument (arity mismatch)".to_string(),
                ))
            } else {
                match &args[0] {
                    Value::List(list) => Ok(Value::Int(list.borrow().len() as i64)),
                    Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
                    _ => Err(RuntimeError::TypeError(
                        "length() expects list or string (type mismatch)".to_string(),
                    )),
                }
            }
        }
        "freeze" => match args {
            [Value::Dict(map)] => {
                let frozen = map.borrow().clone();
                Ok(Value::FrozenDict(Rc::new(frozen)))
            }
            [Value::FrozenDict(map)] => Ok(Value::FrozenDict(map.clone())),
            _ => Err(RuntimeError::TypeError(
                "freeze() expects a dict (type mismatch)".to_string(),
            )),
        },
        "panic" => match args {
            // TODO depracated in favour of raise
            [Value::Str(msg)] => {
                eprintln!("{}", msg);
                process::exit(1);
            }
            _ => Err(RuntimeError::TypeError(
                "panic() expects a string (type mismatch)".to_string(),
            )),
        },
        "read_file" => match args {
            [Value::Str(path)] => {
                let mut path_buf = PathBuf::from(path.replace("\\", "/"));
                if path_buf.is_relative() {
                    if let Some(Value::Str(cur)) = env
                        .get("current_dir")
                        .or_else(|| globals.get("current_dir"))
                    {
                        let base = PathBuf::from(cur.replace("\\", "/"));
                        path_buf = base.join(path_buf);
                    }
                }
                match fs::read_to_string(&path_buf) {
                    Ok(content) => Ok(Value::Str(content)),
                    Err(_) => Ok(Value::Bool(false)),
                }
            }
            _ => Err(RuntimeError::TypeError(
                "read_file() expects a file path".to_string(),
            )),
        },
        "call_builtin" => match args {
            [Value::Str(inner), Value::List(list)] => {
                let inner_args = list.borrow().clone();
                call_builtin(inner, &inner_args, env, globals)
            }
            _ => Err(RuntimeError::TypeError(
                "call_builtin() expects a name and argument list".to_string(),
            )),
        },
        _ => Err(RuntimeError::TypeError(format!(
            "unknown builtin: {}",
            name
        ))),
    }
}

/// Execute bytecode on a stack-based virtual machine.
pub fn run(
    code: &[Instr],
    funcs: &HashMap<String, Function>,
    program_args: &[String],
) -> Result<(), RuntimeError> {
    let mut stack: Vec<Value> = Vec::new();
    let mut globals: HashMap<String, Value> = HashMap::new();
    // Expose command line arguments to bytecode programs via the global `args` list
    let arg_values: Vec<Value> = program_args.iter().map(|s| Value::Str(s.clone())).collect();
    globals.insert(
        "args".to_string(),
        Value::List(Rc::new(RefCell::new(arg_values))),
    );
    if let Some(first) = program_args.first() {
        let path = PathBuf::from(first.replace("\\", "/"));
        globals.insert(
            "module_file".to_string(),
            Value::Str(path.to_string_lossy().replace("\\", "/")),
        );
        if let Some(parent) = path.parent() {
            globals.insert(
                "current_dir".to_string(),
                Value::Str(parent.to_string_lossy().replace("\\", "/")),
            );
        } else {
            globals.insert("current_dir".to_string(), Value::Str(".".to_string()));
        }
    } else {
        globals.insert("module_file".to_string(), Value::Str("<stdin>".to_string()));
        globals.insert("current_dir".to_string(), Value::Str(".".to_string()));
    }
    let mut env: HashMap<String, Value> = HashMap::new();
    let mut env_stack: Vec<HashMap<String, Value>> = Vec::new();
    let mut ret_stack: Vec<usize> = Vec::new();
    let mut pc: usize = 0;
    let mut block_stack: Vec<Block> = Vec::new();
    let mut error_flag: Option<RuntimeError> = None;
    while pc < code.len() {
        let mut advance_pc = true;
        let instr_res: Result<(), RuntimeError> = loop {
            match &code[pc] {
                Instr::PushInt(v) => stack.push(Value::Int(*v)),
                Instr::PushStr(s) => stack.push(Value::Str(s.clone())),
                Instr::PushBool(b) => stack.push(Value::Bool(*b)),
                Instr::BuildList(n) => {
                    let mut elements = Vec::new();
                    for _ in 0..*n {
                        elements.push(pop(&mut stack)?);
                    }
                    elements.reverse();
                    stack.push(Value::List(Rc::new(RefCell::new(elements))));
                }
                Instr::BuildDict(n) => {
                    let mut map: HashMap<String, Value> = HashMap::new();
                    for _ in 0..*n {
                        let val = pop(&mut stack)?;
                        let key = pop(&mut stack)?.to_string();
                        map.insert(key, val);
                    }
                    stack.push(Value::Dict(Rc::new(RefCell::new(map))));
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
                    if let Err(e) = ops_arith::handle_add(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Sub => {
                    if let Err(e) = ops_arith::handle_sub(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Mul => {
                    if let Err(e) = ops_arith::handle_mul(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Div => {
                    if let Err(e) = ops_arith::handle_div(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Mod => {
                    if let Err(e) = ops_arith::handle_mod(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Eq => {
                    if let Err(e) = ops_arith::handle_eq(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Ne => {
                    if let Err(e) = ops_arith::handle_ne(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Lt => {
                    if let Err(e) = ops_arith::handle_lt(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Le => {
                    if let Err(e) = ops_arith::handle_le(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Gt => {
                    if let Err(e) = ops_arith::handle_gt(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Ge => {
                    if let Err(e) = ops_arith::handle_ge(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::BAnd => {
                    if let Err(e) = ops_arith::handle_band(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::BOr => {
                    if let Err(e) = ops_arith::handle_bor(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::BXor => {
                    if let Err(e) = ops_arith::handle_bxor(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Shl => {
                    if let Err(e) = ops_arith::handle_shl(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Shr => {
                    if let Err(e) = ops_arith::handle_shr(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::And => {
                    if let Err(e) = ops_arith::handle_and(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Or => {
                    if let Err(e) = ops_arith::handle_or(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Not => {
                    if let Err(e) = ops_arith::handle_not(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Neg => {
                    if let Err(e) = ops_arith::handle_neg(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Index => {
                    if let Err(e) = ops_struct::handle_index(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Slice => {
                    if let Err(e) = ops_struct::handle_slice(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::StoreIndex => {
                    if let Err(e) = ops_struct::handle_store_index(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Attr(attr) => {
                    if let Err(e) = ops_struct::handle_attr(&mut stack, attr) {
                        break Err(e);
                    }
                }
                Instr::StoreAttr(attr) => {
                    if let Err(e) = ops_struct::handle_store_attr(&mut stack, attr) {
                        break Err(e);
                    }
                }
                Instr::Assert => {
                    if let Err(e) = ops_control::handle_assert(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::CallValue(argc) => {
                    if let Err(e) = ops_control::handle_call_value(
                        *argc,
                        &mut stack,
                        &funcs,
                        &mut env_stack,
                        &mut ret_stack,
                        &mut env,
                        &mut pc,
                        &mut advance_pc,
                    ) {
                        break Err(e);
                    }
                }
                Instr::PushNone => ops_control::handle_push_none(&mut stack),
                Instr::Jump(target) => ops_control::handle_jump(&mut pc, *target, &mut advance_pc),
                Instr::JumpIfFalse(target) => {
                    if let Err(e) = ops_control::handle_jump_if_false(
                        &mut stack,
                        &mut pc,
                        *target,
                        &mut advance_pc,
                    ) {
                        break Err(e);
                    }
                }
                Instr::Call(name) => {
                    if let Err(e) = ops_control::handle_call(
                        name,
                        &funcs,
                        &mut stack,
                        &mut env,
                        &mut env_stack,
                        &mut ret_stack,
                        &mut pc,
                        &mut advance_pc,
                    ) {
                        break Err(e);
                    }
                }
                Instr::TailCall(name) => {
                    if let Err(e) = ops_control::handle_tail_call(
                        name,
                        &funcs,
                        &mut stack,
                        &mut env,
                        &mut pc,
                        &mut advance_pc,
                    ) {
                        break Err(e);
                    }
                }
                Instr::CallBuiltin(name, argc) => {
                    if let Err(e) = ops_control::handle_call_builtin(
                        name,
                        *argc,
                        &mut stack,
                        &env,
                        &globals,
                    ) {
                        break Err(e);
                    }
                }
                Instr::Pop => ops_control::handle_pop(&mut stack),
                Instr::Ret => ops_control::handle_ret(
                    &mut stack,
                    &mut ret_stack,
                    &mut env_stack,
                    &mut env,
                    &mut pc,
                    &mut advance_pc,
                ),
                Instr::Emit => ops_control::handle_emit(&mut stack),
                Instr::Halt => ops_control::handle_halt(&mut pc, code.len(), &mut advance_pc),
                Instr::SetupExcept(target) => ops_control::handle_setup_except(
                    &mut block_stack,
                    *target,
                    stack.len(),
                    env_stack.len(),
                    ret_stack.len(),
                ),
                Instr::PopBlock => ops_control::handle_pop_block(&mut block_stack),
                Instr::Raise(kind) => {
                    if let Err(e) = ops_control::handle_raise(&mut stack, *kind) {
                        break Err(e);
                    }
                }
            }
            break Ok(());
        };

        if let Err(e) = instr_res {
            error_flag = Some(e);
        }

        if let Some(err) = error_flag.take() {
            let mut handled = false;
            while let Some(block) = block_stack.pop() {
                while env_stack.len() > block.env_depth {
                    env = env_stack.pop().unwrap();
                    ret_stack.pop();
                }
                ret_stack.truncate(block.ret_depth);
                stack.truncate(block.stack_size);
                pc = block.handler;
                stack.push(Value::Str(err.to_string()));
                handled = true;
                break;
            }
            if !handled {
                return Err(err);
            } else {
                continue;
            }
        }

        if advance_pc {
            pc += 1;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests;
