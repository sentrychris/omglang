# OMG Native: docs

OMG is a small dynamically-typed language with a self-hosted compiler. This
section documents the **native compilation path** — turning `.omg` source
into standalone executables with no Rust runtime needed, on Linux, macOS
and Windows.

## Start here

| If you want to...                                  | Read                          |
| -------------------------------------------------- | ----------------------------- |
| ...write and run your first program in 5 minutes   | [01-quickstart.md](01-quickstart.md) |
| ...know what each part is for, on one page         | [../toolchain.md](../toolchain.md) |
| ...understand how Rust, OMG, and C fit together    | [02-architecture.md](02-architecture.md) |
| ...learn the language itself                       | [03-language-tour.md](03-language-tour.md) |
| ...see what happens between `.omg` and `./foo`     | [04-pipeline.md](04-pipeline.md) |
| ...add a new builtin, opcode, or syntax form       | [05-extending.md](05-extending.md) |
| ...understand how `omg_rt.h` works                 | [06-runtime.md](06-runtime.md) |
| ...debug something that's gone wrong               | [07-debugging.md](07-debugging.md) |
| ...use it on Windows                               | [08-windows.md](08-windows.md) |

## TL;DR

```sh
# One-time bootstrap (uses the Rust runtime to build the native toolchain)
cd runtime && cargo build --release && cd ..
bootstrap/build.sh

# After that: no Rust required
./bootstrap/bin/omg foo.omg              # compile and run
./bootstrap/bin/omg --build foo.omg foo  # AOT to a small executable (~30 KB)
./foo
./bootstrap/bin/omg --emit-c  foo.omg foo.c    # stop at the generated C
./bootstrap/bin/omg --emit-js foo.omg foo.js   # or generate JavaScript
```

## What's in `bootstrap/bin/`

One program and the two runtime files it needs:

| File        | Role                                                        |
| ----------- | ----------------------------------------------------------- |
| `omg`       | The whole toolchain: run / REPL / `--compile` / `--build` / `--emit-c` / `--emit-js` / `--disasm` |
| `omg_rt.h`  | C runtime header (pasted into every `.c` that `omg` generates) |
| `omg_rt.js` | JS runtime (pasted into every `.js` that `omg` generates)   |

`omg` is a native ELF compiled from OMG source. It imports
`compiler.omg`, `vm.omg`, `native-c.omg` and `native-js.omg` directly,
so compile, run, REPL, `omg foo.omgb` (running precompiled bytecode)
and code generation all happen in-process — the only external command
invoked is `cc` for the final ELF link in `--build`.

Earlier versions shipped three more binaries (`omgc`, `omgcc`,
`omgjs`). They are gone; their jobs are `omg --compile`,
`omg --emit-c` and `omg --emit-js`.

## Conventions in these docs

| Shorthand                          | Means                                         |
| ---------------------------------- | --------------------------------------------- |
| `omg <file>`                       | `bootstrap/bin/omg <file>` (native driver) |
| `runtime/target/release/omg <…>`   | the Rust runtime, spelled out in full         |

Some commands (notably `--verify-omg-vm`, `--verify-self-hosted`, and
`--self-hosted-compile`) only exist on the Rust runtime, so they're always
written out in full. Drop `bootstrap/bin/` on your `$PATH` if you'd like to
use the bare `omg` form yourself.
