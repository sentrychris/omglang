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

## TL;DR

```sh
# One-time bootstrap (uses the Rust runtime to build the native toolchain)
cd runtime && cargo build --release && cd ..
bootstrap/build-native-toolchain.sh

# After that: no Rust required
./bootstrap/native/omg foo.omg              # compile and run
./bootstrap/native/omg --build foo.omg foo  # AOT to a small ELF (~30 KB)
./foo
```

## What's in `bootstrap/native/`

| Binary    | Role                                  | Size    |
| --------- | ------------------------------------- | ------- |
| `omg`     | User-facing driver (run / compile / build) | 1.5 KB  |
| `omg-build` | One-shot AOT: `.omg` → ELF          | 0.6 KB  |
| `omgc`    | Compiler: `.omg` → `.omgb` bytecode   | 432 KB  |
| `omgcc`   | Transpiler: `.omgb` → `.c`            | 290 KB  |
| `omgvm`   | Bytecode interpreter                  | 197 KB  |
| `omg_rt.h`| C runtime header (inlined into output) | 63 KB   |

`omg` and `omg-build` are shell scripts; everything else is a native ELF.

## Conventions in these docs

| Shorthand                          | Means                                         |
| ---------------------------------- | --------------------------------------------- |
| `omg <file>`                       | `bootstrap/native/omg <file>` (native driver) |
| `runtime/target/release/omg <…>`   | the Rust runtime, spelled out in full         |

Some commands (notably `--disasm` and `--verify-omg-vm`) only exist on the
Rust runtime, so they're always written out in full. Drop `bootstrap/native/`
on your `$PATH` if you'd like to use the bare `omg` form yourself.
