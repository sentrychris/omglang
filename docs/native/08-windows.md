# 08 · Windows

The native `omg` runs on Windows as an ordinary program, `omg.exe`. It
is the same toolchain as on Linux and macOS (the same OMG source, put
through the same C back end), with every command: run, REPL,
`--compile`, `--build`, `--emit-c`, `--emit-js`, `--disasm`. No WSL, no
Cygwin, no Rust.

## What you need

| To do this                                  | You need |
| ------------------------------------------- | -------- |
| Run programs, use the REPL, `--compile`, `--disasm`, `--emit-c`, `--emit-js` | nothing: `bootstrap\bin\omg.exe` is checked in |
| `--build` (make an `.exe` out of a program) | a C compiler: GCC or Clang from MinGW-w64, on `PATH` |
| Rebuild `omg.exe` itself                    | the same C compiler |

Any MinGW-w64 distribution works. Two easy ones:

```powershell
# WinLibs (a standalone GCC)
winget install BrechtSanders.WinLibs.POSIX.UCRT

# or MSYS2, then add C:\msys64\ucrt64\bin to PATH
pacman -S mingw-w64-ucrt-x86_64-gcc
```

Open a new terminal afterwards and check with `gcc --version`. `omg`
looks for `gcc`, then `clang`, then `cc`.

Microsoft's `cl.exe` is not supported: the C runtime uses GNU C
(`__builtin_frame_address`, `__attribute__`), which GCC and Clang share
and MSVC does not have.

## Using it

```powershell
git clone https://github.com/sentrychris/omglang
cd omglang

bootstrap\bin\omg.exe examples\prime_sieve.omg      # run
bootstrap\bin\omg.exe                               # REPL
bootstrap\bin\omg.exe --build hello.omg             # writes hello.exe
.\hello.exe
bootstrap\bin\omg.exe --emit-js hello.omg hello.js  # then: node hello.js
```

Say `omg.exe`, not `omg`, when you run it from the repository:
`bootstrap\bin\omg` with no extension is the Linux binary, which sits
in the same folder.

To have `omg` on your `PATH`, copy three files into one folder that is
on it: `omg.exe`, `omg_rt.h` and `omg_rt.js`. `omg` reads the two
runtime files from the folder it lives in and pastes them into the C
and JavaScript it generates, so they travel together.

```powershell
Set-Alias omg "$PWD\bootstrap\bin\omg.exe"   # or just an alias, for this session
```

An executable made by `--build` is statically linked. It needs no DLLs
beyond the ones that are part of Windows, and no OMG, so you can copy it
to another machine as it is.

## Rebuilding the toolchain

After changing anything in `bootstrap\src\`:

```powershell
powershell -ExecutionPolicy Bypass -File bootstrap\build.ps1
```

or, from Git Bash, `bootstrap/build.sh`, which is the same build. Both
are two commands underneath:

```powershell
bootstrap\bin\omg.exe --emit-c bootstrap\src\omg.omg omg.c
gcc -O3 -w omg.c -o bootstrap\bin\omg.exe -static -lm -lws2_32
```

The `omg.exe` that is checked in is not built on Windows. `build.sh` on
Linux produces it next to the Linux binary by putting the same `omg.c`
through the MinGW-w64 cross compiler, because the C that `omg` writes
does not depend on the system it was written on: every difference
between systems is an `#ifdef _WIN32` inside `omg_rt.h`.

## What is different on Windows

OMG programs are meant to behave the same everywhere. These are the
places where Windows shows through. The builtins that cannot work there
(`fork`, `pty_*`) fail the same way, with the same message, as they do
in the Rust runtime on Windows.

| Area | On Windows |
| ---- | ---------- |
| `fork()` | Raises a catchable `ValueError`; Windows has no `fork`. Use `subprocess()`. (`tools/lib/http.omg` catches this and serves one request at a time.) |
| `pty_spawn`, `pty_write` | Raise a catchable `ValueError`. `pty_read` returns `false`; `pty_close` and `pty_resize` do nothing. |
| Paths | `/` and `\` both work as separators, and `C:\dir` and `\\server\share` are absolute. `current_dir` and `executable_path()` are spelled the way Windows spells them, with backslashes. |
| Files | Always bytes. Nothing turns `\n` into `\r\n` on the way out or back on the way in, for files or for standard input and output. |
| Text | Strings are UTF-8. Paths, command lines and arguments are converted to and from UTF-16 where they meet Windows, so non-ASCII file names and arguments work. |
| Console | Output is UTF-8 and ANSI escape sequences are switched on, so colours and cursor movement work in the classic console as well as in Windows Terminal. The console's previous code page is put back when the program ends. |
| `emit` | Flushed after every call. (Elsewhere standard output is line-buffered, which the Windows C library does not offer.) |
| `subprocess(argv)` | The program is looked up on `PATH`, with `.exe` added if needed. Arguments are quoted the way Windows programs expect to parse them. A batch file needs `["cmd", "/c", "script.bat"]`. |
| `--build` | Adds `.exe` to the output name if it is missing. |
| Recursion in a built executable | The program runs on its own thread with a 64 MiB stack, which is good for about 15,000 nested calls, as on Linux. Past that it raises `RuntimeError: maximum recursion depth exceeded`. |

A program that cannot be started by `subprocess()` raises a catchable
`ValueError` on every system.

## How the port is put together

Everything is in [`bootstrap/src/omg_rt.h`](../../bootstrap/src/omg_rt.h),
the C runtime that is pasted into every generated program:

- A small **platform layer** (`omg_os_*`: open, exists, mkdir, seek,
  current directory, clock, sleep) that the builtins call instead of
  the C library directly. On Windows these go through the wide-character
  APIs.
- Windows versions of the builtins that are system calls at heart:
  `subprocess` (`CreateProcessW`), `read_dir` (`FindFirstFileW`), the
  raw-mode keyboard (`ReadConsoleInputW`, translated to the same escape
  sequences a Unix terminal sends), `executable_path`, `getpid`, and
  TCP on Winsock.
- `omg_start()`, the per-system start-up that the generated `main`
  hands the program to. On Windows it sets binary standard streams and
  the console, converts the command line to UTF-8, and runs the program
  on the 64 MiB thread.
- `try` / `except` is `setjmp` / `longjmp`. On 64-bit Windows the
  default `longjmp` walks the stack with the system unwinder, which is
  not reliable through generated code, so the runtime asks MinGW for
  the plain register-restoring kind (`__USE_MINGW_SETJMP_NON_SEH`).

The driver ([`omg.omg`](../../bootstrap/src/omg.omg)) has one Windows
branch, in `--build`: which compiler names to try, the `.exe` suffix,
the static link, and where the temporary C file goes. It tells Windows
from the others by looking at `current_dir`.

## Testing

CI builds the toolchain on `windows-latest` on every push and runs the
whole of `tests/run.sh` against `omg.exe` under Git Bash: it first
checks that the checked-in `omg.exe` runs and builds a program, then
rebuilds it from its own source with `build.sh` and again with
`build.ps1`.

To run the suite yourself, use Git Bash with `gcc` and `node` on `PATH`:

```sh
cargo build --release --manifest-path runtime/Cargo.toml
bootstrap/build.sh
tests/run.sh
```

One thing the automated tests cannot reach is the raw-mode keyboard
(`stdin_set_raw`, `stdin_read_key`), which needs a real console and a
person pressing keys. It is the same logic as the Rust runtime's
Windows implementation; the games in `examples/` are the test.
