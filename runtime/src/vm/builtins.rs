//! Built-in function dispatch for the OMG VM.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process;
use std::rc::Rc;

use crate::error::RuntimeError;
use crate::value::Value;

/// Call a built-in function by name.
pub fn call_builtin(
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