# `bootstrap/src/` — the OMG toolchain in OMG

Four of these files are compiled (by [`bootstrap/build.sh`](../build.sh))
into the ELFs that live in [`bootstrap/bin/`](../bin/). The other source
files (`vm.omg`, the runtime headers, the web bundle) are libraries or
inlined runtimes rather than standalone binaries. Each file's first
comment line records what it becomes so you can trace the mapping from
either end.

## Compiled to binaries

| Source                                       | Built binary                | Role                                                  |
| -------------------------------------------- | --------------------------- | ----------------------------------------------------- |
| [`compiler.omg`](compiler.omg)               | `bootstrap/bin/omgc`        | OMG frontend: `.omg` → `.omgb` bytecode               |
| [`native-c.omg`](native-c.omg)               | `bootstrap/bin/omgcc`       | C backend: `.omgb` → `.c` (splices in `omg_rt.h`)     |
| [`native-js.omg`](native-js.omg)             | `bootstrap/bin/omgjs`       | JS backend: `.omgb` → `.js` (splices in `omg_rt.js`)  |
| [`omg.omg`](omg.omg)                         | `bootstrap/bin/omg`         | Unified driver: run / compile / `--build` / REPL      |

Why two names for the same thing? Source files keep descriptive names
(`native-c.omg` says what it *is* — the C backend) while binaries follow
the Unix `omg*` convention so they sit cleanly on `$PATH` (`omgc`,
`omgcc`, `omgjs`).

## Libraries (imported, never compiled standalone)

| Source                                       | Where it's used                                                            |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| [`vm.omg`](vm.omg)                           | OMG-in-OMG VM. Embedded into `runtime/target/release/omg` (via cargo) and imported by `omg.omg` so `bin/omg foo.omgb` works in-process. Also used by `--verify-omg-vm` for the triple-meta fixed-point check. |
| [`omg-web.omg`](omg-web.omg)                 | Browser playground driver. Transpiled to `web/omg-web.js` by `bootstrap/build-web.sh`. |

## Runtime headers (inlined into transpiled output)

These two files are not compiled to anything by themselves. They get
*inlined* at the top of every transpiled output, so the resulting
program is self-contained.

| Source                          | Where it ends up                                                  |
| ------------------------------- | ----------------------------------------------------------------- |
| [`omg_rt.h`](omg_rt.h)          | Pasted at the top of every `.c` emitted by `omgcc` / `native-c.omg` |
| [`omg_rt.js`](omg_rt.js)        | Pasted at the top of every `.js` emitted by `omgjs` / `native-js.omg` |

`build.sh` also copies both into `bootstrap/bin/` so `omgcc` and `omgjs`
can find them via `executable_path` at transpile time.

## Runtime headers (not binaries)

These two files are not compiled to anything by themselves. They get
*inlined* at the top of every transpiled output, so the resulting
program is self-contained.

| Source                          | Where it ends up                                                  |
| ------------------------------- | ----------------------------------------------------------------- |
| [`omg_rt.h`](omg_rt.h)          | Pasted at the top of every `.c` emitted by `omgcc` / `native-c.omg` |
| [`omg_rt.js`](omg_rt.js)        | Pasted at the top of every `.js` emitted by `omgjs` / `native-js.omg` |

`build.sh` also copies both into `bootstrap/bin/` so `omgcc` and `omgjs`
can find them via `executable_path` at transpile time.

## How a build works

The full picture (cargo, build.sh, the self-rebuild loop, the AOT paths,
verify, packaging) lives at [`docs/flow/`](../../docs/flow/) — open
`index.html` for the interactive version, or read the JSON directly if
you prefer plain text.

Short version:

1. `cargo build --release --manifest-path runtime/Cargo.toml` produces
   `runtime/target/release/omg` — the Rust runtime, with `compiler.omg`
   and `vm.omg` pre-compiled to bytecode and **embedded inside the
   binary** so the resulting `omg` can do self-hosted compilation
   without reading any external file.
2. `bash bootstrap/build.sh` then uses that Rust runtime as the driver
   to translate every `.omg` source in this directory into a standalone
   ELF in `bootstrap/bin/`. For each source, it runs three steps
   internally (not commands you type):
   - `omg --compile X.omg /tmp/X.omgb` — Rust frontend produces bytecode
   - `omg native-c.omg /tmp/X.omgb /tmp/X.c` — `native-c.omg` running
     under the Rust VM transpiles the bytecode to C, with `omg_rt.h`
     spliced in at the top.
   - `cc -O2 /tmp/X.c -o bootstrap/bin/X -lm` — system C compiler
     produces the ELF.
3. After this finishes, you can re-run `bootstrap/build.sh` and it
   picks up `bootstrap/bin/omgc` + `bootstrap/bin/omgcc` instead of the
   Rust runtime — the toolchain rebuilds itself. cargo and the Rust
   runtime are no longer required.
