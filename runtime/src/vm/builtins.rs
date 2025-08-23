//! # Built-in function dispatch for the OMG VM
//!
//! This module exposes the runtime’s **standard library**—a collection of
//! built-in functions callable from OMG programs via `CallBuiltin` and
//! `ops_control::handle_call_builtin`.
//!
//! ## Design highlights
//! - **Pure function style:** Each builtin takes arguments as `Value`s and
//!   returns a `Value` or a `RuntimeError`.
//! - **No direct VM coupling:** Builtins don’t read VM registers; any state
//!   needed is passed in explicitly (e.g., `env`, `globals`) or stored in this
//!   module (like file handles).
//! - **Filesystem helpers:** Relative paths are resolved against `current_dir`
//!   (from the current env or globals) to keep script behavior predictable.
//! - **File I/O table:** Simple integer file descriptors (`i32`) map to
//!   open files. Access is synchronized for thread-safety.
//!
//! ## Provided builtins (summary)
//! - **Data / conversion:** `chr`, `ascii`, `hex`, `binary`, `length`, `freeze`
//! - **Errors:** `panic`, `raise`
//! - **Filesystem:** `read_file`, `file_exists`
//! - **File descriptors:** `file_open`, `file_read`, `file_write`, `file_close`
//! - **Meta:** `call_builtin` (dispatch another builtin dynamically)
//!
//! ## Error conventions
//! - Arity/type mismatches → `RuntimeError::TypeError`
//! - Value problems (e.g., bad width, invalid file mode) → `RuntimeError::ValueError`
//! - IO failures → mapped to `ValueError` or `ModuleImportError` (for `read_file`,
//!   since it’s commonly used by import loaders)
//! - `raise()` manufactures a `RuntimeError` via the VM’s raise handler
//!
//! ## Notes on text vs binary I/O
//! - `file_open(path, "r"|"w"|"a")` → **text** (UTF‑8 strings)
//! - `file_open(path, "rb"|"wb"|"ab")` → **binary** (list of byte ints 0–255)
//! - `file_write` enforces the correct data type for the handle kind.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{atomic::{AtomicI32, Ordering}, Mutex};

use once_cell::sync::Lazy;

use super::ops_control;
use crate::error::{ErrorKind, RuntimeError};
use crate::value::Value;

/// Entry in the in-process file descriptor table.
struct FileEntry {
    file: fs::File,
    /// Whether this handle is opened in **binary** mode (`rb`, `wb`, `ab`).
    binary: bool,
}

/// Global FD table. A simple, process-local registry mapping `i32` handles to open files.
/// Wrapped in a `Mutex` to be usable from multiple threads safely.
static FILE_HANDLES: Lazy<Mutex<HashMap<i32, FileEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Monotonic counter to allocate new integer file descriptors.
static NEXT_FD: AtomicI32 = AtomicI32::new(0);

/// Resolve a user-supplied path relative to `current_dir` (env or globals).
///
/// The VM injects `current_dir` and `module_file` globals/locals on program start.
/// If `path` is relative, we join it against `current_dir`. Backslashes are
/// normalized to forward slashes for portability.
fn resolve_path(path: &str, env: &HashMap<String, Value>, globals: &HashMap<String, Value>) -> PathBuf {
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

/// Dispatch a built-in function by name.
///
/// * `name`  – builtin identifier (e.g. `"length"`, `"file_open"`)  
/// * `args`  – positional arguments as already-evaluated `Value`s  
/// * `env`   – current local environment (for `current_dir`)  
/// * `globals` – global environment (fallback for `current_dir`)
///
/// Returns a `Value` on success or a `RuntimeError` on failure.
pub fn call_builtin(
    name: &str,
    args: &[Value],
    env: &HashMap<String, Value>,
    globals: &HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match name {
        // --- Data / conversion ------------------------------------------------

        // chr(i64) -> single-character string (low 8 bits)
        "chr" => match args {
            [Value::Int(i)] => Ok(Value::Str((*i as u8 as char).to_string())),
            _ => Err(RuntimeError::TypeError(
                "chr() expects one integer".to_string(),
            )),
        },

        // ascii("c") -> integer code point (requires exactly one character)
        "ascii" => match args {
            [Value::Str(s)] if s.chars().count() == 1 => {
                Ok(Value::Int(s.chars().next().unwrap() as i64))
            }
            _ => Err(RuntimeError::TypeError(
                "ascii() expects a single character (arity mismatch)".to_string(),
            )),
        },

        // hex(i64) -> lowercase hex string
        "hex" => match args {
            [Value::Int(i)] => Ok(Value::Str(format!("{:x}", i))),
            _ => Err(RuntimeError::TypeError(
                "hex() expects one integer (arity mismatch)".to_string(),
            )),
        },

        // binary(n[, width]) -> binary string; with width, mask & zero-pad
        "binary" => match args {
            [Value::Int(n)] => Ok(Value::Str(format!("{:b}", n))),
            [Value::Int(n), Value::Int(width)] => {
                if *width <= 0 {
                    Err(RuntimeError::ValueError(
                        "binary() width must be positive".to_string(),
                    ))
                } else {
                    // Mask to width, then print padded.
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

        // length(x) for list or string
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

        // freeze(dict) -> FrozenDict (shallow copy); idempotent on FrozenDict
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

        // --- Errors -----------------------------------------------------------

        // panic("message") -> directly raise RuntimeError::Raised
        "panic" => match args {
            [Value::Str(msg)] => Err(RuntimeError::Raised(msg.clone())),
            _ => Err(RuntimeError::TypeError(
                "panic() expects a string (type mismatch)".to_string(),
            )),
        },

        // raise("message") -> synthesize a VM raise of ErrorKind::Generic
        //
        // We reuse the VM’s raise path to ensure handlers (SetupExcept) can catch it.
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

        // --- Filesystem -------------------------------------------------------

        // read_file("path") -> String content; resolves relative to current_dir
        "read_file" => match args {
            [Value::Str(path)] => {
                let path_buf = resolve_path(path, env, globals);
                match fs::read_to_string(&path_buf) {
                    Ok(content) => Ok(Value::Str(content)),
                    // Use ModuleImportError because this is commonly used by importers.
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

        // file_open("path", "r|rb|w|wb|a|ab") -> handle (int)
        "file_open" => match args {
            [Value::Str(path), Value::Str(mode)] => {
                let path_buf = resolve_path(path, env, globals);
                let mut opts = OpenOptions::new();
                let binary = mode.contains('b');
                // Configure options based on mode; we support read/write/append.
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
                        return Err(RuntimeError::ValueError(
                            "invalid file mode".to_string(),
                        ));
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

        // file_read(handle) -> String for text; List[Int bytes] for binary
        "file_read" => match args {
            [Value::Int(handle)] => {
                let mut table = FILE_HANDLES.lock().unwrap();
                if let Some(entry) = table.get_mut(&(*handle as i32)) {
                    if entry.binary {
                        // Binary: read whole file to Vec<u8>, return as list of Ints [0..255]
                        let mut buf = Vec::new();
                        entry
                            .file
                            .read_to_end(&mut buf)
                            .map_err(|e| RuntimeError::ValueError(e.to_string()))?;
                        let list: Vec<Value> =
                            buf.into_iter().map(|b| Value::Int(b as i64)).collect();
                        Ok(Value::List(Rc::new(RefCell::new(list))))
                    } else {
                        // Text: read whole file to String
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

        // file_write(handle, data) -> Int bytes written
        // - Text handle expects String
        // - Binary handle expects List[Int 0..255]
        "file_write" => match args {
            // Text write
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
            // Binary write
            [Value::Int(handle), Value::List(list)] => {
                let mut table = FILE_HANDLES.lock().unwrap();
                if let Some(entry) = table.get_mut(&(*handle as i32)) {
                    if !entry.binary {
                        return Err(RuntimeError::TypeError(
                            "file_write() text handle expects string".to_string(),
                        ));
                    }
                    // Validate and pack list of ints into bytes
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

        // file_close(handle) -> None
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

        // file_exists("path") -> Bool
        "file_exists" => match args {
            [Value::Str(path)] => {
                let path_buf = resolve_path(path, env, globals);
                Ok(Value::Bool(path_buf.exists()))
            }
            _ => Err(RuntimeError::TypeError(
                "file_exists() expects a path".to_string(),
            )),
        },

        // call_builtin("name", [args...]) -> Value (delegates to another builtin)
        "call_builtin" => match args {
            [Value::Str(inner), Value::List(list)] => {
                let inner_args = list.borrow().clone();
                call_builtin(inner, &inner_args, env, globals)
            }
            _ => Err(RuntimeError::TypeError(
                "call_builtin() expects a name and argument list".to_string(),
            )),
        },

        // Unknown builtin
        _ => Err(RuntimeError::TypeError(format!(
            "unknown builtin: {}",
            name
        ))),
    }
}
