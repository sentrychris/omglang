# The substrate runtime for OMG execution

OMG bytecode needs *something* to execute it. There are now two answers
to "what's the substrate":

1. **The Rust VM** — `runtime/src/vm.rs`. The default. Interprets bytecode
   directly, fast and well-tested.
2. **A native ELF binary** — produced by transpiling bytecode to C via
   `bootstrap/native-c.omg` and compiling with `cc -O2`. No VM at runtime;
   the OS and CPU host the resulting machine code directly.

```txt
                 OMG compiler source
                        │
                        │ compiled by OMG compiler (running on VM)
                        ▼
                  compiler.omgb
                        │
              ┌─────────┴─────────┐
              ▼                   ▼
        Rust VM executes    native-c.omg + cc
        (interprets)         produce native ELF
              │                   │
              └─────────┬─────────┘
                        ▼
                  program output
```

The **compiler authority** is OMG itself: the compiler is self-hosted
and reproducibly produces the same artifacts byte-for-byte. The Rust VM
and the native ELF path are two ways of *running* what the OMG compiler
produces.

| Layer                  | Role                                                  |
| ---------------------- | ----------------------------------------------------- |
| OMG compiler source    | Canonical compiler implementation                     |
| `compiler.omgb`        | Compiled compiler artefact                            |
| Rust VM                | One execution substrate (interprets bytecode)         |
| Native ELF + `omg_rt.h` | Other execution substrate (compiled C, runs directly) |
| Host OS / hardware     | Physical execution substrate, common to both          |

It's no longer accurate to say "OMG depends on the Rust compiler
implementation as the source of truth." Even calling the Rust VM "the
substrate" is now a simplification — for native binaries, the OS and CPU
*are* the substrate, with `omg_rt.h` providing a small refcounting
runtime in C.

See [`docs/native/02-architecture.md`](native/02-architecture.md) for the
full picture of how Rust, OMG, and C fit together.
