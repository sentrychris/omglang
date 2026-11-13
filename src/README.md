# `src/` — the OMG toolchain in OMG

Everything here is OMG source, plus the C and JavaScript runtime files
that two of the back ends paste into what they generate. One file,
[`omg.omg`](omg.omg), is the program; the rest are modules it imports.
[`scripts/build.sh`](../scripts/build.sh) has it build itself into
[`bin/omg`](../bin/).

For what each part is for, see
[`docs/toolchain.md`](../docs/toolchain.md).

```
src/
├── omg.omg        the omg command
├── compiler.omg   source to bytecode
├── bytecode.omg   reads a .omgb file back in
├── vm.omg         the bytecode interpreter
├── backends/      bytecode to something that runs without the VM
├── runtime/       what a built program carries with it
└── browser/       entry points of the two web bundles
```

## The program

| Source               | Becomes             | Role |
| -------------------- | ------------------- | ---- |
| [`omg.omg`](omg.omg) | `bin/omg`, `bin/omg.exe` | The `omg` command: run, REPL, `--compile`, `--build`, `--emit-c`, `--emit-js`, `--disasm` |

## The modules it imports

| Source                           | Role |
| -------------------------------- | ---- |
| [`compiler.omg`](compiler.omg)   | Source to bytecode: lexer, parser, code generator, `.omgb` writer. A single file with no imports, so it can be embedded anywhere. |
| [`vm.omg`](vm.omg)               | The VM: runs bytecode. It is what `omg foo.omg`, `omg foo.omgb` and the REPL execute on. |
| [`bytecode.omg`](bytecode.omg)   | Reads a `.omgb` file back into instructions, for the back ends and `--disasm`. |
| [`backends/native-x64.omg`](backends/native-x64.omg) | The machine-code back end: bytecode to x86-64, written out as an ELF (Linux) or PE (Windows) executable. Instruction encoder, code generator, linker and file writers in one module. Used by `--build`. |
| [`backends/native-c.omg`](backends/native-c.omg)   | The C back end: bytecode to one C file with `omg_rt.h` pasted in. Used by `--build --target c` and `--emit-c`. |
| [`backends/native-js.omg`](backends/native-js.omg) | The JS back end: bytecode to one JavaScript file with `omg_rt.js` pasted in. Used by `--emit-js` and by the web builds. |
| [`backends/backend.omg`](backends/backend.omg)     | What the back ends share: the scope analysis (is each name a parameter, a local, captured, or a global?) and the output buffer. |

None of these is a program in its own right. There used to be a binary
for three of them (`omgc`, `omgcc`, `omgjs`); `omg --compile`,
`omg --emit-c` and `omg --emit-js` do those jobs now.

`omg.omg` can also be *run* as a script, by `omg` itself, which puts
the whole toolchain on the VM. It is slow, and it is one of the
checks in `tests/parity.sh`:

```sh
bin/omg src/omg.omg --compile src/compiler.omg /tmp/compiler.omgb
```

## The browser drivers

| Source                                 | Becomes               | Role |
| -------------------------------------- | --------------------- | ---- |
| [`browser/omg-web.omg`](browser/omg-web.omg)           | `web/omg-web.js`      | The playground: compiler + VM. |
| [`browser/omg-explorer.omg`](browser/omg-explorer.omg) | `web/omg-explorer.js` | The compiler explorer: compiler + VM + the C and JS back ends, so the page can show every stage. |

Both are built by [`scripts/build-web.sh`](../scripts/build-web.sh), which is
`omg --compile` followed by `omg --emit-js`.

## Runtime files

A built program carries a runtime, and each back end has its own.

| Source                   | What it is | Where it ends up |
| ------------------------ | ---------- | ---------------- |
| [`runtime/x64/*.omg`](runtime/x64/) | The runtime for machine code, written in OMG in a restricted dialect ("raw mode": one machine word per value, memory and system calls through reserved `__names`). `core`, `float` and `builtins` are shared; `linux` and `windows` are the system layer. | Compiled by `omg --build` on every build and linked into the executable; only the procs the program reaches are kept |
| [`runtime/omg_rt.h`](runtime/omg_rt.h)   | The runtime in C | The top of every `.c` from `omg --build --target c` / `omg --emit-c` |
| [`runtime/omg_rt.js`](runtime/omg_rt.js) | The runtime in JavaScript | The top of every `.js` from `omg --emit-js` |

`build.sh` copies all of them into `bin/` (`bin/x64/`, `bin/omg_rt.h`,
`bin/omg_rt.js`), where `omg` finds them next to itself (through the
`executable_path` builtin). When `omg.omg` is run from source, it
reads the originals in `runtime/`.

The files in `runtime/x64/` are not ordinary OMG and cannot be run on
the VM. [`docs/06-runtime.md`](../docs/06-runtime.md) describes the
dialect and the runtime's design.

## How a build works

`scripts/build.sh` (`scripts/build.ps1` in PowerShell) builds
`bin/omg` (`omg.exe` on Windows) with the `omg` that is already there:

1. The runtime files are copied into `bin/`.
2. `omg --build src/omg.omg <tmp>/stage1/omg`: the existing `omg`
   compiles the new source, and everything it imports, to bytecode,
   and its x86-64 back end writes an executable. No other program
   runs.
3. `<tmp>/stage1/omg --build src/omg.omg bin/omg`: that executable
   does the same again. What is installed is therefore the new source
   built by the new source.
4. On Linux, stage 1 also writes `bin/omg.exe` with
   `--target windows-x64`.

The whole thing takes a few seconds, and rebuilding again gives the
same bytes.

On a system the machine-code back end does not cover (macOS, Linux on
ARM), steps 2 and 3 are `omg --emit-c src/omg.omg omg.c` and
`cc -O3 omg.c -o bin/omg -lm`, and the first `omg` comes from a
generated `omg.c` or from the frozen Rust runtime in `rust/`; see
[`docs/01-quickstart.md`](../docs/01-quickstart.md#other-systems).

Because the toolchain is built by its own previous build, its source
may only use language features the checked-in `bin/omg` already
understands ([`docs/05-extending.md`](../docs/05-extending.md#the-bootstrap-rule)).

After changing anything in this directory, rebuild and commit
`bin/` (and `web/` after `scripts/build-web.sh`): both are
checked in, and CI fails when they are stale
([`scripts/check-bin.sh`](../scripts/check-bin.sh)). `bin/` holds
two binaries, `omg` for Linux x86-64 and `omg.exe` for Windows x86-64,
and a build on Linux writes both.
