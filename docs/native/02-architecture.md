# 02 · Architecture

OMG runs on three substrates. Knowing which is which prevents most confusion.

## The three substrates

```
       ┌──────────────────────────────────────────────────┐
       │  OMG: the language. Source files are .omg        │
       │  Bytecode files are .omgb                        │
       │  All "interesting" logic — compiler, parser,     │
       │  transpiler, meta-circular VM — is OMG.          │
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
       │  bootstrap/native-c.omg generates  │ ◄── compile to C,
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
| Picked the AOT path: build | OMG-on-VM + cc | `native-c.omg` runs on the VM emitting C; cc compiles |
| AOT path: run            | OS + CPU  | Native ELF, no VM, no Rust    |

The "running on the VM" rows can use either the Rust VM (`runtime/src/vm.rs`)
or the OMG-in-OMG VM (`bootstrap/vm.omg`) — but the OMG-in-OMG VM is itself
bytecode that runs on a host VM, so something Rust-or-C-shaped is always at
the bottom.

## The bootstrap chain

```
bootstrap/compiler.omg          (OMG source — the compiler)
        │
        │ compiled by
        ▼
bootstrap/compiler.omgb          (bytecode — what the runtime executes)
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
3. **An embedded `compiler.omgb`** — the OMG-in-OMG compiler is shipped
   inside the binary as bytecode, so the very first run can compile `.omg`
   sources without the user already having a compiler.

The native toolchain (`bootstrap/native/`) replaces #1 and #2 with C-compiled
versions: `omgc` is #1 + #3 fused into a binary, `omgvm` is #2.

## Native toolchain inventory

```
bootstrap/native/
├── omg         shell wrapper: pick the right tool by file ext
├── omg-build   shell wrapper: AOT in one shot
├── omgc        compiler.omg compiled to native
├── omgcc       native-c.omg compiled to native (transpiler)
├── omgvm       vm.omg compiled to native (interpreter)
└── omg_rt.h    C runtime header (linked into AOT outputs)
```

These are produced by `bootstrap/build-native-toolchain.sh`, which:

1. Uses the Rust binary (or itself, if already present) to compile each of
   `compiler.omg`, `native-c.omg`, `vm.omg` to bytecode.
2. Runs `native-c.omg` on each bytecode file to emit C.
3. Runs `cc -O2` to produce the three native binaries.
4. Copies `omg_rt.h` next to them so AOT builds find it.

**The script is idempotent.** Run it again and the native binaries rebuild
themselves with no Rust involvement (as long as `omgc`/`omgcc` are already
in place from a prior build).

## The fixed-point check

```
runtime/target/release/omg --verify-omg-vm bootstrap/compiler.omg
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
- `bootstrap/omg_rt.h` — C. Value representation, refcounting, builtins,
  setjmp-based exception handling. ~1600 lines. Linked into every AOT binary.

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
