# 08 · Windows

The native `omg` runs on Windows as an ordinary program, `omg.exe`. It
is the same toolchain as on Linux (the same OMG source, built by its
own x86-64 back end), with every command: run, REPL, `--compile`,
`--build`, `--emit-c`, `--emit-js`, `--disasm`. No WSL, no Cygwin, no
C compiler, no Rust.

## What you need

| To do this                                  | You need |
| ------------------------------------------- | -------- |
| Run programs, use the REPL, `--compile`, `--disasm`, `--emit-c`, `--emit-js` | nothing: `bin\omg.exe` is checked in |
| `--build` (make an `.exe` out of a program) | nothing: `omg.exe` writes the executable itself |
| Rebuild `omg.exe` itself                    | nothing |
| `--build --target c` (go through C instead) | a C compiler: GCC or Clang from MinGW-w64, on `PATH` |

The last row is optional. `--target c` is the route OMG takes on
systems its machine-code back end does not cover, and it is there on
Windows as a second opinion. If you want it, any MinGW-w64
distribution works:

```powershell
# WinLibs (a standalone GCC)
winget install BrechtSanders.WinLibs.POSIX.UCRT

# or MSYS2, then add C:\msys64\ucrt64\bin to PATH
pacman -S mingw-w64-ucrt-x86_64-gcc
```

`omg` looks for `gcc`, then `clang`, then `cc`. Microsoft's `cl.exe`
is not supported: the C runtime uses GNU C (`__builtin_frame_address`,
`__attribute__`), which GCC and Clang share and MSVC does not have.

## Using it

```powershell
git clone https://github.com/sentrychris/omglang
cd omglang

bin\omg.exe examples\prime_sieve.omg      # run
bin\omg.exe                               # REPL
bin\omg.exe --build hello.omg             # writes hello.exe
.\hello.exe
bin\omg.exe --build hello.omg hello --target linux-x64   # a Linux program, from Windows
bin\omg.exe --emit-js hello.omg hello.js  # then: node hello.js
```

Say `omg.exe`, not `omg`, when you run it from the repository:
`bin\omg` with no extension is the Linux binary, which sits
in the same folder.

To have `omg` on your `PATH`, copy `omg.exe` and the `x64` folder
beside it into one folder that is on it (and `omg_rt.h` and
`omg_rt.js` too, if you use `--target c`, `--emit-c` or `--emit-js`).
`omg` reads these runtime files from the folder it lives in and builds
them into what it generates, so they travel together.

```powershell
Set-Alias omg "$PWD\bin\omg.exe"   # or just an alias, for this session
```

An executable made by `--build` is one file of a few tens of
kilobytes. It imports from `kernel32.dll` (and `ws2_32.dll` if it uses
TCP, `user32.dll` and `gdi32.dll` if it opens a window), which are part
of Windows, and from nothing else: no C runtime DLL, no OMG. You can copy it to another machine as it is.

## Rebuilding the toolchain

After changing anything in `src\`:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build.ps1
```

or, from Git Bash, `scripts/build.sh`, which is the same build. Both
are one command underneath, run twice so that the `omg.exe` that ends
up in `bin\` is the new source built by itself:

```powershell
bin\omg.exe --build src\omg.omg bin\omg.exe
```

It takes a few seconds. A third run would write the same bytes.

The `omg.exe` that is checked in is not built on Windows. `build.sh`
on Linux writes it next to the Linux binary with
`--target windows-x64`, because the back end does not depend on the
system it runs on. CI checks on a real Windows machine that the
checked-in `omg.exe` runs and builds programs, and that it rebuilds
itself there, twice, to the same bytes.

## What is different on Windows

OMG programs are meant to behave the same everywhere. These are the
places where Windows shows through.

| Area | On Windows |
| ---- | ---------- |
| `fork()` | Raises a catchable `ValueError`; Windows has no `fork`. Use `subprocess()`. |
| `interrupted()` | Counts Ctrl-C and Ctrl-Break in the console, where elsewhere it counts SIGINT and SIGTERM. A `tcp_poll` in progress notices within a tenth of a second. |
| Sockets | The `tcp_*` builtins, `tcp_poll` included, behave as elsewhere, so the [HTTP server](09-http-server.md) serves many connections at once here too. |
| A window | The `window_` builtins work as on Linux. An executable that uses them imports `user32.dll` and `gdi32.dll` beside `kernel32.dll`; others do not. While the user drags or resizes the window the program waits inside `window_poll`. See [10-video.md](10-video.md). |
| `pty_spawn`, `pty_write` | Raise a catchable `ValueError`. `pty_read` returns `false`; `pty_close` and `pty_resize` do nothing. |
| Paths | `/` and `\` both work as separators, and `C:\dir` and `\\server\share` are absolute. `current_dir` and `executable_path()` are spelled the way Windows spells them, with backslashes. |
| Files | Always bytes. Nothing turns `\n` into `\r\n` on the way out or back on the way in, for files or for standard input and output. |
| Text | Strings are UTF-8. Paths, command lines and arguments are converted to and from UTF-16 where they meet Windows, so non-ASCII file names and arguments work. |
| Console | Output is UTF-8 and ANSI escape sequences are switched on, so colours and cursor movement work in the classic console as well as in Windows Terminal. The console's previous code page is put back when the program ends. |
| `subprocess(argv)` | The program is looked up on `PATH`, with `.exe` added if needed. Arguments are quoted the way Windows programs expect to parse them. A batch file needs `["cmd", "/c", "script.bat"]`. |
| `--build` | Adds `.exe` to the output name if it is missing. |
| Recursion in a built executable | The executable asks Windows for a 64 MiB stack, which is good for several hundred thousand nested calls, as on Linux. Past that it raises `RuntimeError: maximum recursion depth exceeded`. |

A program that cannot be started by `subprocess()` raises a catchable
`ValueError` on every system.

## How the port is put together

Two pieces, both OMG:

- [`src/backends/native-x64.omg`](../src/backends/native-x64.omg)
  writes the file. The machine code for your program is the same on
  both systems; for Windows it is wrapped as a PE image with one code
  section, one data section and an import table listing exactly the
  system functions the program reaches.
- [`src/runtime/x64/windows.omg`](../src/runtime/x64/windows.omg) is
  the system layer of the runtime: about sixty `os_*` procedures
  (open, read, write, directory listing, clock, sleep, subprocess,
  sockets, console, a window) written against `kernel32.dll` and, for
  TCP, `ws2_32.dll`, and for a window `user32.dll` and `gdi32.dll`. [`linux.omg`](../src/runtime/x64/linux.omg) next to it
  is the same interface on Linux system calls. The rest of the runtime
  (values, memory, strings, builtins, errors) is shared and knows
  nothing about either system.

What the Windows layer does that Linux has no need of: converts
between UTF-8 and UTF-16 at every call that takes a name, switches the
console to UTF-8 and ANSI at start-up and puts it back at exit, splits
the command line into arguments itself, and translates console key
events into the escape sequences a Unix terminal sends, so that
raw-mode programs (the games) see the same bytes on both.

The driver ([`omg.omg`](../src/omg.omg)) asks the runtime it was built
with which system it is on (`linux-x64` or `windows-x64`) and builds
for that unless `--target` says otherwise.

`--target c` still has its own Windows port, inside
[`src/runtime/omg_rt.h`](../src/runtime/omg_rt.h): a platform layer of
`omg_os_*` functions on the wide-character APIs, `setjmp`/`longjmp`
of the plain register-restoring kind (`__USE_MINGW_SETJMP_NON_SEH`),
and the program run on a 64 MiB thread.

## Testing

CI runs two jobs on `windows-latest` on every push. One runs the whole
of `tests/run.sh` against `omg.exe` under Git Bash: it first checks
that the checked-in `omg.exe` runs and builds a program, then rebuilds
it from its own source with `build.sh` and again with `build.ps1`. The
other takes every compiler off `PATH` and runs `tests/standalone.sh`:
the self-rebuild, byte for byte, and the conformance corpus,
interpreted and built.

To run the suite yourself, use Git Bash:

```sh
scripts/build.sh
tests/run.sh
```

`node` on `PATH` adds the JavaScript checks, and `gcc` adds the ones
for `--target c`; both are skipped without them.

One thing the automated tests cannot reach is the raw-mode keyboard
(`stdin_set_raw`, `stdin_read_key`), which needs a real console and a
person pressing keys. The games in `examples/games/` are the test.
