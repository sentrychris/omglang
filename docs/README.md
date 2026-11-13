# OMG docs

OMG is a small dynamically-typed language whose whole toolchain is
written in OMG. These pages are the manual: running programs, turning
`.omg` source into standalone executables, and how the compiler, the
VM, the back ends and the runtimes are put together.

## Start here

| If you want to...                                  | Read                          |
| -------------------------------------------------- | ----------------------------- |
| ...write and run your first program in 5 minutes   | [01-quickstart.md](01-quickstart.md) |
| ...know what each part is for, on one page         | [../toolchain.md](toolchain.md) |
| ...understand how the pieces fit together          | [02-architecture.md](02-architecture.md) |
| ...learn the language itself                       | [03-language-tour.md](03-language-tour.md) |
| ...see what happens between `.omg` and `./foo`     | [04-pipeline.md](04-pipeline.md) |
| ...add a new builtin, opcode, or syntax form       | [05-extending.md](05-extending.md) |
| ...understand what a built program carries with it | [06-runtime.md](06-runtime.md) |
| ...debug something that's gone wrong               | [07-debugging.md](07-debugging.md) |
| ...use it on Windows                               | [08-windows.md](08-windows.md) |

## TL;DR

```sh
# Linux and Windows (x86-64): nothing to install, nothing to build
bin/omg foo.omg              # compile and run
bin/omg --build foo.omg foo  # a small native executable (from about 23 KB)
./foo
bin/omg --build foo.omg foo --target windows-x64   # or one for the other system
bin/omg --emit-c  foo.omg foo.c    # stop at generated C
bin/omg --emit-js foo.omg foo.js   # or generate JavaScript

scripts/build.sh             # after changing src/: omg rebuilds itself
```

On macOS and other systems `omg` goes through C; see
[01-quickstart.md](01-quickstart.md#other-systems).

## What's in `bin/`

One program for each system, and the runtime files it reads:

| File        | Role                                                        |
| ----------- | ----------------------------------------------------------- |
| `omg`, `omg.exe` | The whole toolchain, for Linux and for Windows: run / REPL / `--compile` / `--build` / `--emit-c` / `--emit-js` / `--disasm` |
| `x64/*.omg` | The runtime for machine code, as OMG source (compiled into every executable `--build` writes) |
| `omg_rt.h`  | C runtime header (pasted into every `.c` that `omg` generates) |
| `omg_rt.js` | JS runtime (pasted into every `.js` that `omg` generates)   |

`omg` is a native executable built from OMG source by itself. It
imports the compiler, the VM and the three back ends directly, so
compile, run, REPL, `omg foo.omgb` (running precompiled bytecode) and
`--build` all happen in-process. No other program is started, unless
you ask for `--target c`, which runs `cc`.

Earlier versions shipped three more binaries (`omgc`, `omgcc`,
`omgjs`). They are gone; their jobs are `omg --compile`,
`omg --emit-c` and `omg --emit-js`.

## Conventions in these docs

`omg <file>` means `bin/omg <file>` (`bin\omg.exe` on Windows). Put
`bin/` on your `PATH`, or make an alias, if you'd like to type the
bare `omg` form yourself.

The Rust runtime in `rust/` is where OMG started. It is frozen and
these pages do not use it; [toolchain.md](toolchain.md) says what it
is still kept for.
