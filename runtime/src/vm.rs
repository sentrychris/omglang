use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::rc::Rc;

use crate::bytecode::{Function, Instr};
use crate::error::RuntimeError;
use crate::value::Value;

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
                stack.push(Value::List(Rc::new(RefCell::new(elements))));
            }
            Instr::BuildDict(n) => {
                let mut map: HashMap<String, Value> = HashMap::new();
                for _ in 0..*n {
                    let val = stack.pop().unwrap();
                    let key = stack.pop().unwrap().to_string();
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
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
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
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Int(a - b));
            }
            Instr::Mul => {
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Int(a.checked_mul(b).unwrap_or(0)));
            }
            Instr::Div => {
                let b = stack.pop().unwrap().as_int();
                if b == 0 {
                    return Err(RuntimeError::ZeroDivisionError);
                }
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Int(a / b));
            }
            Instr::Mod => {
                let b = stack.pop().unwrap().as_int();
                if b == 0 {
                    return Err(RuntimeError::ZeroDivisionError);
                }
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
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let res = match (&a, &b) {
                    (Value::Str(sa), Value::Str(sb)) => sa < sb,
                    _ => a.as_int() < b.as_int(),
                };
                stack.push(Value::Bool(res));
            }
            Instr::Le => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let res = match (&a, &b) {
                    (Value::Str(sa), Value::Str(sb)) => sa <= sb,
                    _ => a.as_int() <= b.as_int(),
                };
                stack.push(Value::Bool(res));
            }
            Instr::Gt => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let res = match (&a, &b) {
                    (Value::Str(sa), Value::Str(sb)) => sa > sb,
                    _ => a.as_int() > b.as_int(),
                };
                stack.push(Value::Bool(res));
            }
            Instr::Ge => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let res = match (&a, &b) {
                    (Value::Str(sa), Value::Str(sb)) => sa >= sb,
                    _ => a.as_int() >= b.as_int(),
                };
                stack.push(Value::Bool(res));
            }
            Instr::BAnd => {
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Int(a & b));
            }
            Instr::BOr => {
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Int(a | b));
            }
            Instr::BXor => {
                let b = stack.pop().unwrap().as_int();
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Int(a ^ b));
            }
            Instr::Shl => {
                let b = stack.pop().unwrap().as_int() as u32;
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Int(a << b));
            }
            Instr::Shr => {
                let b = stack.pop().unwrap().as_int() as u32;
                let a = stack.pop().unwrap().as_int();
                stack.push(Value::Int(a >> b));
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
                let v = stack.pop().unwrap().as_int();
                stack.push(Value::Int(!v));
            }
            Instr::Neg => {
                let v = stack.pop().unwrap().as_int();
                stack.push(Value::Int(-v));
            }
            Instr::Index => {
                let idx = stack.pop().unwrap();
                let base = stack.pop().unwrap();
                match (base, idx) {
                    (Value::List(list), Value::Int(i)) => {
                        if i < 0 {
                            return Err(RuntimeError::IndexError(
                                "List index out of bounds!".to_string(),
                            ));
                        }
                        let l = list.borrow();
                        let idx_usize = i as usize;
                        if idx_usize < l.len() {
                            stack.push(l[idx_usize].clone());
                        } else {
                            return Err(RuntimeError::IndexError(
                                "List index out of bounds!".to_string(),
                            ));
                        }
                    }
                    (Value::Dict(map), Value::Str(k)) => {
                        if let Some(v) = map.borrow().get(&k).cloned() {
                            stack.push(v);
                        } else {
                            return Err(RuntimeError::KeyError(k));
                        }
                    }
                    (Value::Dict(map), Value::Int(i)) => {
                        let key = i.to_string();
                        if let Some(v) = map.borrow().get(&key).cloned() {
                            stack.push(v);
                        } else {
                            return Err(RuntimeError::KeyError(key));
                        }
                    }
                    (Value::FrozenDict(map), Value::Str(k)) => {
                        if let Some(v) = map.get(&k).cloned() {
                            stack.push(v);
                        } else {
                            return Err(RuntimeError::KeyError(k));
                        }
                    }
                    (Value::FrozenDict(map), Value::Int(i)) => {
                        let key = i.to_string();
                        if let Some(v) = map.get(&key).cloned() {
                            stack.push(v);
                        } else {
                            return Err(RuntimeError::KeyError(key));
                        }
                    }
                    (Value::Str(s), Value::Int(i)) => {
                        if i < 0 {
                            return Err(RuntimeError::IndexError(
                                "String index out of bounds!".to_string(),
                            ));
                        }
                        let chars: Vec<char> = s.chars().collect();
                        let idx_usize = i as usize;
                        if idx_usize < chars.len() {
                            stack.push(Value::Str(chars[idx_usize].to_string()));
                        } else {
                            return Err(RuntimeError::IndexError(
                                "String index out of bounds!".to_string(),
                            ));
                        }
                    }
                    (other, _) => {
                        return Err(RuntimeError::TypeError(format!(
                            "{} is not indexable",
                            other.to_string()
                        )));
                    }
                }
            }
            Instr::Slice => {
                let end_val = stack.pop().unwrap();
                let start = stack.pop().unwrap().as_int() as usize;
                let base = stack.pop().unwrap();
                match base {
                    Value::List(list) => {
                        let list_ref = list.borrow();
                        let end = match end_val {
                            Value::None => list_ref.len(),
                            v => v.as_int() as usize,
                        };
                        let slice = list_ref[start..end].to_vec();
                        stack.push(Value::List(Rc::new(RefCell::new(slice))));
                    }
                    Value::Str(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        let end = match end_val {
                            Value::None => chars.len(),
                            v => v.as_int() as usize,
                        };
                        let slice: String = chars[start..end].iter().collect();
                        stack.push(Value::Str(slice));
                    }
                    _ => stack.push(Value::Int(0)),
                }
            }
            Instr::StoreIndex => {
                let val = stack.pop().unwrap();
                let idx = stack.pop().unwrap();
                let base = stack.pop().unwrap();
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
                        return Err(RuntimeError::FrozenWriteError);
                    }
                    _ => {}
                }
            }
            Instr::Attr(attr) => {
                let base = stack.pop().unwrap();
                match base {
                    Value::Dict(map) => {
                        if let Some(v) = map.borrow().get(attr).cloned() {
                            stack.push(v);
                        } else {
                            return Err(RuntimeError::KeyError(attr.clone()));
                        }
                    }
                    Value::FrozenDict(map) => {
                        if let Some(v) = map.get(attr).cloned() {
                            stack.push(v);
                        } else {
                            return Err(RuntimeError::KeyError(attr.clone()));
                        }
                    }
                    other => {
                        return Err(RuntimeError::TypeError(format!(
                            "{} has no attribute '{}'",
                            other.to_string(),
                            attr
                        )));
                    }
                }
            }
            Instr::StoreAttr(attr) => {
                let val = stack.pop().unwrap();
                let base = stack.pop().unwrap();
                match base {
                    Value::Dict(map) => {
                        map.borrow_mut().insert(attr.clone(), val);
                    }
                    Value::FrozenDict(_) => {
                        return Err(RuntimeError::FrozenWriteError);
                    }
                    _ => {}
                }
            }
            Instr::Assert => {
                let cond = stack.pop().unwrap().as_bool();
                if !cond {
                    panic!("Assertion failed");
                }
            }
            Instr::CallValue(argc) => {
                let mut args_vec: Vec<Value> = Vec::new();
                for _ in 0..*argc {
                    args_vec.push(stack.pop().unwrap());
                }
                args_vec.reverse();
                let func_val = stack.pop().unwrap();
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
                        continue;
                    } else {
                        panic!("Unknown function: {}", name);
                    }
                } else {
                    panic!("CALL_VALUE expects function name");
                }
            }
            Instr::PushNone => {
                stack.push(Value::None);
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
                    "length" => {
                        if args.len() != 1 {
                            panic!("length() expects one positional argument");
                        }
                        match &args[0] {
                            Value::List(list) => Value::Int(list.borrow().len() as i64),
                            Value::Str(s) => Value::Int(s.chars().count() as i64),
                            _ => panic!("length() expects list or string"),
                        }
                    },
                    "freeze" => match args.as_slice() {
                        [Value::Dict(map)] => {
                            let frozen = map.borrow().clone();
                            Value::FrozenDict(Rc::new(frozen))
                        }
                        [Value::FrozenDict(map)] => Value::FrozenDict(map.clone()),
                        _ => panic!("freeze() expects a dict"),
                    },
                    "panic" => match args.as_slice() {
                        [Value::Str(msg)] => {
                            eprintln!("{}", msg);
                            process::exit(1);
                        }
                        _ => panic!("panic() expects a string"),
                    },
                    "read_file" => match args.as_slice() {
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
                                Ok(content) => Value::Str(content),
                                Err(_) => Value::Bool(false),
                            }
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
    Ok(())
}
#[cfg(test)]
mod tests;
