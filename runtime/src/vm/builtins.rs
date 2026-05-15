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
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{atomic::{AtomicI32, Ordering}, Mutex};

use once_cell::sync::Lazy;

use super::ops_control;
use crate::error::{ErrorKind, RuntimeError};
use crate::value::Env;
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

/// Entry in the in-process TCP handle table. `Listener` is a passive
/// socket awaiting `tcp_accept`; `Stream` is a connected peer (either
/// the result of `accept` or `tcp_connect`).
enum TcpEntry {
    Listener(TcpListener),
    Stream(TcpStream),
}

/// Global TCP handle table. Kept separate from `FILE_HANDLES` so the
/// type-check at dispatch is cheap (no enum unwrap on every file op).
static TCP_HANDLES: Lazy<Mutex<HashMap<i32, TcpEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Monotonic counter for TCP handles. Starts at 1 so a 0 handle is
/// always invalid (mirrors the C runtime's convention).
static NEXT_TCP_FD: AtomicI32 = AtomicI32::new(1);

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
fn resolve_path(path: &str, env: &Env, globals: &HashMap<String, Value>) -> PathBuf {
    let mut path_buf = PathBuf::from(path.replace("\\", "/"));
    if path_buf.is_relative() {
        // env's slots are EnvCells; borrow the cell to read its current
        // value. Fall back to globals (which stay plain Values).
        let env_val: Option<Value> = env.get("current_dir").map(|c| c.borrow().clone());
        let candidate = env_val.as_ref().or_else(|| globals.get("current_dir"));
        if let Some(Value::Str(cur)) = candidate {
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
    env: &Env,
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

        // exit_with_error("message") -> print msg to stderr verbatim
        // (no kind prefix), then std::process::exit(1). Bypasses the
        // try/except machinery deliberately — used by `vm.omg` to
        // surface an already-formatted error message at top level
        // without wrapping it in another `RuntimeError:` prefix.
        "exit_with_error" => match args {
            [Value::Str(msg)] => {
                eprintln!("{}", msg);
                std::process::exit(1);
            }
            _ => Err(RuntimeError::TypeError(
                "exit_with_error() expects a string".to_string(),
            )),
        },

        // exit(code) -> std::process::exit(code). Used by the OMG-native
        // `omg` driver to propagate child-process exit codes.
        "exit" => match args {
            [Value::Int(code)] => {
                std::process::exit(*code as i32);
            }
            _ => Err(RuntimeError::TypeError(
                "exit() expects an integer".to_string(),
            )),
        },

        // getpid() -> int. Used by the OMG-native drivers to make
        // unique tempfile paths without needing a mktemp builtin.
        "getpid" => {
            if !args.is_empty() {
                return Err(RuntimeError::TypeError(
                    "getpid() takes no arguments".to_string(),
                ));
            }
            Ok(Value::Int(std::process::id() as i64))
        }

        // stdin_readline() -> str | bool. Reads one line from stdin
        // (without the trailing newline). Returns `false` on EOF — the
        // same convention `read_file` uses, so callers can `if line ==
        // false`. Used by the OMG-native REPL.
        "stdin_readline" => {
            if !args.is_empty() {
                return Err(RuntimeError::TypeError(
                    "stdin_readline() takes no arguments".to_string(),
                ));
            }
            use std::io::BufRead;
            let stdin = std::io::stdin();
            let mut line = String::new();
            match stdin.lock().read_line(&mut line) {
                Ok(0) => Ok(Value::Bool(false)),
                Ok(_) => {
                    // Strip trailing \n (and \r if present, for Windows
                    // line endings). Keep everything else verbatim.
                    if line.ends_with('\n') {
                        line.pop();
                        if line.ends_with('\r') {
                            line.pop();
                        }
                    }
                    Ok(Value::Str(line))
                }
                Err(e) => Err(RuntimeError::ValueError(format!(
                    "stdin_readline: {}", e
                ))),
            }
        }

        // stdin_read() -> str. Slurps all of stdin to EOF and returns
        // it as a UTF-8 string. The pipe-friendly counterpart to
        // read_file(): `cat input | omg tool.omg` works once the tool
        // calls stdin_read(). Returns the empty string if stdin is
        // already at EOF (e.g. no input piped in).
        "stdin_read" => {
            if !args.is_empty() {
                return Err(RuntimeError::TypeError(
                    "stdin_read() takes no arguments".to_string(),
                ));
            }
            use std::io::Read;
            let mut buf = String::new();
            match std::io::stdin().read_to_string(&mut buf) {
                Ok(_) => Ok(Value::Str(buf)),
                Err(e) => Err(RuntimeError::ValueError(format!(
                    "stdin_read: {}", e
                ))),
            }
        }

        // === Real-time terminal I/O ==================================
        //
        // Pairs of (time_ms, sleep_ms) for frame pacing and
        // (stdin_set_raw, stdin_read_key) for non-blocking keyboard
        // input. Together they unlock interactive terminal programs —
        // see games/snake.omg.

        // time_ms() -> int. Monotonic-ish millisecond clock based on
        // SystemTime. Suitable for elapsed-time measurements; not for
        // wall-clock display (system time can be adjusted backwards).
        "time_ms" => {
            if !args.is_empty() {
                return Err(RuntimeError::TypeError(
                    "time_ms() takes no arguments".to_string(),
                ));
            }
            use std::time::{SystemTime, UNIX_EPOCH};
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            Ok(Value::Int(ms))
        }

        // sleep_ms(n) -> none. Pauses the current process for n
        // milliseconds. Negative or zero n is a no-op.
        "sleep_ms" => match args {
            [Value::Int(n)] => {
                if *n > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(*n as u64));
                }
                Ok(Value::None)
            }
            _ => Err(RuntimeError::TypeError(
                "sleep_ms() expects an int".to_string(),
            )),
        },

        // stdin_set_raw(on) -> none. Toggles cbreak / no-echo mode on
        // the controlling terminal so reads return immediately and the
        // tty doesn't line-buffer or echo input. Must be called twice
        // around an interactive section (`true` to enter raw mode,
        // `false` to restore). Linux-only — Windows would need
        // SetConsoleMode; not implemented.
        "stdin_set_raw" => match args {
            [Value::Bool(on)] => {
                unsafe {
                    use std::os::fd::AsRawFd;
                    let fd = std::io::stdin().as_raw_fd();
                    let mut t: libc::termios = std::mem::zeroed();
                    if libc::tcgetattr(fd, &mut t) != 0 {
                        return Err(RuntimeError::ValueError(
                            "stdin_set_raw: tcgetattr failed".to_string(),
                        ));
                    }
                    if *on {
                        // Disable canonical mode + echo. VMIN/VTIME = 0
                        // makes read() return 0 with no input rather
                        // than blocking — pairs with stdin_read_key().
                        // ICANON / ECHO: line buffering and char echo off
                        // (the obvious raw-mode bits).
                        // ISIG: don't let the kernel intercept Ctrl-C,
                        // Ctrl-Z, Ctrl-\\ as signals — TUI apps want
                        // those as plain bytes (e.g. an editor binding
                        // Ctrl-Z to undo).
                        // IXON: don't let Ctrl-S/Ctrl-Q pause output
                        // via XON/XOFF — same reasoning, plus it stops
                        // Ctrl-S from freezing the terminal.
                        t.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
                        t.c_iflag &= !libc::IXON;
                        t.c_cc[libc::VMIN] = 0;
                        t.c_cc[libc::VTIME] = 0;
                    } else {
                        t.c_lflag |= libc::ICANON | libc::ECHO | libc::ISIG;
                        t.c_iflag |= libc::IXON;
                    }
                    if libc::tcsetattr(fd, libc::TCSANOW, &t) != 0 {
                        return Err(RuntimeError::ValueError(
                            "stdin_set_raw: tcsetattr failed".to_string(),
                        ));
                    }
                }
                Ok(Value::None)
            }
            _ => Err(RuntimeError::TypeError(
                "stdin_set_raw() expects a bool".to_string(),
            )),
        },

        // stdin_read_key() -> str | bool. Non-blocking single-byte
        // read. Returns a 1-character string if a key is available, or
        // false otherwise. Requires stdin_set_raw(true) first; in
        // cooked mode the kernel buffers input until newline so reads
        // would still block at the read syscall.
        "stdin_read_key" => {
            if !args.is_empty() {
                return Err(RuntimeError::TypeError(
                    "stdin_read_key() takes no arguments".to_string(),
                ));
            }
            unsafe {
                use std::os::fd::AsRawFd;
                let fd = std::io::stdin().as_raw_fd();
                let mut buf: [u8; 1] = [0];
                let n = libc::read(fd, buf.as_mut_ptr() as *mut _, 1);
                if n == 1 {
                    Ok(Value::Str((buf[0] as char).to_string()))
                } else {
                    // n == 0 with VMIN=VTIME=0 means "no input ready"
                    // when raw mode is on; n < 0 is an error (EAGAIN
                    // when nonblocking — we treat the same).
                    Ok(Value::Bool(false))
                }
            }
        }

        // stdin_read_bytes() -> [int, ...]. Like stdin_read but for
        // binary pipes — returns a list of byte values (0-255). The
        // pipe-friendly counterpart to file_open(path, "rb") +
        // file_read().
        "stdin_read_bytes" => {
            if !args.is_empty() {
                return Err(RuntimeError::TypeError(
                    "stdin_read_bytes() takes no arguments".to_string(),
                ));
            }
            use std::io::Read;
            let mut buf: Vec<u8> = Vec::new();
            match std::io::stdin().read_to_end(&mut buf) {
                Ok(_) => {
                    let list: Vec<Value> =
                        buf.into_iter().map(|b| Value::Int(b as i64)).collect();
                    Ok(Value::List(Rc::new(RefCell::new(list))))
                }
                Err(e) => Err(RuntimeError::ValueError(format!(
                    "stdin_read_bytes: {}", e
                ))),
            }
        }

        // print(s) -> None. Like emit, but no trailing newline. Used by
        // the REPL so the prompt sits on the same line as user input.
        "print" => match args {
            [Value::Str(s)] => {
                use std::io::Write;
                print!("{}", s);
                let _ = std::io::stdout().flush();
                Ok(Value::None)
            }
            _ => Err(RuntimeError::TypeError(
                "print() expects a string".to_string(),
            )),
        },

        // subprocess(["cmd", "arg1", ...]) -> int (exit code).
        // Forks, execs the command (PATH-resolved), waits for it.
        // stdin/stdout/stderr are inherited. Used by the OMG-native
        // `omg` driver to shell out to `cc` during `--build`; the
        // rest of the toolchain (compiler, transpiler, VM) all run
        // in-process via imports, no subprocess hop needed.
        //
        // Returns the child's exit code (or 128+signal if killed).
        // Raises ValueError if exec itself fails (e.g. binary not
        // found) — lets the caller decide whether to swallow it.
        "subprocess" => match args {
            [Value::List(list)] => {
                let borrowed = list.borrow();
                let argv: Vec<String> = borrowed
                    .iter()
                    .map(|v| match v {
                        Value::Str(s) => Ok(s.clone()),
                        _ => Err(RuntimeError::TypeError(
                            "subprocess() expects a list of strings".to_string(),
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if argv.is_empty() {
                    return Err(RuntimeError::ValueError(
                        "subprocess() needs at least the command".to_string(),
                    ));
                }
                let mut cmd = std::process::Command::new(&argv[0]);
                cmd.args(&argv[1..]);
                match cmd.status() {
                    Ok(status) => {
                        let code = status.code().unwrap_or_else(|| {
                            #[cfg(unix)]
                            {
                                use std::os::unix::process::ExitStatusExt;
                                128 + status.signal().unwrap_or(0)
                            }
                            #[cfg(not(unix))]
                            {
                                -1
                            }
                        });
                        Ok(Value::Int(code as i64))
                    }
                    Err(e) => Err(RuntimeError::ValueError(format!(
                        "subprocess: cannot exec '{}': {}",
                        argv[0], e
                    ))),
                }
            }
            _ => Err(RuntimeError::TypeError(
                "subprocess() expects a list of strings".to_string(),
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

        // file_open("path", "r|rb|w|wb|a|ab|rb+|wb+") -> handle (int)
        // The `+` modes are binary-only random-access:
        //   rb+ — open existing for read+write; preserves contents.
        //   wb+ — create/truncate for read+write.
        // Pair with file_seek + file_tell for page-level I/O.
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
                    "rb+" => {
                        opts.read(true).write(true);
                    }
                    "wb+" => {
                        opts.read(true).write(true).create(true).truncate(true);
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

        // file_seek(handle, offset) -> Int (new absolute position)
        // Seeks the handle to `offset` bytes from the start of the file.
        // Negative offsets are rejected. Pair with file_tell.
        "file_seek" => match args {
            [Value::Int(handle), Value::Int(offset)] => {
                if *offset < 0 {
                    return Err(RuntimeError::ValueError(
                        "file_seek() expects a non-negative offset".to_string(),
                    ));
                }
                use std::io::Seek;
                let mut table = FILE_HANDLES.lock().unwrap();
                if let Some(entry) = table.get_mut(&(*handle as i32)) {
                    match entry.file.seek(std::io::SeekFrom::Start(*offset as u64)) {
                        Ok(pos) => Ok(Value::Int(pos as i64)),
                        Err(e) => Err(RuntimeError::ValueError(format!(
                            "file_seek: {}", e
                        ))),
                    }
                } else {
                    Err(RuntimeError::ValueError("invalid file handle".to_string()))
                }
            }
            _ => Err(RuntimeError::TypeError(
                "file_seek() expects handle and offset".to_string(),
            )),
        },

        // file_tell(handle) -> Int (current absolute position)
        "file_tell" => match args {
            [Value::Int(handle)] => {
                use std::io::Seek;
                let mut table = FILE_HANDLES.lock().unwrap();
                if let Some(entry) = table.get_mut(&(*handle as i32)) {
                    match entry.file.stream_position() {
                        Ok(pos) => Ok(Value::Int(pos as i64)),
                        Err(e) => Err(RuntimeError::ValueError(format!(
                            "file_tell: {}", e
                        ))),
                    }
                } else {
                    Err(RuntimeError::ValueError("invalid file handle".to_string()))
                }
            }
            _ => Err(RuntimeError::TypeError(
                "file_tell() expects a handle".to_string(),
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

        // --- TCP networking --------------------------------------------------
        //
        // Six builtins; handle ints come from `NEXT_TCP_FD` and live in
        // `TCP_HANDLES`. Listener handles only accept `tcp_accept`/`tcp_close`;
        // stream handles only accept `tcp_read`/`tcp_write`/`tcp_close`.
        // Cross-use is a ValueError rather than a panic so OMG `try` can
        // recover.

        // tcp_listen(host, port) -> handle. Binds + listens on host:port.
        // Use "0.0.0.0" to listen on all interfaces, "127.0.0.1" for
        // loopback only. Port 0 picks an ephemeral port (call tcp_addr
        // afterwards if you need to know which — not yet exposed).
        "tcp_listen" => match args {
            [Value::Str(host), Value::Int(port)] => {
                if *port < 0 || *port > 65535 {
                    return Err(RuntimeError::ValueError(format!(
                        "tcp_listen: port {} out of range 0..65535",
                        port
                    )));
                }
                let addr = format!("{}:{}", host, port);
                match TcpListener::bind(&addr) {
                    Ok(listener) => {
                        let handle = NEXT_TCP_FD.fetch_add(1, Ordering::SeqCst);
                        TCP_HANDLES
                            .lock()
                            .unwrap()
                            .insert(handle, TcpEntry::Listener(listener));
                        Ok(Value::Int(handle as i64))
                    }
                    Err(e) => Err(RuntimeError::ValueError(format!(
                        "tcp_listen: cannot bind '{}': {}",
                        addr, e
                    ))),
                }
            }
            _ => Err(RuntimeError::TypeError(
                "tcp_listen() expects (host: str, port: int)".to_string(),
            )),
        },

        // tcp_accept(handle) -> client_handle. Blocks until a peer
        // connects. The returned handle is a *stream*; close it with
        // tcp_close once you're done with that request.
        "tcp_accept" => match args {
            [Value::Int(handle)] => {
                // Hold the lock only long enough to pull the listener
                // out by reference; .accept() blocks and we don't want
                // to keep the global table locked while it does.
                let listener_clone = {
                    let table = TCP_HANDLES.lock().unwrap();
                    match table.get(&(*handle as i32)) {
                        Some(TcpEntry::Listener(l)) => {
                            l.try_clone().map_err(|e| {
                                RuntimeError::ValueError(format!(
                                    "tcp_accept: cannot clone listener: {}", e
                                ))
                            })?
                        }
                        Some(TcpEntry::Stream(_)) => {
                            return Err(RuntimeError::ValueError(
                                "tcp_accept: handle is a stream, not a listener".to_string(),
                            ));
                        }
                        None => {
                            return Err(RuntimeError::ValueError(
                                "tcp_accept: invalid tcp handle".to_string(),
                            ));
                        }
                    }
                };
                match listener_clone.accept() {
                    Ok((stream, _peer)) => {
                        let new_handle = NEXT_TCP_FD.fetch_add(1, Ordering::SeqCst);
                        TCP_HANDLES
                            .lock()
                            .unwrap()
                            .insert(new_handle, TcpEntry::Stream(stream));
                        Ok(Value::Int(new_handle as i64))
                    }
                    Err(e) => Err(RuntimeError::ValueError(format!(
                        "tcp_accept: {}", e
                    ))),
                }
            }
            _ => Err(RuntimeError::TypeError(
                "tcp_accept() expects a listener handle".to_string(),
            )),
        },

        // tcp_connect(host, port) -> handle. Outbound client connection.
        "tcp_connect" => match args {
            [Value::Str(host), Value::Int(port)] => {
                if *port < 0 || *port > 65535 {
                    return Err(RuntimeError::ValueError(format!(
                        "tcp_connect: port {} out of range 0..65535",
                        port
                    )));
                }
                let addr = format!("{}:{}", host, port);
                match TcpStream::connect(&addr) {
                    Ok(stream) => {
                        let handle = NEXT_TCP_FD.fetch_add(1, Ordering::SeqCst);
                        TCP_HANDLES
                            .lock()
                            .unwrap()
                            .insert(handle, TcpEntry::Stream(stream));
                        Ok(Value::Int(handle as i64))
                    }
                    Err(e) => Err(RuntimeError::ValueError(format!(
                        "tcp_connect: cannot connect to '{}': {}",
                        addr, e
                    ))),
                }
            }
            _ => Err(RuntimeError::TypeError(
                "tcp_connect() expects (host: str, port: int)".to_string(),
            )),
        },

        // tcp_read(handle, max_bytes) -> [int, ...]. Reads up to
        // `max_bytes` bytes from the stream and returns them as a list
        // of byte values (0-255). An empty list means EOF (peer closed
        // the write end). A short read does NOT mean EOF — the peer
        // may simply not have sent all `max_bytes` yet. Callers that
        // need a delimiter-framed message (like an HTTP request) must
        // loop until they see the delimiter.
        "tcp_read" => match args {
            [Value::Int(handle), Value::Int(max_bytes)] => {
                if *max_bytes < 0 {
                    return Err(RuntimeError::ValueError(
                        "tcp_read: max_bytes must be non-negative".to_string(),
                    ));
                }
                let mut stream_clone = {
                    let table = TCP_HANDLES.lock().unwrap();
                    match table.get(&(*handle as i32)) {
                        Some(TcpEntry::Stream(s)) => {
                            s.try_clone().map_err(|e| {
                                RuntimeError::ValueError(format!(
                                    "tcp_read: cannot clone stream: {}", e
                                ))
                            })?
                        }
                        Some(TcpEntry::Listener(_)) => {
                            return Err(RuntimeError::ValueError(
                                "tcp_read: handle is a listener, not a stream".to_string(),
                            ));
                        }
                        None => {
                            return Err(RuntimeError::ValueError(
                                "tcp_read: invalid tcp handle".to_string(),
                            ));
                        }
                    }
                };
                let mut buf = vec![0u8; *max_bytes as usize];
                match stream_clone.read(&mut buf) {
                    Ok(n) => {
                        buf.truncate(n);
                        let list: Vec<Value> =
                            buf.into_iter().map(|b| Value::Int(b as i64)).collect();
                        Ok(Value::List(Rc::new(RefCell::new(list))))
                    }
                    Err(e) => Err(RuntimeError::ValueError(format!(
                        "tcp_read: {}", e
                    ))),
                }
            }
            _ => Err(RuntimeError::TypeError(
                "tcp_read() expects (handle: int, max_bytes: int)".to_string(),
            )),
        },

        // tcp_write(handle, data) -> int bytes written. Accepts either
        // a string (UTF-8 encoded) or a list of byte values 0-255.
        // Writes the whole buffer (loops internally on partial writes).
        "tcp_write" => match args {
            [Value::Int(handle), Value::Str(data)] => {
                let mut stream_clone = {
                    let table = TCP_HANDLES.lock().unwrap();
                    match table.get(&(*handle as i32)) {
                        Some(TcpEntry::Stream(s)) => {
                            s.try_clone().map_err(|e| {
                                RuntimeError::ValueError(format!(
                                    "tcp_write: cannot clone stream: {}", e
                                ))
                            })?
                        }
                        Some(TcpEntry::Listener(_)) => {
                            return Err(RuntimeError::ValueError(
                                "tcp_write: handle is a listener, not a stream".to_string(),
                            ));
                        }
                        None => {
                            return Err(RuntimeError::ValueError(
                                "tcp_write: invalid tcp handle".to_string(),
                            ));
                        }
                    }
                };
                let bytes = data.as_bytes();
                stream_clone.write_all(bytes).map_err(|e| {
                    RuntimeError::ValueError(format!("tcp_write: {}", e))
                })?;
                Ok(Value::Int(bytes.len() as i64))
            }
            [Value::Int(handle), Value::List(list)] => {
                let mut stream_clone = {
                    let table = TCP_HANDLES.lock().unwrap();
                    match table.get(&(*handle as i32)) {
                        Some(TcpEntry::Stream(s)) => {
                            s.try_clone().map_err(|e| {
                                RuntimeError::ValueError(format!(
                                    "tcp_write: cannot clone stream: {}", e
                                ))
                            })?
                        }
                        Some(TcpEntry::Listener(_)) => {
                            return Err(RuntimeError::ValueError(
                                "tcp_write: handle is a listener, not a stream".to_string(),
                            ));
                        }
                        None => {
                            return Err(RuntimeError::ValueError(
                                "tcp_write: invalid tcp handle".to_string(),
                            ));
                        }
                    }
                };
                let bytes = list
                    .borrow()
                    .iter()
                    .map(|v| match v {
                        Value::Int(i) if *i >= 0 && *i <= 255 => Ok(*i as u8),
                        _ => Err(RuntimeError::TypeError(
                            "tcp_write() expects bytes 0-255".to_string(),
                        )),
                    })
                    .collect::<Result<Vec<u8>, RuntimeError>>()?;
                stream_clone.write_all(&bytes).map_err(|e| {
                    RuntimeError::ValueError(format!("tcp_write: {}", e))
                })?;
                Ok(Value::Int(bytes.len() as i64))
            }
            _ => Err(RuntimeError::TypeError(
                "tcp_write() expects (handle: int, data: str | [int])".to_string(),
            )),
        },

        // fork() -> int. POSIX fork: returns 0 in the child and the
        // child's PID in the parent. Used for per-request concurrency
        // in the HTTP server example — each accept forks a worker so
        // the parent loop can immediately accept the next connection.
        //
        // We set SIGCHLD to SIG_IGN on the first call so the kernel
        // auto-reaps exiting children; without it, naive servers
        // accumulate zombies indefinitely. Setting it idempotently
        // means there's no separate `wait()` builtin to remember.
        //
        // Refcounted heap state isn't thread-safe in either runtime,
        // so we deliberately chose processes over threads — each
        // child gets its own copy-on-write address space, no shared-
        // mutability hazards. Unix-only.
        "fork" => {
            if !args.is_empty() {
                return Err(RuntimeError::TypeError(
                    "fork() takes no arguments".to_string(),
                ));
            }
            // SAFETY: signal/fork are standard libc functions; calling
            // them is fine from a single-threaded VM. Setting SIGCHLD
            // to SIG_IGN is the POSIX-blessed way to disable zombies
            // without a SIGCHLD handler.
            unsafe {
                libc::signal(libc::SIGCHLD, libc::SIG_IGN);
                let pid = libc::fork();
                if pid < 0 {
                    let err = std::io::Error::last_os_error();
                    Err(RuntimeError::ValueError(format!("fork: {}", err)))
                } else {
                    Ok(Value::Int(pid as i64))
                }
            }
        }

        // tcp_close(handle) -> None. Drops the listener or stream,
        // releasing the OS resource. Closing an already-closed handle
        // raises ValueError (mirrors file_close).
        "tcp_close" => match args {
            [Value::Int(handle)] => {
                let mut table = TCP_HANDLES.lock().unwrap();
                if table.remove(&(*handle as i32)).is_some() {
                    Ok(Value::None)
                } else {
                    Err(RuntimeError::ValueError("invalid tcp handle".to_string()))
                }
            }
            _ => Err(RuntimeError::TypeError(
                "tcp_close() expects a handle".to_string(),
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

        // list_repeat(item, count) -> [item, item, ..., item]. Allocates
        // a fresh list of length `count` with every slot holding the
        // same value. Bridges the gap to amortised-doubling buffer
        // growth in pure OMG — repeated `xs + [v]` is O(n²), but
        // `list_repeat(0, new_cap)` plus per-slot writes is O(n).
        // Used by `bootstrap/src/compiler.omg`'s bytecode writer to
        // build the .omgb byte vector at amortised O(1) per byte
        // instead of O(n). Negative counts raise ValueError.
        "list_repeat" => match args {
            [item, Value::Int(count)] => {
                if *count < 0 {
                    return Err(RuntimeError::ValueError(format!(
                        "list_repeat() count must be non-negative, got {}",
                        count
                    )));
                }
                let n = *count as usize;
                let mut items: Vec<Value> = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(item.clone());
                }
                Ok(Value::List(Rc::new(RefCell::new(items))))
            }
            _ => Err(RuntimeError::TypeError(
                "list_repeat() expects (any, int)".to_string(),
            )),
        },

        // has_key(d, k) -> bool. True iff the dict (or frozen dict) `d`
        // contains the string key `k`. Non-throwing alternative to the
        // `try { d[k] }` probe pattern; the OMG-on-OMG VM's hot path
        // (vm_lookup / vm_store) does multiple such probes per
        // bytecode op, so the try/except overhead matters.
        "has_key" => match args {
            // Key is coerced via Value::to_string() to match how OMG
            // already stringifies dict keys on write/read (see
            // ops_struct::handle_index). Non-dict containers cleanly
            // return false — the OMG-on-OMG VM's `is_vm_none` /
            // `is_vm_closure` predicates rely on probing arbitrary
            // values without a try/except wrapper.
            [Value::Dict(map), key] => {
                let key_str = key.to_string();
                Ok(Value::Bool(map.borrow().contains_key(&key_str)))
            }
            [Value::FrozenDict(map), key] => {
                let key_str = key.to_string();
                Ok(Value::Bool(map.contains_key(&key_str)))
            }
            [_, _] => Ok(Value::Bool(false)),
            _ => Err(RuntimeError::TypeError(
                "has_key() expects (any, key)".to_string(),
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
        // OMG-in-OMG VM (`bootstrap/src/vm.omg`) to read length-
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
