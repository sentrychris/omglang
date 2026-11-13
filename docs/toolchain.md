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
                                         run it now      the VM
                                         make a program  the x86-64 back end
                                                         (or the C back end, then `cc`)
                                         make a web page the JS back end
```

- There is **one command**, `omg`. It is written in OMG, and it is the
  whole toolchain: compiler, VM and back ends in one executable.
- There is **one bytecode format**. Everything downstream of the
  compiler reads the same `.omgb`.
- There are **three back ends**: x86-64 machine code, C and
  JavaScript. They are libraries inside `omg`, not separate tools.
- Nothing else is involved. On Linux and Windows (x86-64) `omg` writes
  executables itself, its own included: no C compiler, no linker, no
  other language.

## What `omg` needs

|                                   | Linux, Windows (x86-64)        | macOS, Linux on ARM, others |
| --------------------------------- | ------------------------------ | --------------------------- |
| Where it is                       | `bin/omg`, `bin\omg.exe`, checked in | built once from generated C (see below) |
| Run a program, REPL, `--compile`, `--disasm`, `--emit-c`, `--emit-js` | nothing | nothing |
| `--build` (native executable)     | nothing                        | a C compiler (`cc`)         |
| Rebuild `omg` itself              | nothing                        | a C compiler (`cc`)         |

The machine-code back end covers x86-64 on Linux and Windows. Any
`omg` can write for either of them with `--target linux-x64` or
`--target windows-x64`, whatever system it runs on. `--target c` goes
through C instead, and is the default wherever the machine-code back
end has nothing to offer. [08-windows.md](08-windows.md) has the
Windows details and the short list of things that behave differently
there; [01-quickstart.md](01-quickstart.md#other-systems) has the
steps for other systems.

## How `omg` builds itself

`omg` is an OMG program (`src/omg.omg`), so it builds itself the way
it builds anything:

```sh
omg --build src/omg.omg bin/omg
```

That is the whole of `scripts/build.sh` (and of `scripts/build.ps1`,
the same thing for PowerShell), run twice so that the binary left in
`bin/` is the new source built by itself. It takes a few seconds. A
third pass writes the same bytes, and the tests check that it does.

Where the build has to go through C, it is two commands:

```sh
omg --emit-c src/omg.omg omg.c    # the toolchain compiles itself to C
cc -O3 omg.c -o omg -lm           # and a C compiler does the rest
```

## Where it came from: the Rust seed

A compiler written in OMG cannot compile itself until something else
has compiled it once. That something was a runtime written in Rust,
which lives in `rust/`. It is **frozen**: it stays in the repository
at the last state where it matched the language, CI no longer builds
it, and nothing in daily use touches it. It is kept for two reasons:

- It is the proof that OMG can be rebuilt from source alone. The
  frozen Rust runtime builds the `omg` of its commit, and each `omg`
  builds the next.
- It is a route onto a system where there is no `omg` yet and no
  other machine to generate C on.

New language features are not added to it. From the freeze on, the
checked-in binaries are the seed: every `omg` is built by the one
before it. [rust/README.md](../rust/README.md) says exactly where it
was frozen and how to bootstrap from it.

## The parts

Everything is in `src/`:

| File            | What it is                                  | Used by |
| --------------- | ------------------------------------------- | ------- |
| `omg.omg`       | The `omg` command. Imports everything below and decides what to do with its arguments. | becomes `bin/omg` |
| `compiler.omg`  | Source text to bytecode: lexer, parser, code generator, `.omgb` writer. One file, no imports, so it can be embedded anywhere. | `omg.omg`, both web drivers |
| `vm.omg`        | Runs bytecode. This is what `omg foo.omg` uses. | `omg.omg`, both web drivers |
| `bytecode.omg`  | Reads a `.omgb` file back into instructions. | `omg.omg` |
| `backends/native-x64.omg` | The machine-code back end: bytecode to x86-64, wrapped as an ELF file (Linux) or a PE file (Windows). Encoder, code generator and linker in one. | `omg.omg` (`--build`) |
| `backends/native-c.omg`  | The C back end: bytecode to one C file. | `omg.omg` (`--build --target c`, `--emit-c`), the explorer |
| `backends/native-js.omg` | The JS back end: bytecode to one JavaScript file. | `omg.omg` (`--emit-js`), the explorer |
| `backends/backend.omg`   | What the back ends have in common: working out, for every name in every proc, whether it is a parameter, a local, captured from an enclosing proc, or a global. | all three back ends |
| `runtime/x64/*.omg` | The runtime for machine code, written in OMG: values, memory, strings, floats, builtins, errors (`core`, `float`, `builtins`), and one file per system for talking to it (`linux`, `windows`). Compiled into every executable `--build` writes. | everything `native-x64.omg` produces |
| `runtime/omg_rt.h`  | The same job in C. Pasted at the top of every generated C file. | everything `native-c.omg` produces |
| `runtime/omg_rt.js` | The same job for JavaScript. | everything `native-js.omg` produces |
| `browser/omg-web.omg` | The playground's driver: compiler + VM, for the browser. | `web/omg-web.js` |
| `browser/omg-explorer.omg` | The compiler explorer's driver: compiler + VM + the C and JS back ends, so the page can show every stage. | `web/omg-explorer.js` |

The runtime files are also installed next to the binary (`bin/x64/`,
`bin/omg_rt.h`, `bin/omg_rt.js`), because `omg` reads them from the
folder it lives in when it generates a program.

## Do we need three back ends?

**x86-64: yes.** It is what makes OMG stand on its own. It is how the
toolchain becomes a program without anyone else's compiler, and it is
what `omg --build` uses for your programs.

**C: yes, for reach and as a check.** It is the route to every system
the machine-code back end does not cover (macOS, ARM), and where both
exist the tests build every program both ways and compare.

**JS: yes, for the browser.** The playground and the compiler explorer
are the OMG compiler and VM put through the JS back end
(`scripts/build-web.sh`). Nothing else depends on it, so it costs
nothing when you are not using it.

**`vm.omg`: yes.** It runs a program without building it first, and it
is the REPL.

## How many ways can a program run?

Four, and they are all the same bytecode underneath:

| # | How                                        | What executes the program |
| - | ------------------------------------------ | ------------------------- |
| 1 | `omg foo.omg`                              | `vm.omg`, itself compiled to machine code |
| 2 | `omg --build foo.omg` then `./foo`         | your program, compiled to machine code |
| 3 | `omg --build foo.omg --target c` then `./foo` | your program, compiled to C and then by `cc` |
| 4 | `omg --emit-js foo.omg foo.js` then `node foo.js` | your program, as JavaScript |

In daily use you will meet 1 and 2. Number 3 is what 2 means on
systems without the machine-code back end, and 4 exists for the web.
The conformance corpus in `tests/conformance/` runs on all four and
requires the same output.

## What has to stay in step

A few things are written more than once on purpose, and each has a
check that fails when they drift:

| Written more than once | Checked by |
| ---------------------- | ---------- |
| Builtins and value semantics: `vm.omg`, `runtime/x64/`, `omg_rt.h`, `omg_rt.js` | the conformance corpus (every program states what it must print, and runs every way), `tests/builtins.sh`, `tests/parity.sh` |
| Opcode numbers and the format version: the tables in `compiler.omg`, `vm.omg` and `bytecode.omg` | `tests/driver.sh` compares the three tables line for line, and `omg --disasm` against a fixed listing |
| The compiler against itself: `omg` compiling `compiler.omg`, and that compiler (run from source) compiling it again, must give the same bytes | `tests/parity.sh` |
| The checked-in `bin/omg`, `bin/omg.exe` and `web/*.js` against their sources | `scripts/check-bin.sh` and the web bundle check, both in CI |
| The toolchain against itself: rebuilding `omg` twice must give the same file | `tests/parity.sh`, `tests/standalone.sh` |

[`docs/05-extending.md`](05-extending.md) walks through
changing each of these.

## Where to read next

- New to the language: the [README](../README.md) tour.
- How `omg foo.omg` runs, step by step: [compilation-pipeline.md](compilation-pipeline.md).
- The manual, page by page: [README.md](README.md).
- The frozen Rust runtime: [rust/README.md](../rust/README.md).
- The browser builds: [web/README.md](../web/README.md).
