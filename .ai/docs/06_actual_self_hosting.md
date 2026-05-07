# OMGlang — Actual Self-Hosting

Earlier docs said the runtime was "fully self-hosted". That was wrong: a Rust
implementation of OMG isn't self-hosting any more than CPython is "Python in
Python". This document records what was needed to make the project actually
self-hosting, and how the proof works.

## Definition

A language is **self-hosting** when its compiler/interpreter can be expressed
in itself. For OMG specifically:

> There exists an OMG program `compiler.omg` whose execution on the OMG VM
> reads OMG source and produces bytecode equivalent to what the bootstrap
> compiler produces — *including* when applied to its own source.

The fixed-point clause is what makes the claim rigorous. Without it,
"compiler in OMG" could be a stub that handles `Hello, World` and nothing
else.

## What ships

| Artefact                          | Role                                                       |
| --------------------------------- | ---------------------------------------------------------- |
| `bootstrap/compiler.omg`          | The OMG-in-OMG compiler. ~1900 lines, single file.         |
| `bootstrap/compiler.omgb`         | Stage-1 image: compiler.omg compiled by the Rust frontend. |
| `runtime/src/compiler.rs`         | Stage-0 (bootstrap) Rust frontend.                         |
| `runtime/src/main.rs`             | Embeds `compiler.omgb` and exposes `--self-hosted` and `--verify-self-hosted`. |
| `runtime/build.rs`                | At `cargo build` time, uses the Rust frontend to (re)compile `compiler.omg` → `compiler.omgb`. |

The Rust frontend remains the **default** for performance: `omg foo.omg`
compiles in-process via the Rust frontend (~3 ms for `prime_sieve`). The
self-hosted path is opt-in:

```sh
omg --self-hosted foo.omg            # compile via the OMG-written compiler, then run
omg --verify-self-hosted foo.omg     # compile both ways, assert byte-identical
```

## Architecture

```
        ┌────────────────────────────┐
        │      bootstrap/compiler.omg │   <-- OMG source (the OMG compiler)
        └─────────────┬──────────────┘
                      │
                      │ build.rs at `cargo build`
                      │ (Rust stage-0 compiles OMG source)
                      ▼
        ┌────────────────────────────┐
        │   bootstrap/compiler.omgb   │   <-- bytecode of the OMG compiler
        └─────────────┬──────────────┘
                      │
                      │ include_bytes! into the runtime binary
                      ▼
        ┌────────────────────────────┐
        │     omg_runtime executable │
        └─────────────┬──────────────┘
                      │
              ┌───────┴────────┐
              │                │
         user .omg        --self-hosted
              │                │
              ▼                ▼
   Rust stage-0 compile   Stage-1 compile (compiler.omgb runs on the VM
   (fast, default)         and emits its own bytecode)
              │                │
              └────────┬───────┘
                       │
                       ▼
                run via VM
```

## What the OMG compiler covers

`bootstrap/compiler.omg` is a complete OMG → `.omgb` translator. It owns:

* **Lexer** — single-pass scanner over the source string. Strips the
  `;;;omg` header, decodes string escapes, recognises every keyword and
  operator that the Rust lexer recognises.
* **Parser** — recursive descent with the same precedence table (10 levels)
  and the same statement forms, including `try`/`except`, `import`, `proc`
  with first-class binding, attribute and index assignment.
* **Compiler** — AST → tagged-instruction list. Implements:
  * Native imports with the same module-mangling scheme as the Rust frontend
    (`__mod_N__name`).
  * Short-circuit `and`/`or` codegen.
  * Direct `Call` for top-level procs vs `Load + CallValue` for closures and
    parameter-as-function patterns.
  * `MakeFunc` after every `proc` so first-class function references work.
  * Tail-call optimisation for direct calls in tail position.
  * `StoreLocal` for `alloc`/import-alias/except-binding so `alloc args := ...`
    inside a function never clobbers the runtime-injected `args` global.
* **Bytecode writer** — emits the on-disk `.omgb` binary, including
  little-endian integer encoding, length-prefixed UTF-8 strings, and a
  function table sorted by name (so output is deterministic regardless of
  hash-map iteration order).

## The fixed-point check

Run by `omg --verify-self-hosted bootstrap/compiler.omg`:

1. Read `bootstrap/compiler.omg` from disk.
2. Compile it via the Rust frontend → byte stream `R`.
3. Compile it via the OMG frontend (i.e. run the embedded `compiler.omgb`
   on the same source) → byte stream `O`.
4. Assert `R == O`.

If the assertion holds, the OMG-written compiler reproduces its own
bootstrap byte-for-byte. That's the fixed point.

```sh
$ omg --verify-self-hosted bootstrap/compiler.omg
OK  bootstrap/compiler.omg (48554 bytes) — self-hosted output matches Rust output
```

It also holds for every program in `examples/`:

```sh
$ for ex in examples/*.omg; do omg --verify-self-hosted "$ex"; done
OK  examples/assignment.omg (271 bytes) — self-hosted output matches Rust output
OK  examples/bitwise.omg (596 bytes) — self-hosted output matches Rust output
OK  examples/dictionaries.omg (192 bytes) — self-hosted output matches Rust output
... (all 15 examples)
```

## Bootstrapping plan

The Rust frontend is **stage-0**. The OMG frontend is **stage-1**. There is no
stage-2 — the OMG frontend's image is itself the artefact distributed.

* On a fresh check-out, `cargo build` runs `build.rs`, which uses the Rust
  stage-0 to recompile `bootstrap/compiler.omg` into
  `bootstrap/compiler.omgb`.
* `main.rs` `include_bytes!`s that `.omgb` so the published binary is
  self-contained.
* If you mutate `bootstrap/compiler.omg`, the next `cargo build` regenerates
  `compiler.omgb`. To verify your change preserves the fixed point, run
  `omg --verify-self-hosted bootstrap/compiler.omg`.

This dependency on Rust at build time is the standard self-hosting model:
GHC needs a previous GHC, Rust needs a previous Rust, OMG needs the Rust
frontend in this repo. The crucial property is that **the language can
compile itself** — not that no other tool is involved in the build.

## Performance

| Compile target           | Stage-0 (Rust) | Stage-1 (OMG) | Stage-1 / Stage-0 |
| ------------------------ | -------------- | ------------- | ----------------- |
| `examples/hello_world.omg` | ~1 ms          | ~50 ms        | 50×               |
| `examples/prime_sieve.omg` | ~2 ms          | ~200 ms       | 100×              |
| `bootstrap/compiler.omg`   | ~30 ms         | ~85 s         | ~3000×            |

The 50–3000× slowdown is expected: the OMG compiler runs as bytecode on the
VM, and its hot path is dominated by O(n²) list appends (every emitted
instruction is `xs := xs + [instr]`, which allocates and copies). The Rust
frontend stays the default precisely so this cost only shows up when you
explicitly ask for the self-hosted path.

A future optimisation pass would add mutable list extension (or a builtin
list-buffer) to drop the self-compile time below 5 seconds.

## What changed in the runtime to make this work

| Change                                                | Reason                                                                 |
| ----------------------------------------------------- | ---------------------------------------------------------------------- |
| `Instr::StoreLocal` opcode (53)                       | `alloc` must always create a fresh local; otherwise inner functions overwrite same-named globals (notably `args`). |
| Sorted function table in `write_bytecode`              | HashMap iteration order is non-deterministic; without sorting, the same input produces different bytes between runs. |
| Removed extra `PushNone` after `Raise` in compiler.omg | The Rust compiler emits no follow-up after `Raise`; matching that behaviour was required for the fixed point. |
| `build.rs` includes the runtime modules to bootstrap   | `cargo build` now performs the stage-0 → stage-1 compilation in-process. |

## What's still on the Rust side

* The VM (stack machine, exception unwinding, closure invocation).
* The bytecode loader.
* All built-in functions (`length`, `chr`, `ascii`, `freeze`, file I/O, …).
* The REPL shell.

These are the substrate. The same structure exists in any self-hosted
language: GHC has a C runtime, Rust has LLVM and a tiny C runtime, OMG has
the Rust VM. **Self-hosting describes the compiler/interpreter, not the
substrate.**

## Honest one-liner

> OMG is self-hosting: `bootstrap/compiler.omg` is an OMG program that
> compiles OMG source — including its own — into the same `.omgb` bytes the
> Rust bootstrap produces. The Rust runtime hosts the VM and is the build-
> time bootstrap; from a language-design point of view, the compiler is
> written in OMG.
