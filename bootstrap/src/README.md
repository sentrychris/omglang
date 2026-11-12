# `bootstrap/src/` — the OMG toolchain in OMG

Everything here is OMG source, plus the two runtime files that
generated code needs. One file, [`omg.omg`](omg.omg), is the program;
the rest are modules it imports. [`bootstrap/build.sh`](../build.sh)
turns it into [`bootstrap/bin/omg`](../bin/).

For what each part is for and how they relate to the Rust runtime, see
[`docs/toolchain.md`](../../docs/toolchain.md).

## The program

| Source               | Becomes             | Role |
| -------------------- | ------------------- | ---- |
| [`omg.omg`](omg.omg) | `bootstrap/bin/omg` | The `omg` command: run, REPL, `--compile`, `--build`, `--emit-c`, `--emit-js`, `--disasm` |

## The modules it imports

| Source                           | Role |
| -------------------------------- | ---- |
| [`compiler.omg`](compiler.omg)   | Source to bytecode: lexer, parser, code generator, `.omgb` writer. A single file with no imports. Also embedded in the Rust runtime (`cargo build` compiles it to `compiler.omgb`), where it is the default frontend. |
| [`vm.omg`](vm.omg)               | The OMG-in-OMG VM: runs bytecode. It is what `bin/omg foo.omg` and `bin/omg foo.omgb` execute on. Also embedded in the Rust runtime for the `--verify-omg-vm` fixed-point check. |
| [`bytecode.omg`](bytecode.omg)   | Reads a `.omgb` file back into instructions, for the back ends and `--disasm`. |
| [`native-c.omg`](native-c.omg)   | The C back end: bytecode to one C file with `omg_rt.h` pasted in. Used by `--build` and `--emit-c`. |
| [`native-js.omg`](native-js.omg) | The JS back end: bytecode to one JavaScript file with `omg_rt.js` pasted in. Used by `--emit-js` and by the web builds. |
| [`backend.omg`](backend.omg)     | What the two back ends share: the scope analysis (is each name a parameter, a local, captured, or a global?) and the output buffer. |

None of these is a program in its own right. There used to be a binary
for three of them (`omgc`, `omgcc`, `omgjs`); `omg --compile`,
`omg --emit-c` and `omg --emit-js` do those jobs now.

`compiler.omg` and `vm.omg` can still be *run* as scripts by the Rust
runtime, because that is how it uses its embedded copies:

```sh
runtime/target/release/omg --rust bootstrap/src/vm.omg foo.omgb
```

## The browser drivers

| Source                                 | Becomes               | Role |
| -------------------------------------- | --------------------- | ---- |
| [`omg-web.omg`](omg-web.omg)           | `web/omg-web.js`      | The playground: compiler + VM. |
| [`omg-explorer.omg`](omg-explorer.omg) | `web/omg-explorer.js` | The compiler explorer: compiler + VM + both back ends, so the page can show every stage. |

Both are built by [`bootstrap/build-web.sh`](../build-web.sh), which is
`omg --compile` followed by `omg --emit-js`.

## Runtime files (pasted into generated code)

These two files are not compiled to anything by themselves. They are
pasted at the top of every file a back end generates, so the result is
self-contained.

| Source                   | Where it ends up |
| ------------------------ | ---------------- |
| [`omg_rt.h`](omg_rt.h)   | The top of every `.c` from `omg --build` / `omg --emit-c` |
| [`omg_rt.js`](omg_rt.js) | The top of every `.js` from `omg --emit-js` |

`build.sh` copies both into `bootstrap/bin/`, where `omg` finds them
next to itself (through the `executable_path` builtin).

## How a build works

1. `cargo build --release --manifest-path runtime/Cargo.toml` produces
   `runtime/target/release/omg` — the Rust runtime, with `compiler.omg`
   and `vm.omg` pre-compiled to bytecode and **embedded inside the
   binary** so it can do self-hosted compilation without reading any
   external file. This step is only needed when there is no
   `bootstrap/bin/omg` that runs on your machine.
2. `bootstrap/build.sh` builds `bootstrap/bin/omg` in two commands:
   - `omg --emit-c bootstrap/src/omg.omg omg.c` — the driver compiles
     its own source, and everything it imports, to bytecode, and puts
     that through the C back end with `omg_rt.h` pasted in at the top.
   - `cc -O3 omg.c -o bootstrap/bin/omg -lm` — the system C compiler
     produces the ELF. `-O3` inlines the runtime helpers (`omg_eq`,
     `omg_index`, `omg_dict_find` etc.) which compound in the VM's hot
     path; the output is a single translation unit, so `-flto` adds no
     value here.

   The `omg` in the first command is `bootstrap/bin/omg` itself if it
   runs here. If not, it is the Rust runtime running the driver from
   source: `runtime/target/release/omg --rust bootstrap/src/omg.omg`.
3. From then on `bootstrap/build.sh` is a self-rebuild: the toolchain
   compiles itself, and cargo and the Rust runtime are no longer
   required. Rebuilding twice gives the same bytes both times.

After changing anything in this directory, rebuild and commit
`bootstrap/bin/` (and `web/` after `bootstrap/build-web.sh`): both are
checked in, and CI fails when they are stale
([`bootstrap/check-bin.sh`](../check-bin.sh)).
