# OMG Native: docs

OMG is a small dynamically-typed language with a self-hosted compiler. This
section documents the **native compilation path** — turning `.omg` source
into standalone ELF binaries with no Rust runtime needed.

## Start here

| If you want to...                                  | Read                          |
| -------------------------------------------------- | ----------------------------- |
| ...write and run your first program in 5 minutes   | [01-quickstart.md](01-quickstart.md) |
| ...understand how Rust, OMG, and C fit together    | [02-architecture.md](02-architecture.md) |
| ...learn the language itself                       | [03-language-tour.md](03-language-tour.md) |
| ...see what happens between `.omg` and `./foo`     | [04-pipeline.md](04-pipeline.md) |
| ...add a new builtin, opcode, or syntax form       | [05-extending.md](05-extending.md) |
| ...understand how `omg_rt.h` works                 | [06-runtime.md](06-runtime.md) |
| ...debug something that's gone wrong               | [07-debugging.md](07-debugging.md) |
| ...ship a Rust-free dist of just the native bits   | [packaging.md](packaging.md) |

## TL;DR

```sh
# One-time bootstrap (uses the Rust runtime to build the native toolchain)
cd runtime && cargo build --release && cd ..
bootstrap/build.sh

# After that: no Rust required
./bootstrap/bin/omg foo.omg              # compile and run
./bootstrap/bin/omg --build foo.omg foo  # AOT to a small ELF (~30 KB)
./foo
```

## What's in `bootstrap/bin/`

| Binary     | Role                                              |
| ---------- | ------------------------------------------------- |
| `omg`      | Unified driver: run / compile / build / REPL      |
| `omgc`     | Compiler: `.omg` → `.omgb` bytecode               |
| `omgcc`    | C transpiler: `.omgb` → `.c`                      |
| `omgjs`    | JS transpiler: `.omgb` → `.js`                    |
| `omgvm`    | Bytecode interpreter (executes `.omgb`)           |
| `omg_rt.h` | C runtime header (inlined into every `.c` omgcc emits)   |
| `omg_rt.js`| JS runtime (inlined into every `.js` omgjs emits) |

All five binaries are native ELFs compiled from OMG source. `omg`
imports `compiler.omg`, `vm.omg`, and `native-c.omg` directly so
compile, run, and REPL happen in-process — the only external command
invoked is `cc` for the final ELF link in `--build`.

## Conventions in these docs

| Shorthand                          | Means                                         |
| ---------------------------------- | --------------------------------------------- |
| `omg <file>`                       | `bootstrap/bin/omg <file>` (native driver) |
| `runtime/target/release/omg <…>`   | the Rust runtime, spelled out in full         |

Some commands (notably `--disasm` and `--verify-omg-vm`) only exist on the
Rust runtime, so they're always written out in full. Drop `bootstrap/bin/`
on your `$PATH` if you'd like to use the bare `omg` form yourself.
