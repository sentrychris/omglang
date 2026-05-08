# The substrate runtime for OMG execution

the Rust VM acts as the **execution substrate** for OMG: it provides the concrete machine that runs OMG bytecode

```txt
OMG compiler source
    ↓ compiled by OMG compiler
compiler.omgb
    ↓ embedded into Rust VM
Rust VM executes compiler.omgb
    ↓
new compiler output
```

But the **compiler authority** has moved to OMG itself, because the compiler is now self-hosted and reproducibly produces the same compiler artifacts.

| Layer               | Role                                     |
| ------------------- | ---------------------------------------- |
| OMG compiler source | Canonical compiler implementation        |
| `compiler.omgb`     | Compiled compiler artifact               |
| Rust VM             | Substrate VM / bytecode execution engine |
| Host OS / hardware  | Physical execution substrate             |

It's no longer accurate to say “OMG depends on the Rust compiler implementation as the source of truth.” The Rust VM is now more like the initial execution engine or host substrate.