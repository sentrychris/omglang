//! Built-in function dispatch for the OMG VM.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{
    atomic::{AtomicI32, Ordering},
    Mutex,
};

use once_cell::sync::Lazy;

use super::ops_control;
use crate::error::{ErrorKind, RuntimeError};
use crate::value::Value;

struct FileEntry {
    file: fs::File,
    binary: bool,
}

static FILE_HANDLES: Lazy<Mutex<HashMap<i32, FileEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static NEXT_FD: AtomicI32 = AtomicI32::new(0);

fn resolve_path(
    path: &str,
    env: &HashMap<String, Value>,
    globals: &HashMap<String, Value>,
) -> PathBuf {
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
    path_buf
}

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
            [Value::Str(msg)] => Err(RuntimeError::Raised(msg.clone())),
            _ => Err(RuntimeError::TypeError(
                "panic() expects a string (type mismatch)".to_string(),
            )),
        },
        "raise" => match args {
            [Value::Str(msg)] => {
                let mut stack = vec![Value::Str(msg.clone())];
                ops_control::handle_raise(&ErrorKind::Generic, &mut stack)?;
                unreachable!()
            }
            _ => Err(RuntimeError::TypeError(
                "raise() expects a string (type mismatch)".to_string(),
            )),
        },
        "read_file" => match args {
            [Value::Str(path)] => {
                let path_buf = resolve_path(path, env, globals);
                match fs::read_to_string(&path_buf) {
                    Ok(content) => Ok(Value::Str(content)),
                    Err(err) => Err(RuntimeError::ModuleImportError(format!(
                        "failed to read '{}': {}",
                        path_buf.display(),
                        err
                    ))),
                }
            }
            _ => Err(RuntimeError::TypeError(
                "read_file() expects a file path".to_string(),
            )),
        },
        "write_file" => match args {
            [Value::Str(path), Value::Str(data)] => {
                let path_buf = resolve_path(path, env, globals);
                fs::write(&path_buf, data.as_bytes())
                    .map_err(|e| RuntimeError::ValueError(e.to_string()))?;
                Ok(Value::Int(data.as_bytes().len() as i64))
            }
            [Value::Str(path), Value::List(list)] => {
                let path_buf = resolve_path(path, env, globals);
                let vec = list
                    .borrow()
                    .iter()
                    .map(|v| match v {
                        Value::Int(i) if *i >= 0 && *i <= 255 => Ok(*i as u8),
                        _ => Err(RuntimeError::TypeError(
                            "write_file() expects bytes 0-255".to_string(),
                        )),
                    })
                    .collect::<Result<Vec<u8>, RuntimeError>>()?;
                fs::write(&path_buf, &vec).map_err(|e| RuntimeError::ValueError(e.to_string()))?;
                Ok(Value::Int(vec.len() as i64))
            }
            _ => Err(RuntimeError::TypeError(
                "write_file() expects path and data".to_string(),
            )),
        },
        "file_open" => match args {
            [Value::Str(path), Value::Str(mode)] => {
                let path_buf = resolve_path(path, env, globals);
                let mut opts = OpenOptions::new();
                let binary = mode.contains('b');
                match mode.as_str() {
                    "r" | "rb" => {
                        opts.read(true);
                    }
                    "w" | "wb" => {
                        opts.write(true).create(true).truncate(true);
                    }
                    "a" | "ab" => {
                        opts.write(true).create(true).append(true);
                    }
                    _ => {
                        return Err(RuntimeError::ValueError("invalid file mode".to_string()));
                    }
                }
                match opts.open(&path_buf) {
                    Ok(file) => {
                        let handle = NEXT_FD.fetch_add(1, Ordering::SeqCst);
                        FILE_HANDLES
                            .lock()
                            .unwrap()
                            .insert(handle, FileEntry { file, binary });
                        Ok(Value::Int(handle as i64))
                    }
                    Err(err) => Err(RuntimeError::ValueError(format!(
                        "cannot open '{}': {}",
                        path_buf.display(),
                        err
                    ))),
                }
            }
            _ => Err(RuntimeError::TypeError(
                "file_open() expects path and mode".to_string(),
            )),
        },
        "file_read" => match args {
            [Value::Int(handle)] => {
                let mut table = FILE_HANDLES.lock().unwrap();
                if let Some(entry) = table.get_mut(&(*handle as i32)) {
                    if entry.binary {
                        let mut buf = Vec::new();
                        entry
                            .file
                            .read_to_end(&mut buf)
                            .map_err(|e| RuntimeError::ValueError(e.to_string()))?;
                        let list: Vec<Value> =
                            buf.into_iter().map(|b| Value::Int(b as i64)).collect();
                        Ok(Value::List(Rc::new(RefCell::new(list))))
                    } else {
                        let mut s = String::new();
                        entry
                            .file
                            .read_to_string(&mut s)
                            .map_err(|e| RuntimeError::ValueError(e.to_string()))?;
                        Ok(Value::Str(s))
                    }
                } else {
                    Err(RuntimeError::ValueError("invalid file handle".to_string()))
                }
            }
            _ => Err(RuntimeError::TypeError(
                "file_read() expects a handle".to_string(),
            )),
        },
        "file_write" => match args {
            [Value::Int(handle), Value::Str(data)] => {
                let mut table = FILE_HANDLES.lock().unwrap();
                if let Some(entry) = table.get_mut(&(*handle as i32)) {
                    if entry.binary {
                        return Err(RuntimeError::TypeError(
                            "file_write() binary handle expects list".to_string(),
                        ));
                    }
                    entry
                        .file
                        .write_all(data.as_bytes())
                        .map_err(|e| RuntimeError::ValueError(e.to_string()))?;
                    Ok(Value::Int(data.as_bytes().len() as i64))
                } else {
                    Err(RuntimeError::ValueError("invalid file handle".to_string()))
                }
            }
            [Value::Int(handle), Value::List(list)] => {
                let mut table = FILE_HANDLES.lock().unwrap();
                if let Some(entry) = table.get_mut(&(*handle as i32)) {
                    if !entry.binary {
                        return Err(RuntimeError::TypeError(
                            "file_write() text handle expects string".to_string(),
                        ));
                    }
                    let vec = list
                        .borrow()
                        .iter()
                        .map(|v| match v {
                            Value::Int(i) if *i >= 0 && *i <= 255 => Ok(*i as u8),
                            _ => Err(RuntimeError::TypeError(
                                "file_write() expects bytes 0-255".to_string(),
                            )),
                        })
                        .collect::<Result<Vec<u8>, RuntimeError>>()?;
                    entry
                        .file
                        .write_all(&vec)
                        .map_err(|e| RuntimeError::ValueError(e.to_string()))?;
                    Ok(Value::Int(vec.len() as i64))
                } else {
                    Err(RuntimeError::ValueError("invalid file handle".to_string()))
                }
            }
            _ => Err(RuntimeError::TypeError(
                "file_write() expects handle and data".to_string(),
            )),
        },
        "file_close" => match args {
            [Value::Int(handle)] => {
                let mut table = FILE_HANDLES.lock().unwrap();
                if table.remove(&(*handle as i32)).is_some() {
                    Ok(Value::None)
                } else {
                    Err(RuntimeError::ValueError("invalid file handle".to_string()))
                }
            }
            _ => Err(RuntimeError::TypeError(
                "file_close() expects handle".to_string(),
            )),
        },
        "file_exists" => match args {
            [Value::Str(path)] => {
                let path_buf = resolve_path(path, env, globals);
                Ok(Value::Bool(path_buf.exists()))
            }
            _ => Err(RuntimeError::TypeError(
                "file_exists() expects a path".to_string(),
            )),
        },
        "_omg_vm_syntax_error_handle" => match args {
            [Value::Str(msg)] => {
                let mut stack = vec![Value::Str(msg.clone())];
                ops_control::handle_raise(&ErrorKind::Syntax, &mut stack)?;
                unreachable!()
            }
            _ => Err(RuntimeError::TypeError(
                "_omg_vm_syntax_error_handle() expects a string".to_string(),
            )),
        },
        "_omg_vm_type_error_handle" => match args {
            [Value::Str(msg)] => {
                let mut stack = vec![Value::Str(msg.clone())];
                ops_control::handle_raise(&ErrorKind::Type, &mut stack)?;
                unreachable!()
            }
            _ => Err(RuntimeError::TypeError(
                "_omg_vm_type_error_handle() expects a string".to_string(),
            )),
        },
        "_omg_vm_undef_ident_error_handle" => match args {
            [Value::Str(msg)] => {
                let mut stack = vec![Value::Str(msg.clone())];
                ops_control::handle_raise(&ErrorKind::UndefinedIdent, &mut stack)?;
                unreachable!()
            }
            _ => Err(RuntimeError::TypeError(
                "_omg_vm_undef_ident_error_handle() expects a string".to_string(),
            )),
        },
        "_omg_vm_value_error_handle" => match args {
            [Value::Str(msg)] => {
                let mut stack = vec![Value::Str(msg.clone())];
                ops_control::handle_raise(&ErrorKind::Value, &mut stack)?;
                unreachable!()
            }
            _ => Err(RuntimeError::TypeError(
                "_omg_vm_value_error_handle() expects a string".to_string(),
            )),
        },
        "_omg_vm_module_import_error_handle" => match args {
            [Value::Str(msg)] => {
                let mut stack = vec![Value::Str(msg.clone())];
                ops_control::handle_raise(&ErrorKind::ModuleImport, &mut stack)?;
                unreachable!()
            }
            _ => Err(RuntimeError::TypeError(
                "_omg_vm_module_import_error_handle() expects a string".to_string(),
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
