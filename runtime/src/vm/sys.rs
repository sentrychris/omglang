//! # Platform layer for the terminal / process builtins
//!
//! Everything in the runtime that has to talk to the operating system
//! through something other than `std` lives here, behind one small API
//! with a backend per platform:
//!
//! | Builtin            | Unix (Linux, macOS, BSD)     | Windows                        |
//! | ------------------ | ---------------------------- | ------------------------------ |
//! | `stdin_set_raw`    | termios                      | console mode                   |
//! | `stdin_read_key`   | non-blocking `read(0)`       | console input records          |
//! | `pty_*`            | `forkpty` + non-blocking fd  | not supported (clean error)    |
//! | `fork`             | `fork` + `SIGCHLD = SIG_IGN` | not supported (clean error)    |
//!
//! `builtins.rs` only ever sees this API, so it contains no `cfg` and no
//! `unsafe`. Unsupported operations return an `Err(String)` that the
//! caller turns into a catchable `ValueError` — an OMG program can probe
//! for a capability with `try { ... } except { ... }` instead of failing
//! to build on that platform.
//!
//! The backend also remembers the terminal state from before the first
//! `stdin_set_raw(true)`, so [`restore_terminal`] can put the user's
//! shell back the way it was when a program exits (normally, through
//! `exit()`, or with an uncaught error) while still in raw mode.

/// Result of a non-blocking read from a pty master.
// Platforms without pty support only ever construct `Eof`.
#[cfg_attr(not(unix), allow(dead_code))]
pub enum PtyRead {
    /// Bytes were available.
    Data(Vec<u8>),
    /// The child is alive but has nothing to say right now.
    Empty,
    /// The child closed its end (or the fd is invalid).
    Eof,
}

pub use imp::*;

// ===========================================================================
// Unix
// ===========================================================================

#[cfg(unix)]
mod imp {
    use super::PtyRead;
    use std::ffi::CString;
    use std::sync::Mutex;

    /// Terminal attributes captured before the first switch to raw mode.
    /// `None` while the terminal is in whatever state we found it in.
    static SAVED_TERMIOS: Mutex<Option<libc::termios>> = Mutex::new(None);

    /// One-time process setup. Rust ignores `SIGPIPE` by default, which
    /// turns `omg script.omg | head -1` into a panic ("failed printing to
    /// stdout") once `head` exits. Restoring the default disposition makes
    /// the runtime behave like every other Unix filter — and like the
    /// AOT-compiled native binaries — and die quietly instead.
    pub fn init() {
        // SAFETY: called once at startup, before any other thread exists.
        unsafe {
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        }
    }

    pub fn stdin_set_raw(on: bool) -> Result<(), String> {
        let fd = libc::STDIN_FILENO;
        let mut saved = SAVED_TERMIOS.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: termios is plain-old-data; tcgetattr fully initialises
        // it on success and we bail out on failure.
        unsafe {
            let mut t: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut t) != 0 {
                return Err("stdin_set_raw: tcgetattr failed".to_string());
            }
            if on {
                if saved.is_none() {
                    *saved = Some(t);
                }
                // ICANON / ECHO: line buffering and character echo off.
                // ISIG: deliver Ctrl-C / Ctrl-Z / Ctrl-\ as plain bytes
                // rather than signals (an editor binds Ctrl-Z to undo).
                // IXON: stop Ctrl-S / Ctrl-Q from pausing output.
                // VMIN = VTIME = 0 makes read() return immediately when
                // no key is waiting, which is what stdin_read_key needs.
                t.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG);
                t.c_iflag &= !libc::IXON;
                t.c_cc[libc::VMIN] = 0;
                t.c_cc[libc::VTIME] = 0;
            } else if let Some(orig) = saved.take() {
                // Put back exactly what we found, including VMIN/VTIME
                // and any flags the user's shell had customised.
                t = orig;
            } else {
                // Raw mode was never entered through us; fall back to
                // forcing the cooked-mode flags on.
                t.c_lflag |= libc::ICANON | libc::ECHO | libc::ISIG;
                t.c_iflag |= libc::IXON;
            }
            if libc::tcsetattr(fd, libc::TCSANOW, &t) != 0 {
                return Err("stdin_set_raw: tcsetattr failed".to_string());
            }
        }
        Ok(())
    }

    /// Undo a pending `stdin_set_raw(true)`. Safe to call at any time and
    /// any number of times; does nothing unless raw mode is active.
    pub fn restore_terminal() {
        let mut saved = SAVED_TERMIOS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(orig) = saved.take() {
            // SAFETY: `orig` came from a successful tcgetattr.
            unsafe {
                libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig);
            }
        }
    }

    pub fn stdin_read_key() -> Option<u8> {
        let mut buf = [0u8; 1];
        // SAFETY: reading one byte into a one-byte buffer.
        let n = unsafe { libc::read(libc::STDIN_FILENO, buf.as_mut_ptr().cast(), 1) };
        // 0 means "no key waiting" in raw mode (or EOF); negative is an
        // error such as EAGAIN. Either way there is no key to report.
        if n == 1 {
            Some(buf[0])
        } else {
            None
        }
    }

    pub fn pty_spawn(argv: &[String]) -> Result<i64, String> {
        // Everything the child needs is allocated *before* forking: after
        // fork only async-signal-safe calls are allowed in the child.
        let mut cstrs: Vec<CString> = Vec::with_capacity(argv.len());
        for a in argv {
            cstrs.push(
                CString::new(a.as_str())
                    .map_err(|_| "pty_spawn(): nul byte in argv element".to_string())?,
            );
        }
        let mut argv_p: Vec<*const libc::c_char> = cstrs.iter().map(|s| s.as_ptr()).collect();
        argv_p.push(std::ptr::null());
        let term_name = CString::new("TERM").expect("static string");
        let term_value = CString::new("xterm-256color").expect("static string");
        // SAFETY: forkpty/execvp are used as documented; the child only
        // calls setenv/execvp/_exit with memory prepared above.
        unsafe {
            let mut master_fd: libc::c_int = 0;
            let pid = libc::forkpty(
                &mut master_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            if pid < 0 {
                return Err("pty_spawn(): forkpty failed".to_string());
            }
            if pid == 0 {
                // Child: give the hosted program a sensible TERM, exec,
                // and _exit if that fails so we never unwind back into
                // the VM inside the child.
                libc::setenv(term_name.as_ptr(), term_value.as_ptr(), 1);
                libc::execvp(argv_p[0], argv_p.as_ptr());
                libc::_exit(127);
            }
            // Parent: non-blocking master so pty_read can poll cheaply.
            let flags = libc::fcntl(master_fd, libc::F_GETFL, 0);
            libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
            Ok(master_fd as i64)
        }
    }

    pub fn pty_read(fd: i64) -> PtyRead {
        let mut buf = [0u8; 4096];
        // SAFETY: reading at most buf.len() bytes into buf.
        let n = unsafe { libc::read(fd as libc::c_int, buf.as_mut_ptr().cast(), buf.len()) };
        if n > 0 {
            PtyRead::Data(buf[..n as usize].to_vec())
        } else if n == 0 {
            PtyRead::Eof
        } else {
            // `last_os_error` reads errno portably — `__errno_location`
            // only exists on Linux (macOS spells it `__error`).
            match std::io::Error::last_os_error().kind() {
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted => PtyRead::Empty,
                _ => PtyRead::Eof,
            }
        }
    }

    pub fn pty_write(fd: i64, bytes: &[u8]) -> Result<i64, String> {
        // SAFETY: writing bytes.len() bytes from a live slice.
        let n = unsafe { libc::write(fd as libc::c_int, bytes.as_ptr().cast(), bytes.len()) };
        Ok(n as i64)
    }

    pub fn pty_close(fd: i64) -> Result<(), String> {
        // SAFETY: close on an arbitrary int is harmless (EBADF at worst).
        unsafe {
            libc::close(fd as libc::c_int);
        }
        Ok(())
    }

    pub fn pty_resize(fd: i64, rows: i64, cols: i64) -> Result<(), String> {
        let ws = libc::winsize {
            ws_row: rows.clamp(0, u16::MAX as i64) as u16,
            ws_col: cols.clamp(0, u16::MAX as i64) as u16,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: TIOCSWINSZ takes a pointer to a winsize. The request
        // constant's integer type differs between libcs, hence the cast.
        unsafe {
            libc::ioctl(
                fd as libc::c_int,
                libc::TIOCSWINSZ as _,
                &ws as *const libc::winsize,
            );
        }
        Ok(())
    }

    pub fn fork() -> Result<i64, String> {
        // SAFETY: the VM is single-threaded, so fork is safe to call.
        // SIGCHLD = SIG_IGN is the POSIX way to auto-reap children so a
        // fork-per-request server doesn't accumulate zombies.
        unsafe {
            libc::signal(libc::SIGCHLD, libc::SIG_IGN);
            let pid = libc::fork();
            if pid < 0 {
                Err(format!("fork: {}", std::io::Error::last_os_error()))
            } else {
                Ok(pid as i64)
            }
        }
    }
}

// ===========================================================================
// Windows
// ===========================================================================

#[cfg(windows)]
mod imp {
    use super::PtyRead;
    use std::collections::VecDeque;
    use std::io::Read;
    use std::sync::Mutex;

    use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetNumberOfConsoleInputEvents, GetStdHandle, ReadConsoleInputW,
        SetConsoleMode, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT,
        ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING, INPUT_RECORD,
        KEY_EVENT, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };

    struct ConsoleState {
        /// Console input mode from before the first `stdin_set_raw(true)`.
        saved_mode: Option<u32>,
        /// Whether the console accepted `ENABLE_VIRTUAL_TERMINAL_INPUT`.
        /// When it did, the console itself turns arrow keys and friends
        /// into the same escape sequences a Unix terminal sends; when it
        /// didn't (legacy conhost) we synthesise them from key codes.
        vt_input: bool,
        /// UTF-8 bytes decoded from key events but not yet handed out —
        /// `stdin_read_key` returns one byte per call, like `read(0, 1)`.
        pending: VecDeque<u8>,
        /// First half of a UTF-16 surrogate pair awaiting its partner.
        high_surrogate: Option<u16>,
    }

    static CONSOLE: Mutex<ConsoleState> = Mutex::new(ConsoleState {
        saved_mode: None,
        vt_input: false,
        pending: VecDeque::new(),
        high_surrogate: None,
    });

    fn std_handle(which: u32) -> Option<HANDLE> {
        // SAFETY: GetStdHandle has no preconditions.
        let h = unsafe { GetStdHandle(which) };
        if h.is_null() || h == INVALID_HANDLE_VALUE {
            None
        } else {
            Some(h)
        }
    }

    fn console_mode(h: HANDLE) -> Option<u32> {
        let mut mode: u32 = 0;
        // SAFETY: `mode` is a valid out-pointer. Fails (returns 0) when
        // the handle is a pipe or file rather than a console.
        if unsafe { GetConsoleMode(h, &mut mode) } != 0 {
            Some(mode)
        } else {
            None
        }
    }

    /// One-time process setup: ask the console to interpret ANSI escape
    /// sequences on output. Windows Terminal does this already; the
    /// classic console host needs to be told. Without it every colour
    /// code and cursor move the games emit shows up as literal garbage.
    pub fn init() {
        for which in [STD_OUTPUT_HANDLE, windows_sys::Win32::System::Console::STD_ERROR_HANDLE] {
            if let Some(h) = std_handle(which) {
                if let Some(mode) = console_mode(h) {
                    // SAFETY: valid console handle; failure is harmless.
                    unsafe {
                        SetConsoleMode(h, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
                    }
                }
            }
        }
    }

    pub fn stdin_set_raw(on: bool) -> Result<(), String> {
        let h = std_handle(STD_INPUT_HANDLE)
            .ok_or_else(|| "stdin_set_raw: stdin is not a console".to_string())?;
        let mode =
            console_mode(h).ok_or_else(|| "stdin_set_raw: stdin is not a console".to_string())?;
        let mut st = CONSOLE.lock().unwrap_or_else(|e| e.into_inner());
        if on {
            if st.saved_mode.is_none() {
                st.saved_mode = Some(mode);
            }
            // The three flags mirror the termios bits cleared on Unix:
            // LINE_INPUT ~ ICANON, ECHO_INPUT ~ ECHO, PROCESSED_INPUT ~
            // ISIG (Ctrl-C arrives as byte 0x03 instead of a signal).
            let raw = mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT);
            // SAFETY: valid console handle.
            unsafe {
                if SetConsoleMode(h, raw | ENABLE_VIRTUAL_TERMINAL_INPUT) != 0 {
                    st.vt_input = true;
                } else if SetConsoleMode(h, raw) != 0 {
                    st.vt_input = false;
                } else {
                    return Err("stdin_set_raw: SetConsoleMode failed".to_string());
                }
            }
        } else {
            let restore = st.saved_mode.take().unwrap_or(
                (mode | ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT)
                    & !ENABLE_VIRTUAL_TERMINAL_INPUT,
            );
            st.vt_input = false;
            st.pending.clear();
            st.high_surrogate = None;
            // SAFETY: valid console handle.
            if unsafe { SetConsoleMode(h, restore) } == 0 {
                return Err("stdin_set_raw: SetConsoleMode failed".to_string());
            }
        }
        Ok(())
    }

    /// Undo a pending `stdin_set_raw(true)`. Safe to call at any time and
    /// any number of times; does nothing unless raw mode is active.
    pub fn restore_terminal() {
        let mut st = CONSOLE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mode) = st.saved_mode.take() {
            if let Some(h) = std_handle(STD_INPUT_HANDLE) {
                // SAFETY: valid handle; failure is harmless.
                unsafe {
                    SetConsoleMode(h, mode);
                }
            }
        }
    }

    /// Escape sequence a VT terminal would send for a navigation key.
    /// Only used when the console refused `ENABLE_VIRTUAL_TERMINAL_INPUT`.
    fn vk_to_vt(vk: u16) -> Option<&'static [u8]> {
        Some(match vk {
            0x26 => b"\x1b[A",  // VK_UP
            0x28 => b"\x1b[B",  // VK_DOWN
            0x27 => b"\x1b[C",  // VK_RIGHT
            0x25 => b"\x1b[D",  // VK_LEFT
            0x24 => b"\x1b[H",  // VK_HOME
            0x23 => b"\x1b[F",  // VK_END
            0x2D => b"\x1b[2~", // VK_INSERT
            0x2E => b"\x1b[3~", // VK_DELETE
            0x21 => b"\x1b[5~", // VK_PRIOR (Page Up)
            0x22 => b"\x1b[6~", // VK_NEXT (Page Down)
            _ => return None,
        })
    }

    fn push_utf16(st: &mut ConsoleState, unit: u16) {
        let ch = match (st.high_surrogate.take(), unit) {
            (Some(hi), 0xDC00..=0xDFFF) => {
                let c = 0x10000 + (((hi as u32) - 0xD800) << 10) + ((unit as u32) - 0xDC00);
                char::from_u32(c)
            }
            (_, 0xD800..=0xDBFF) => {
                st.high_surrogate = Some(unit);
                None
            }
            (_, u) => char::from_u32(u as u32),
        };
        if let Some(c) = ch {
            let mut buf = [0u8; 4];
            st.pending.extend(c.encode_utf8(&mut buf).as_bytes());
        }
    }

    pub fn stdin_read_key() -> Option<u8> {
        let mut st = CONSOLE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(b) = st.pending.pop_front() {
            return Some(b);
        }
        let h = std_handle(STD_INPUT_HANDLE)?;
        if console_mode(h).is_none() {
            // Redirected stdin (pipe or file): there is no "no key yet"
            // state to report, so behave like a blocking one-byte read —
            // the same thing read(0) does for a pipe on Unix.
            drop(st);
            let mut buf = [0u8; 1];
            return match std::io::stdin().lock().read(&mut buf) {
                Ok(1) => Some(buf[0]),
                _ => None,
            };
        }
        // Drain queued console events until one yields a byte. Mouse,
        // focus and resize events, key releases, and bare modifier
        // presses are consumed and skipped; if we left them queued the
        // event count would never drop to zero.
        loop {
            let mut count: u32 = 0;
            // SAFETY: valid console handle and out-pointer.
            if unsafe { GetNumberOfConsoleInputEvents(h, &mut count) } == 0 || count == 0 {
                return None;
            }
            let mut rec = INPUT_RECORD::default();
            let mut read: u32 = 0;
            // SAFETY: reading exactly one record into `rec`.
            if unsafe { ReadConsoleInputW(h, &mut rec, 1, &mut read) } == 0 || read == 0 {
                return None;
            }
            if rec.EventType as u32 != KEY_EVENT {
                continue;
            }
            // SAFETY: EventType says the union holds a KEY_EVENT_RECORD.
            let key = unsafe { rec.Event.KeyEvent };
            if key.bKeyDown == 0 {
                continue;
            }
            // SAFETY: both union arms are plain integers.
            let unit = unsafe { key.uChar.UnicodeChar };
            let repeat = key.wRepeatCount.max(1);
            if unit != 0 {
                for _ in 0..repeat {
                    push_utf16(&mut st, unit);
                }
            } else if !st.vt_input {
                if let Some(seq) = vk_to_vt(key.wVirtualKeyCode) {
                    for _ in 0..repeat {
                        st.pending.extend(seq);
                    }
                }
            }
            if let Some(b) = st.pending.pop_front() {
                return Some(b);
            }
        }
    }

    const NO_PTY: &str = "pseudo-terminals are not supported on Windows";

    pub fn pty_spawn(_argv: &[String]) -> Result<i64, String> {
        Err(format!("pty_spawn(): {}", NO_PTY))
    }

    pub fn pty_read(_fd: i64) -> PtyRead {
        PtyRead::Eof
    }

    pub fn pty_write(_fd: i64, _bytes: &[u8]) -> Result<i64, String> {
        Err(format!("pty_write(): {}", NO_PTY))
    }

    pub fn pty_close(_fd: i64) -> Result<(), String> {
        Ok(())
    }

    pub fn pty_resize(_fd: i64, _rows: i64, _cols: i64) -> Result<(), String> {
        Ok(())
    }

    pub fn fork() -> Result<i64, String> {
        Err("fork(): not supported on Windows (there is no fork; use subprocess())".to_string())
    }
}

// ===========================================================================
// Anything else (wasm, embedded, ...): compiles, reports "unsupported".
// ===========================================================================

#[cfg(not(any(unix, windows)))]
mod imp {
    use super::PtyRead;

    const UNSUPPORTED: &str = "not supported on this platform";

    pub fn init() {}

    pub fn stdin_set_raw(_on: bool) -> Result<(), String> {
        Err(format!("stdin_set_raw: {}", UNSUPPORTED))
    }

    pub fn restore_terminal() {}

    pub fn stdin_read_key() -> Option<u8> {
        None
    }

    pub fn pty_spawn(_argv: &[String]) -> Result<i64, String> {
        Err(format!("pty_spawn(): {}", UNSUPPORTED))
    }

    pub fn pty_read(_fd: i64) -> PtyRead {
        PtyRead::Eof
    }

    pub fn pty_write(_fd: i64, _bytes: &[u8]) -> Result<i64, String> {
        Err(format!("pty_write(): {}", UNSUPPORTED))
    }

    pub fn pty_close(_fd: i64) -> Result<(), String> {
        Ok(())
    }

    pub fn pty_resize(_fd: i64, _rows: i64, _cols: i64) -> Result<(), String> {
        Ok(())
    }

    pub fn fork() -> Result<i64, String> {
        Err(format!("fork(): {}", UNSUPPORTED))
    }
}
