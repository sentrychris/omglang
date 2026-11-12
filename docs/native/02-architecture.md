# 02 · Architecture

OMG runs on three substrates. Knowing which is which prevents most confusion.
(For a one-page list of every part and what it is for, see
[../toolchain.md](../toolchain.md).)

## The three substrates

```
       ┌──────────────────────────────────────────────────┐
       │  OMG: the language. Source files are .omg        │
       │  Bytecode files are .omgb                        │
       │  All "interesting" logic — compiler, parser,     │
       │  back ends, meta-circular VM — is OMG.           │
       └──────────────────────────────────────────────────┘
                            │
                            │ runs on
                            ▼
       ┌────────────────────────────────────┐
       │  Rust: the production VM.          │     Two ways to
       │  runtime/src/vm.rs interprets      │ ◄── execute OMG
       │  bytecode at full speed.           │     bytecode.
       └────────────────────────────────────┘     Pick one.
                            │
       ┌────────────────────────────────────┐
       │  C: the native target.             │     The other way:
       │  bootstrap/src/native-c.omg generates  │ ◄── compile to C,
       │  C source from bytecode; cc        │     then ELF.
       │  produces an ELF.                  │
       └────────────────────────────────────┘
```

That's it. Rust and C don't both exist at runtime — they're two **alternative**
ways to run OMG bytecode. Rust hosts the VM if you stay on the bytecode path;
C compiles to native and disappears once `cc` finishes.

## Who hosts what

| Phase                    | Substrate | What's running                |
| ------------------------ | --------- | ----------------------------- |
| Editing source           | (none)    | A `.omg` text file            |
| `.omg` → `.omgb`         | OMG-on-VM | `compiler.omg` running on the VM |
| Picked the VM path: run  | Rust      | `vm.rs` interpreting bytecode |
| Picked the AOT path: build | OMG + cc | `native-c.omg` (inside `omg`) emits C; cc compiles |
| AOT path: run            | OS + CPU  | Native ELF, no VM, no Rust    |

The "running on the VM" rows can use either the Rust VM (`runtime/src/vm.rs`)
or the OMG-in-OMG VM (`bootstrap/src/vm.omg`) — but the OMG-in-OMG VM is itself
bytecode that runs on a host VM, so something Rust-or-C-shaped is always at
the bottom.

## The bootstrap chain

```
bootstrap/src/compiler.omg          (OMG source — the compiler)
        │
        │ compiled by
        ▼
bootstrap/src/compiler.omgb          (bytecode — what the runtime executes)
        │
        │ produced by
        ▼
the previous compiler.omgb (or the Rust frontend the very first time)
```

It's a fixed point: each `compiler.omgb` was produced by an earlier
`compiler.omgb` (or originally by the Rust frontend) compiling
`compiler.omg`. The build is reproducible — given source, the output is
byte-identical no matter which historical iteration you started from.

## What the Rust runtime *is* exactly

`runtime/target/release/omg` is a single binary that bundles three things:

1. **A frontend** — Rust code that parses `.omg` directly. Used by `--compile`
   for speed; otherwise the OMG-in-OMG compiler is used.
2. **A bytecode VM** — `vm.rs` plus `vm/ops_*.rs`. This is what runs the OMG
   compiler, the OMG transpiler, and any `.omgb` you give it.
3. **Embedded `compiler.omgb` and `vm.omgb`** — the OMG-in-OMG compiler and
   VM are shipped inside the binary as bytecode (via Rust's `include_bytes!`),
   so the very first run can compile and execute `.omg` sources without the
   user already having either.

The native toolchain (`bootstrap/bin/omg`) is the same three things
with no Rust: the OMG compiler and the OMG VM, compiled to C and fused
into one binary together with the C and JS back ends. `bin/omg`
imports `compiler.omg` and `vm.omg` in-process, so `omg foo.omg` and
`omg foo.omgb` need nothing else.

## Native toolchain inventory

```
bootstrap/bin/
├── omg         the toolchain: run / REPL / --compile / --build /
│               --emit-c / --emit-js / --disasm, all in-process
├── omg_rt.h    C runtime header  (pasted into every .c omg generates)
└── omg_rt.js   JS runtime        (pasted into every .js omg generates)
```

`omg` is a native ELF compiled from OMG source: it is written in OMG
(see [`bootstrap/src/omg.omg`](../../bootstrap/src/omg.omg)) and runs
its imports in-process — the compiler, the VM (which is why running a
precompiled `.omgb` doesn't need a separate ELF) and the two back
ends. `subprocess()` is only invoked for the final `cc` call during
`--build`.

It is produced by `bootstrap/build.sh`, which:

1. Copies `omg_rt.h` and `omg_rt.js` into `bootstrap/bin/` so generated
   code is built on the current runtime.
2. Runs `omg --emit-c bootstrap/src/omg.omg omg.c`: the driver compiles
   its own source (and everything it imports) to bytecode and puts that
   through the C back end.
3. Runs `cc -O3 omg.c` to produce the new `bootstrap/bin/omg`.

The `omg` in step 2 is `bootstrap/bin/omg` itself when one that runs
on this machine is already there. Otherwise it is the Rust runtime
running `omg.omg` from source.

**The script is idempotent.** Run it again and `omg` rebuilds itself
with no Rust involvement, and the result is byte-for-byte the same
binary: `omg` compiling `omg.omg` reproduces `omg`. (In the same
checkout, that is. The binary records the full paths of its sources
for tracebacks, so a build in another directory differs in those
strings and nothing else.)

## The fixed-point check

```
runtime/target/release/omg --verify-omg-vm bootstrap/src/compiler.omg
```

This compiles `compiler.omg` three different ways:

1. **Rust frontend** parsing `.omg` → bytecode
2. **OMG compiler** running on **Rust VM** → bytecode
3. **OMG compiler** running on **OMG VM** running on **Rust VM** → bytecode

…and asserts all three produce **byte-identical** output. That proves:

- The OMG compiler (`compiler.omg`) is a faithful re-implementation of the
  Rust frontend — they agree on every byte of every program.
- The OMG-in-OMG VM (`vm.omg`) executes bytecode with the same observable
  behavior as the Rust VM.
- Therefore the chain is self-consistent: changes to the language don't
  introduce drift between the three.

If you change `compiler.rs`, you almost always need a corresponding change
in `compiler.omg`, or this check will fail. See [05-extending.md](05-extending.md).

## What's *not* OMG

- `runtime/src/` — Rust. Production VM, parser, frontend, CLI.
- `bootstrap/src/omg_rt.h` — C. Value representation, refcounting, builtins,
  setjmp-based exception handling, call-frame stack + traceback formatter.
  ~2200 lines. Linked into every AOT binary.
- `bootstrap/src/omg_rt.js` — JavaScript. The same job for programs
  generated by `omg --emit-js`, and for the browser playground.

That's the whole non-OMG surface. Everything else is `.omg`.

## Why bother with all three?

- **Rust is the seed.** You need *something* to bootstrap from when there's
  no compiler yet. Rust is fast, well-tested, and gives us a rigorous
  reference implementation.
- **OMG is the home.** Once we have a compiler, everything moves to OMG.
  Self-hosting means the language can evolve without dragging Rust along.
- **C is the exit ramp.** OMG bytecode in a VM is portable but always pays
  dispatch cost. C lets us emit straight-line native code with zero runtime
  overhead beyond a small refcount runtime.

You don't have to use all three — pick whichever fits your task. Need a
30 KB CLI tool? `omg --build`. Need to iterate fast? `omg foo.omg`. Want
to embed OMG in a Rust app? Use the runtime crate directly.

## Read next

- [03-language-tour.md](03-language-tour.md) — write OMG programs
- [04-pipeline.md](04-pipeline.md) — what each compilation stage does
- [05-extending.md](05-extending.md) — add features in lockstep across all three
