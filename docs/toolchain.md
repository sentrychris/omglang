# What each part is for

OMG has one language, one bytecode format, and several things that can
produce or run that bytecode. This page says what each of them is, why
it exists, and which one you want. It is the map; the other docs are
the territory.

## The short version

```
                 ┌─────────────┐
   foo.omg ────► │  compiler   │ ────► bytecode (.omgb) ────► one of:
                 └─────────────┘
                                         run it now      a VM
                                         make a program  the C back end, then `cc`
                                         make a web page the JS back end
```

- There are **two commands**, both called `omg`. One is written in Rust
  and one is written in OMG. They accept the same programs and print
  the same output.
- There is **one bytecode format**. Everything downstream of the
  compiler reads the same `.omgb`.
- There are **two back ends**, C and JavaScript. They are libraries
  inside the OMG-written `omg`, not separate tools.

## Which `omg` do I want?

|                                   | Rust `omg`                              | Native `omg`                         |
| --------------------------------- | --------------------------------------- | ------------------------------------ |
| Where it is                       | `rust/target/release/omg`            | `bin/omg` (`omg.exe` on Windows) |
| Built by                          | `cargo build --release`                 | `scripts/build.sh` (`build.ps1` on Windows) |
| Written in                        | Rust                                    | OMG (compiled to C, then by `cc`)    |
| Runs on                           | Linux, macOS, Windows                   | Linux, macOS, Windows                |
| Needs installed                   | nothing                                 | nothing to run; a C compiler for `--build` |
| Run a program, REPL               | yes                                     | yes                                  |
| `--compile`, `--disasm`           | yes                                     | yes                                  |
| `--build` (native executable)     | no                                      | yes                                  |
| `--emit-c`, `--emit-js`           | no                                      | yes                                  |
| `--verify-self-hosted`, `--verify-omg-vm` | yes                             | no                                   |

Use the native one if you want executables, C or JavaScript out of
your programs, or you want to use OMG with no Rust anywhere. Use the
Rust one if you are working on the language itself and want the
verification commands.

Both run on all three systems. A native binary is checked in for Linux
x86-64 and for Windows x86-64, so there a clone is all you need; on
macOS `scripts/build.sh` builds one. The C compiler that `--build`
calls is `cc` on Linux and macOS and GCC or Clang from MinGW-w64 on
Windows ([08-windows.md](08-windows.md) has the details
and the short list of things that behave differently there).

You never need both to *use* OMG. You need both to *develop* it,
because each checks the other.

## Why are there two?

The Rust runtime is the **seed**. A compiler written in OMG cannot
compile itself until something else has compiled it once, and that
something is the Rust frontend. It is also the reference: when the two
implementations disagree, the tests fail.

The native `omg` is the **destination**. It is the compiler, the VM
and both back ends, all written in OMG, turned into a normal executable
by its own C back end. Once it exists it rebuilds itself from its own
source and Rust is no longer involved:

```sh
omg --emit-c src/omg.omg omg.c    # the toolchain compiles itself to C
cc -O3 omg.c -o omg                          # and a C compiler does the rest
```

That is the whole of `scripts/build.sh` (and of `scripts/build.ps1`,
the same thing for PowerShell).

## The parts

Everything in OMG lives in `src/`:

| File            | What it is                                  | Used by |
| --------------- | ------------------------------------------- | ------- |
| `omg.omg`       | The `omg` command. Imports everything below and decides what to do with its arguments. | becomes `bin/omg` |
| `compiler.omg`  | Source text to bytecode: lexer, parser, code generator, `.omgb` writer. One file, no imports, so it can be embedded anywhere. | `omg.omg`, the Rust runtime (embedded), both web drivers |
| `vm.omg`        | Runs bytecode. This is what `omg foo.omg` uses in the native `omg`. | `omg.omg`, the Rust runtime (embedded, for `--verify-omg-vm`), both web drivers |
| `bytecode.omg`  | Reads a `.omgb` file back into instructions. | `omg.omg` |
| `native-c.omg`  | The C back end: bytecode to one C file. | `omg.omg` (`--build`, `--emit-c`), the explorer |
| `native-js.omg` | The JS back end: bytecode to one JavaScript file. | `omg.omg` (`--emit-js`), the explorer |
| `backend.omg`   | What the two back ends have in common: working out, for every name in every proc, whether it is a parameter, a local, captured from an enclosing proc, or a global. | `native-c.omg`, `native-js.omg` |
| `omg_rt.h`      | The C runtime: values, reference counting, builtins, errors. Pasted at the top of every generated C file. | everything `native-c.omg` produces |
| `omg_rt.js`     | The same job for JavaScript. | everything `native-js.omg` produces |
| `omg-web.omg`   | The playground's driver: compiler + VM, for the browser. | `web/omg-web.js` |
| `omg-explorer.omg` | The compiler explorer's driver: compiler + VM + both back ends, so the page can show every stage. | `web/omg-explorer.js` |

And the Rust side, in `rust/src/`:

| File                     | What it is |
| ------------------------ | ---------- |
| `lexer.rs`, `parser.rs`, `compiler.rs` | The Rust frontend. Does the same job as `compiler.omg` and must produce the same bytes. |
| `bytecode.rs`            | The `.omgb` format. The ground truth for opcode numbers and layout. |
| `vm.rs`, `vm/`           | The Rust VM and the builtins. |
| `main.rs`, `repl.rs`     | The Rust `omg` command. |

## Do we need the C back end? The JS one?

**C: yes.** It is not an optional extra. It is how the OMG-written
toolchain becomes a program you can run without Rust, and it is what
`omg --build` uses for your own programs. Remove it and the native
`omg` cannot exist.

**JS: yes, for the browser.** The playground and the compiler explorer
are the OMG compiler and VM put through the JS back end
(`scripts/build-web.sh`). Nothing else depends on it, so it costs
nothing when you are not using it.

**`vm.omg`: yes.** The Rust VM only exists inside the Rust `omg`. The
native `omg` needs a VM of its own to run a program without building
it first, and for the REPL.

## How many ways can a program run?

Five, and they are all the same bytecode underneath:

| # | How                                        | What executes the program |
| - | ------------------------------------------ | ------------------------- |
| 1 | `rust/target/release/omg foo.omg`       | the Rust VM |
| 2 | `rust/target/release/omg --rust src/vm.omg foo.omgb` | `vm.omg`, itself running on the Rust VM |
| 3 | `bin/omg foo.omg`                | `vm.omg`, compiled to machine code |
| 4 | `bin/omg --build foo.omg` then `./foo` | your program, compiled to machine code |
| 5 | `bin/omg --emit-js foo.omg foo.js` then `node foo.js` | your program, as JavaScript |

In daily use you will only meet 1, 3 and 4. Number 2 exists to prove
`vm.omg` correct, and 5 exists for the web. The conformance corpus in
`tests/conformance/` runs on all five and requires the same output.

## What has to stay in step

A few things are written twice on purpose (once in Rust, once in OMG),
and each has a check that fails when they drift:

| Written twice | Checked by |
| ------------- | ---------- |
| The compiler: `rust/src/compiler.rs` and `compiler.omg` must emit identical bytes | `omg --verify-self-hosted`, the smoke test, `tests/parity.sh` |
| The VM: `rust/src/vm.rs` and `vm.omg` must behave the same | `omg --verify-omg-vm`, the conformance corpus |
| Opcode numbers and the format version: `bytecode.rs`, and the tables in `compiler.omg`, `vm.omg`, `bytecode.omg` | `cargo test` (tests in `rust/src/bytecode.rs`) |
| The version string: `rust/Cargo.toml` and `omg.omg` | `cargo test` |
| Builtins: `rust/src/vm/builtins.rs`, `omg_rt.h`, `omg_rt.js` | the conformance corpus, `tests/builtins.sh` |
| The checked-in `bin/omg` and `web/*.js` against their sources | `scripts/check-bin.sh` and the web bundle check, both in CI |

[`docs/05-extending.md`](05-extending.md) walks through
changing each of these.

## Where to read next

- New to the language: the [README](../README.md) tour.
- How `omg foo.omg` runs, step by step: [compilation-pipeline.md](compilation-pipeline.md).
- The manual, page by page: [README.md](README.md).
- The Rust runtime's internals and flags: [rust/README.md](../rust/README.md).
- The browser builds: [web/README.md](../web/README.md).
