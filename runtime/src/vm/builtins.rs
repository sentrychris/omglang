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

/// `floor()`/`ceil()`/`round()` return ints. After the float operation the
/// result is still an f64; convert it to i64 with the same finite/range
/// checks `Value::as_int` uses on floats.
fn float_to_int_rounded(f: f64, op: &'static str) -> Result<Value, RuntimeError> {
    if !f.is_finite() {
        return Err(RuntimeError::ValueError(format!(
            "{}() of a non-finite float: {}",
            op, f
        )));
    }
    if f < i64::MIN as f64 || f > i64::MAX as f64 {
        return Err(RuntimeError::ValueError(format!(
            "{}() result {} is outside the i64 range",
            op, f
        )));
    }
    Ok(Value::Int(f as i64))
}

/// Round half to even (banker's rounding). Matches Python 3 `round()`.
fn round_half_even(f: f64) -> f64 {
    let rounded = f.round(); // half away from zero
    let diff = (f - f.trunc()).abs();
    if (diff - 0.5).abs() < 1e-9 {
        // Exactly halfway: pick the even neighbour.
        let down = f.trunc();
        let up = if f >= 0.0 { down + 1.0 } else { down - 1.0 };
        if (down as i64) % 2 == 0 {
            down
        } else {
            up
        }
    } else {
        rounded
    }
}

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

        // string_bytes(s) → list of UTF-8 byte values for `s`. Used by the
        // OMG-in-OMG compiler so it can write source strings into the
        // `.omgb` byte stream the same way the Rust frontend does
        // (length + raw UTF-8 bytes, not codepoint-per-element).
        "string_bytes" => match args {
            [Value::Str(s)] => {
                let list: Vec<Value> = s
                    .as_bytes()
                    .iter()
                    .map(|b| Value::Int(*b as i64))
                    .collect();
                Ok(Value::List(Rc::new(RefCell::new(list))))
            }
            _ => Err(RuntimeError::TypeError(
                "string_bytes() expects a string".to_string(),
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

        // is_dir("path") -> Bool
        "is_dir" => match args {
            [Value::Str(path)] => {
                let path_buf = resolve_path(path, env, globals);
                Ok(Value::Bool(path_buf.is_dir()))
            }
            _ => Err(RuntimeError::TypeError(
                "is_dir() expects a path".to_string(),
            )),
        },

        // read_dir("path") -> [String, ...] of entry names (no `.` or `..`).
        // Sorted lexicographically so output is deterministic across runs.
        "read_dir" => match args {
            [Value::Str(path)] => {
                let path_buf = resolve_path(path, env, globals);
                let entries = fs::read_dir(&path_buf).map_err(|e| {
                    RuntimeError::ValueError(format!(
                        "cannot read directory '{}': {}",
                        path_buf.display(),
                        e
                    ))
                })?;
                let mut names: Vec<String> = Vec::new();
                for entry in entries {
                    let entry = entry.map_err(|e| RuntimeError::ValueError(e.to_string()))?;
                    if let Some(name) = entry.file_name().to_str() {
                        names.push(name.to_string());
                    }
                }
                names.sort();
                Ok(Value::List(Rc::new(RefCell::new(
                    names.into_iter().map(Value::Str).collect(),
                ))))
            }
            _ => Err(RuntimeError::TypeError(
                "read_dir() expects a directory path".to_string(),
            )),
        },

        // make_dir("path") -> Bool. Creates intermediate directories
        // (`mkdir -p` semantics). Returns `true` on success or if the
        // directory already exists; raises ValueError on real failures.
        "make_dir" => match args {
            [Value::Str(path)] => {
                let path_buf = resolve_path(path, env, globals);
                fs::create_dir_all(&path_buf).map_err(|e| {
                    RuntimeError::ValueError(format!(
                        "cannot create directory '{}': {}",
                        path_buf.display(),
                        e
                    ))
                })?;
                Ok(Value::Bool(true))
            }
            _ => Err(RuntimeError::TypeError(
                "make_dir() expects a path".to_string(),
            )),
        },

        // --- Numeric / math --------------------------------------------------

        // int(x) -> i64. Floats truncate toward zero; strings parse as int.
        "int" => match args {
            [v] => v.as_int().map(Value::Int),
            _ => Err(RuntimeError::TypeError(
                "int() expects one argument".to_string(),
            )),
        },

        // float(x) -> f64. Ints widen; strings parse as float.
        "float" => match args {
            [v] => v.as_float().map(Value::Float),
            _ => Err(RuntimeError::TypeError(
                "float() expects one argument".to_string(),
            )),
        },

        // floor(x) -> int. Largest integer <= x.
        "floor" => match args {
            [Value::Int(i)] => Ok(Value::Int(*i)),
            [Value::Float(f)] => float_to_int_rounded(f.floor(), "floor"),
            _ => Err(RuntimeError::TypeError(
                "floor() expects one number".to_string(),
            )),
        },

        // ceil(x) -> int. Smallest integer >= x.
        "ceil" => match args {
            [Value::Int(i)] => Ok(Value::Int(*i)),
            [Value::Float(f)] => float_to_int_rounded(f.ceil(), "ceil"),
            _ => Err(RuntimeError::TypeError(
                "ceil() expects one number".to_string(),
            )),
        },

        // round(x) -> int. Banker's rounding (ties to even) — matches Python 3.
        "round" => match args {
            [Value::Int(i)] => Ok(Value::Int(*i)),
            [Value::Float(f)] => float_to_int_rounded(round_half_even(*f), "round"),
            _ => Err(RuntimeError::TypeError(
                "round() expects one number".to_string(),
            )),
        },

        // abs(x) -> same type as x (int or float).
        "abs" => match args {
            [Value::Int(i)] => i
                .checked_abs()
                .map(Value::Int)
                .ok_or_else(|| RuntimeError::ValueError("integer overflow on abs".to_string())),
            [Value::Float(f)] => Ok(Value::Float(f.abs())),
            _ => Err(RuntimeError::TypeError(
                "abs() expects one number".to_string(),
            )),
        },

        // sqrt(x) -> float
        "sqrt" => match args {
            [v] => {
                let f = v.as_float()?;
                if f < 0.0 {
                    Err(RuntimeError::ValueError(
                        "sqrt() of a negative number".to_string(),
                    ))
                } else {
                    Ok(Value::Float(f.sqrt()))
                }
            }
            _ => Err(RuntimeError::TypeError(
                "sqrt() expects one number".to_string(),
            )),
        },

        // pow(a, b). int**non_negative_int returns int (overflow-checked);
        // anything else widens to float.
        "pow" => match args {
            [a, b] => {
                if let (Value::Int(ai), Value::Int(bi)) = (a, b) {
                    if *bi >= 0 && *bi <= u32::MAX as i64 {
                        return ai
                            .checked_pow(*bi as u32)
                            .map(Value::Int)
                            .ok_or_else(|| {
                                RuntimeError::ValueError("integer overflow on pow".to_string())
                            });
                    }
                }
                Ok(Value::Float(a.as_float()?.powf(b.as_float()?)))
            }
            _ => Err(RuntimeError::TypeError(
                "pow() expects two numbers".to_string(),
            )),
        },

        // log(x) -> natural log. Errors on x <= 0.
        "log" => match args {
            [v] => {
                let f = v.as_float()?;
                if f <= 0.0 {
                    Err(RuntimeError::ValueError(
                        "log() requires a positive number".to_string(),
                    ))
                } else {
                    Ok(Value::Float(f.ln()))
                }
            }
            _ => Err(RuntimeError::TypeError(
                "log() expects one number".to_string(),
            )),
        },

        "sin" => match args {
            [v] => Ok(Value::Float(v.as_float()?.sin())),
            _ => Err(RuntimeError::TypeError(
                "sin() expects one number".to_string(),
            )),
        },
        "cos" => match args {
            [v] => Ok(Value::Float(v.as_float()?.cos())),
            _ => Err(RuntimeError::TypeError(
                "cos() expects one number".to_string(),
            )),
        },
        "tan" => match args {
            [v] => Ok(Value::Float(v.as_float()?.tan())),
            _ => Err(RuntimeError::TypeError(
                "tan() expects one number".to_string(),
            )),
        },

        // dict_keys(d) -> [String]. Returns the keys of a dict (or frozen
        // dict) as a list. Order is *unspecified* (HashMap iteration);
        // callers that need determinism should sort. OMG previously had
        // no way to enumerate a dict's keys from inside the language.
        "dict_keys" => match args {
            [Value::Dict(map)] => {
                let keys: Vec<Value> = map
                    .borrow()
                    .keys()
                    .map(|k| Value::Str(k.clone()))
                    .collect();
                Ok(Value::List(Rc::new(RefCell::new(keys))))
            }
            [Value::FrozenDict(map)] => {
                let keys: Vec<Value> = map
                    .keys()
                    .map(|k| Value::Str(k.clone()))
                    .collect();
                Ok(Value::List(Rc::new(RefCell::new(keys))))
            }
            _ => Err(RuntimeError::TypeError(
                "dict_keys() expects a dict".to_string(),
            )),
        },

        // bytes_to_string([byte, ...]) -> String. Inverse of `string_bytes`.
        // The input list must be UTF-8 byte values (0-255). Used by the
        // OMG-in-OMG VM (`bootstrap/vm.omg`) to read length-
        // prefixed strings out of a `.omgb` byte stream.
        "bytes_to_string" => match args {
            [Value::List(list)] => {
                let borrowed = list.borrow();
                let mut bytes: Vec<u8> = Vec::with_capacity(borrowed.len());
                for v in borrowed.iter() {
                    match v {
                        Value::Int(b) if *b >= 0 && *b <= 255 => bytes.push(*b as u8),
                        _ => {
                            return Err(RuntimeError::TypeError(
                                "bytes_to_string() expects a list of bytes (0-255)".to_string(),
                            ))
                        }
                    }
                }
                String::from_utf8(bytes)
                    .map(Value::Str)
                    .map_err(|e| {
                        RuntimeError::ValueError(format!(
                            "bytes_to_string(): invalid UTF-8: {}",
                            e
                        ))
                    })
            }
            _ => Err(RuntimeError::TypeError(
                "bytes_to_string() expects a list of bytes".to_string(),
            )),
        },

        // bits_to_float(i64) -> f64 reinterpretation of an IEEE-754 bit
        // pattern. Inverse of `float_bits`. Used by the OMG-in-OMG VM
        // to reconstruct float literals from the 8 raw bytes they were
        // written as in the bytecode stream.
        "bits_to_float" => match args {
            [Value::Int(bits)] => Ok(Value::Float(f64::from_bits(*bits as u64))),
            _ => Err(RuntimeError::TypeError(
                "bits_to_float() expects an integer".to_string(),
            )),
        },

        // float_bits("3.14") -> i64 reinterpretation of the IEEE-754 bits.
        // Used by the OMG-in-OMG compiler so it can embed float literals
        // in the bytecode without doing float math itself: it parses the
        // literal text to its 64-bit pattern and writes the i64 the same
        // way it writes any other 8-byte payload.
        "float_bits" => match args {
            [Value::Str(s)] => {
                let f: f64 = s.trim().parse().map_err(|_| {
                    RuntimeError::ValueError(format!("float_bits(): invalid literal '{}'", s))
                })?;
                Ok(Value::Int(f.to_bits() as i64))
            }
            _ => Err(RuntimeError::TypeError(
                "float_bits() expects a numeric string".to_string(),
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
