use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::rc::Rc;

use crate::bytecode::{Function, Instr};
use crate::error::RuntimeError;
use crate::value::Value;

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
                    let b = pop(&mut stack)?;
                    let a = pop(&mut stack)?;
                    match (a, b) {
                        (Value::Str(sa), Value::Str(sb)) => stack.push(Value::Str(sa + &sb)),
                        (Value::Str(sa), v) => stack.push(Value::Str(sa + &v.to_string())),
                        (v, Value::Str(sb)) => stack.push(Value::Str(v.to_string() + &sb)),
                        (Value::List(la), Value::List(lb)) => {
                            {
                                let mut la_mut = la.borrow_mut();
                                la_mut.extend(lb.borrow().iter().cloned());
                            }
                            stack.push(Value::List(la));
                        }
                        (a, b) => stack.push(Value::Int(a.as_int() + b.as_int())),
                    }
                }
                Instr::Sub => {
                    let b = pop(&mut stack)?.as_int();
                    let a = pop(&mut stack)?.as_int();
                    stack.push(Value::Int(a - b));
                }
                Instr::Mul => {
                    let b = pop(&mut stack)?.as_int();
                    let a = pop(&mut stack)?.as_int();
                    stack.push(Value::Int(a.checked_mul(b).unwrap_or(0)));
                }
                Instr::Div => {
                    let b = pop(&mut stack)?.as_int();
                    if b == 0 {
                        break Err(RuntimeError::ZeroDivisionError);
                    }
                    let a = pop(&mut stack)?.as_int();
                    stack.push(Value::Int(a / b));
                }
                Instr::Mod => {
                    let b = pop(&mut stack)?.as_int();
                    if b == 0 {
                        break Err(RuntimeError::ZeroDivisionError);
                    }
                    let a = pop(&mut stack)?.as_int();
                    stack.push(Value::Int(a % b));
                }
                Instr::Eq => {
                    let b = pop(&mut stack)?.to_string();
                    let a = pop(&mut stack)?.to_string();
                    stack.push(Value::Bool(a == b));
                }
                Instr::Ne => {
                    let b = pop(&mut stack)?.to_string();
                    let a = pop(&mut stack)?.to_string();
                    stack.push(Value::Bool(a != b));
                }
                Instr::Lt => {
                    let b = pop(&mut stack)?;
                    let a = pop(&mut stack)?;
                    let res = match (&a, &b) {
                        (Value::Str(sa), Value::Str(sb)) => sa < sb,
                        _ => a.as_int() < b.as_int(),
                    };
                    stack.push(Value::Bool(res));
                }
                Instr::Le => {
                    let b = pop(&mut stack)?;
                    let a = pop(&mut stack)?;
                    let res = match (&a, &b) {
                        (Value::Str(sa), Value::Str(sb)) => sa <= sb,
                        _ => a.as_int() <= b.as_int(),
                    };
                    stack.push(Value::Bool(res));
                }
                Instr::Gt => {
                    let b = pop(&mut stack)?;
                    let a = pop(&mut stack)?;
                    let res = match (&a, &b) {
                        (Value::Str(sa), Value::Str(sb)) => sa > sb,
                        _ => a.as_int() > b.as_int(),
                    };
                    stack.push(Value::Bool(res));
                }
                Instr::Ge => {
                    let b = pop(&mut stack)?;
                    let a = pop(&mut stack)?;
                    let res = match (&a, &b) {
                        (Value::Str(sa), Value::Str(sb)) => sa >= sb,
                        _ => a.as_int() >= b.as_int(),
                    };
                    stack.push(Value::Bool(res));
                }
                Instr::BAnd => {
                    let b = pop(&mut stack)?.as_int();
                    let a = pop(&mut stack)?.as_int();
                    stack.push(Value::Int(a & b));
                }
                Instr::BOr => {
                    let b = pop(&mut stack)?.as_int();
                    let a = pop(&mut stack)?.as_int();
                    stack.push(Value::Int(a | b));
                }
                Instr::BXor => {
                    let b = pop(&mut stack)?.as_int();
                    let a = pop(&mut stack)?.as_int();
                    stack.push(Value::Int(a ^ b));
                }
                Instr::Shl => {
                    let b = pop(&mut stack)?.as_int() as u32;
                    let a = pop(&mut stack)?.as_int();
                    stack.push(Value::Int(a << b));
                }
                Instr::Shr => {
                    let b = pop(&mut stack)?.as_int() as u32;
                    let a = pop(&mut stack)?.as_int();
                    stack.push(Value::Int(a >> b));
                }
                Instr::And => {
                    let b = pop(&mut stack)?.as_bool();
                    let a = pop(&mut stack)?.as_bool();
                    stack.push(Value::Bool(a && b));
                }
                Instr::Or => {
                    let b = pop(&mut stack)?.as_bool();
                    let a = pop(&mut stack)?.as_bool();
                    stack.push(Value::Bool(a || b));
                }
                Instr::Not => {
                    let v = pop(&mut stack)?.as_int();
                    stack.push(Value::Int(!v));
                }
                Instr::Neg => {
                    let v = pop(&mut stack)?.as_int();
                    stack.push(Value::Int(-v));
                }
                Instr::Index => {
                    let idx = pop(&mut stack)?;
                    let base = pop(&mut stack)?;
                    match (base, idx) {
                        (Value::List(list), Value::Int(i)) => {
                            if i < 0 {
                                break Err(RuntimeError::IndexError(
                                    "List index out of bounds!".to_string(),
                                ));
                            }
                            let l = list.borrow();
                            let idx_usize = i as usize;
                            if idx_usize < l.len() {
                                stack.push(l[idx_usize].clone());
                            } else {
                                break Err(RuntimeError::IndexError(
                                    "List index out of bounds!".to_string(),
                                ));
                            }
                        }
                        (Value::Dict(map), Value::Str(k)) => {
                            if let Some(v) = map.borrow().get(&k).cloned() {
                                stack.push(v);
                            } else {
                                break Err(RuntimeError::KeyError(k));
                            }
                        }
                        (Value::Dict(map), Value::Int(i)) => {
                            let key = i.to_string();
                            if let Some(v) = map.borrow().get(&key).cloned() {
                                stack.push(v);
                            } else {
                                break Err(RuntimeError::KeyError(key));
                            }
                        }
                        (Value::FrozenDict(map), Value::Str(k)) => {
                            if let Some(v) = map.get(&k).cloned() {
                                stack.push(v);
                            } else {
                                break Err(RuntimeError::KeyError(k));
                            }
                        }
                        (Value::FrozenDict(map), Value::Int(i)) => {
                            let key = i.to_string();
                            if let Some(v) = map.get(&key).cloned() {
                                stack.push(v);
                            } else {
                                break Err(RuntimeError::KeyError(key));
                            }
                        }
                        (Value::Str(s), Value::Int(i)) => {
                            if i < 0 {
                                break Err(RuntimeError::IndexError(
                                    "String index out of bounds!".to_string(),
                                ));
                            }
                            let chars: Vec<char> = s.chars().collect();
                            let idx_usize = i as usize;
                            if idx_usize < chars.len() {
                                stack.push(Value::Str(chars[idx_usize].to_string()));
                            } else {
                                break Err(RuntimeError::IndexError(
                                    "String index out of bounds!".to_string(),
                                ));
                            }
                        }
                        (other, _) => {
                            break Err(RuntimeError::TypeError(format!(
                                "{} is not indexable",
                                other.to_string()
                            )));
                        }
                    }
                }
                Instr::Slice => {
                    let end_val = pop(&mut stack)?;
                    let start_val = pop(&mut stack)?;
                    let base = pop(&mut stack)?;
                    let start_i64 = start_val.as_int();
                    match base {
                        Value::List(list) => {
                            let list_ref = list.borrow();
                            let len = list_ref.len();
                            if start_i64 < 0 {
                                break Err(RuntimeError::IndexError(
                                    "Slice indices out of bounds!".to_string(),
                                ));
                            }
                            let start = start_i64 as usize;
                            let end_i64 = match end_val {
                                Value::None => len as i64,
                                v => v.as_int(),
                            };
                            if end_i64 < 0 {
                                break Err(RuntimeError::IndexError(
                                    "Slice indices out of bounds!".to_string(),
                                ));
                            }
                            let end = end_i64 as usize;
                            if start > end || end > len {
                                break Err(RuntimeError::IndexError(
                                    "Slice indices out of bounds!".to_string(),
                                ));
                            }
                            let slice = list_ref[start..end].to_vec();
                            stack.push(Value::List(Rc::new(RefCell::new(slice))));
                        }
                        Value::Str(s) => {
                            let chars: Vec<char> = s.chars().collect();
                            let len = chars.len();
                            if start_i64 < 0 {
                                break Err(RuntimeError::IndexError(
                                    "Slice indices out of bounds!".to_string(),
                                ));
                            }
                            let start = start_i64 as usize;
                            let end_i64 = match end_val {
                                Value::None => len as i64,
                                v => v.as_int(),
                            };
                            if end_i64 < 0 {
                                break Err(RuntimeError::IndexError(
                                    "Slice indices out of bounds!".to_string(),
                                ));
                            }
                            let end = end_i64 as usize;
                            if start > end || end > len {
                                break Err(RuntimeError::IndexError(
                                    "Slice indices out of bounds!".to_string(),
                                ));
                            }
                            let slice: String = chars[start..end].iter().collect();
                            stack.push(Value::Str(slice));
                        }
                        _ => stack.push(Value::Int(0)),
                    }
                }
                Instr::StoreIndex => {
                    let val = pop(&mut stack)?;
                    let idx = pop(&mut stack)?;
                    let base = pop(&mut stack)?;
                    match (base, idx) {
                        (Value::List(list), Value::Int(i)) => {
                            let mut l = list.borrow_mut();
                            let idx_usize = i as usize;
                            if idx_usize >= l.len() {
                                l.resize(idx_usize + 1, Value::Int(0));
                            }
                            l[idx_usize] = val;
                        }
                        (Value::Dict(map), Value::Str(k)) => {
                            map.borrow_mut().insert(k, val);
                        }
                        (Value::Dict(map), Value::Int(i)) => {
                            map.borrow_mut().insert(i.to_string(), val);
                        }
                        (Value::FrozenDict(_), _) => {
                            break Err(RuntimeError::FrozenWriteError);
                        }
                        _ => {}
                    }
                }
                Instr::Attr(attr) => {
                    let base = pop(&mut stack)?;
                    match base {
                        Value::Dict(map) => {
                            if let Some(v) = map.borrow().get(attr).cloned() {
                                stack.push(v);
                            } else {
                                break Err(RuntimeError::KeyError(attr.clone()));
                            }
                        }
                        Value::FrozenDict(map) => {
                            if let Some(v) = map.get(attr).cloned() {
                                stack.push(v);
                            } else {
                                break Err(RuntimeError::KeyError(attr.clone()));
                            }
                        }
                        other => {
                            break Err(RuntimeError::TypeError(format!(
                                "{} has no attribute '{}'",
                                other.to_string(),
                                attr
                            )));
                        }
                    }
                }
                Instr::StoreAttr(attr) => {
                    let val = pop(&mut stack)?;
                    let base = pop(&mut stack)?;
                    match base {
                        Value::Dict(map) => {
                            map.borrow_mut().insert(attr.clone(), val);
                        }
                        Value::FrozenDict(_) => {
                            break Err(RuntimeError::FrozenWriteError);
                        }
                        _ => {}
                    }
                }
                Instr::Assert => {
                    let cond = pop(&mut stack)?.as_bool();
                    if !cond {
                        break Err(RuntimeError::AssertionError);
                    }
                }
                Instr::CallValue(argc) => {
                    let mut args_vec: Vec<Value> = Vec::new();
                    for _ in 0..*argc {
                        args_vec.push(pop(&mut stack)?);
                    }
                    args_vec.reverse();
                    let func_val = pop(&mut stack)?;
                    if let Value::Str(name) = func_val {
                        if let Some(func) = funcs.get(&name) {
                            let mut new_env = HashMap::new();
                            for param in func.params.iter().rev() {
                                let arg = args_vec.pop().unwrap();
                                new_env.insert(param.clone(), arg);
                            }
                            env_stack.push(env);
                            ret_stack.push(pc + 1);
                            env = new_env;
                            pc = func.address;
                            advance_pc = false;
                        } else {
                            break Err(RuntimeError::UndefinedIdentError(name));
                        }
                    } else {
                        break Err(RuntimeError::TypeError(
                            "Call value expects function name".to_string(),
                        ));
                    }
                }
                Instr::PushNone => {
                    stack.push(Value::None);
                }
                Instr::Jump(target) => {
                    pc = *target;
                    advance_pc = false;
                }
                Instr::JumpIfFalse(target) => {
                    let cond = pop(&mut stack)?.as_bool();
                    if !cond {
                        pc = *target;
                        advance_pc = false;
                    }
                }
                Instr::Call(name) => {
                    if let Some(func) = funcs.get(name) {
                        let mut new_env = HashMap::new();
                        for param in func.params.iter().rev() {
                            let arg = pop(&mut stack)?;
                            new_env.insert(param.clone(), arg);
                        }
                        env_stack.push(env);
                        ret_stack.push(pc + 1);
                        env = new_env;
                        pc = func.address;
                        advance_pc = false;
                    } else {
                        break Err(RuntimeError::UndefinedIdentError(name.clone()));
                    }
                }
                Instr::TailCall(name) => {
                    if let Some(func) = funcs.get(name) {
                        let mut new_env = HashMap::new();
                        for param in func.params.iter().rev() {
                            let arg = pop(&mut stack)?;
                            new_env.insert(param.clone(), arg);
                        }
                        env = new_env;
                        pc = func.address;
                        advance_pc = false;
                    } else {
                        break Err(RuntimeError::UndefinedIdentError(name.clone()));
                    }
                }
                Instr::CallBuiltin(name, argc) => {
                    let mut args: Vec<Value> = Vec::new();
                    for _ in 0..*argc {
                        args.push(pop(&mut stack)?);
                    }
                    args.reverse();
                    match call_builtin(name, &args, &env, &globals) {
                        Ok(val) => stack.push(val),
                        Err(e) => break Err(e),
                    }
                }
                Instr::Pop => {
                    stack.pop();
                }
                Instr::Ret => {
                    let ret_val = stack.pop().unwrap_or(Value::Int(0));
                    pc = ret_stack.pop().unwrap();
                    env = env_stack.pop().unwrap();
                    stack.push(ret_val);
                    advance_pc = false;
                }
                Instr::Emit => {
                    if let Some(v) = stack.pop() {
                        println!("{}", v.to_string());
                    }
                }
                Instr::Halt => {
                    pc = code.len();
                    advance_pc = false;
                }
                Instr::SetupExcept(target) => {
                    block_stack.push(Block {
                        handler: *target,
                        stack_size: stack.len(),
                        env_depth: env_stack.len(),
                        ret_depth: ret_stack.len(),
                    });
                }
                Instr::PopBlock => {
                    block_stack.pop();
                }
                Instr::Raise(kind) => {
                    let msg_val = match stack.pop() {
                        Some(v) => v,
                        None => {
                            break Err(RuntimeError::VmInvariant(
                                "stack underflow on RAISE".to_string(),
                            ))
                        }
                    };
                    let msg = msg_val.to_string();
                    break Err(kind.into_runtime(msg));
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
