# 05 · Extending the compiler

Three kinds of changes, in increasing difficulty:

1. **New builtin** — easiest. Pure addition.
2. **New opcode** — medium. Touches every layer but the surface is small.
3. **New syntax form** — biggest. Parser + compiler + opcode + runtime.

The golden rule: **anything in the Rust toolchain has a counterpart in the
OMG toolchain**. Touch one, touch the other. Otherwise the fixed-point
check fails.

## The lockstep rule

| Rust layer             | OMG layer               | C layer (for native)         |
| ---------------------- | ----------------------- | ---------------------------- |
| `lexer.rs`             | first half of `compiler.omg` | (n/a — pre-bytecode)    |
| `parser.rs`            | second half `compiler.omg`   | (n/a — pre-bytecode)    |
| `compiler.rs`          | rest of `compiler.omg`  | (n/a — pre-bytecode)         |
| `bytecode.rs` (opcodes) | constants in `compiler.omg` + `vm.omg` + `native-c.omg` | constants in `omg_rt.h` if user-visible |
| `vm.rs` + `vm/ops_*.rs`| `vm.omg`                | inline emission in `native-c.omg`, helpers in `omg_rt.h` |
| `vm/builtins.rs`       | `cc_builtins` list in `compiler.omg` | `omg_call_builtin` switch in `omg_rt.h`, `emit_builtin` in `native-c.omg` |

After any change, run:

```sh
cd runtime && cargo build --release && cd ..
runtime/target/release/omg --verify-omg-vm bootstrap/compiler.omg
bootstrap/build-native-toolchain.sh
```

If the fixed-point check passes and the toolchain rebuilds, you're aligned.

---

## 1. Adding a new builtin

We'll add `square(x)` — returns `x * x`.

### Step A — Rust runtime

Add a match arm in [runtime/src/vm/builtins.rs](../../runtime/src/vm/builtins.rs):

```rust
"square" => match args {
    [Value::Int(i)] => Ok(Value::Int(i * i)),
    [Value::Float(f)] => Ok(Value::Float(f * f)),
    _ => Err(RuntimeError::TypeError(
        "square() expects one number".to_string(),
    )),
},
```

### Step B — Rust compiler allowlist

Add to [runtime/src/compiler.rs](../../runtime/src/compiler.rs), in `builtin_names()`:

```rust
"square",
```

This tells the compiler that calls to `square(...)` should compile to
`OP_BUILTIN("square", argc)` rather than a regular function call.

### Step C — OMG compiler allowlist

Add to the `cc_builtins` list in [bootstrap/compiler.omg](../../bootstrap/compiler.omg)
(don't forget the comma if it's not the last element):

```omg
alloc cc_builtins := [
    "chr", "ascii", "hex", "binary", "length", "read_file",
    ...
    "exit_with_error", "square"
]
```

### Step D — C runtime

Add the implementation to [bootstrap/omg_rt.h](../../bootstrap/omg_rt.h):

```c
static Value omg_builtin_square(Value v) {
    if (v.tag == OMG_INT)   return omg_int(v.v.i * v.v.i);
    if (v.tag == OMG_FLOAT) return omg_float(v.v.f * v.v.f);
    omg_panic("TypeError", "square() expects one number");
    return omg_none(); /* unreachable */
}
```

Also add it to `omg_call_builtin`'s switch (used by reflective dispatch):

```c
if (strcmp(n, "square") == 0)         return omg_builtin_square(a[0]);
```

### Step E — Native-c.omg dispatch

Add a case to `emit_builtin` in [bootstrap/native-c.omg](../../bootstrap/native-c.omg):

```omg
if name == "square" and argc == 1 { return emit_builtin1("omg_builtin_square") }
```

### Step F — Rebuild and verify

```sh
cd runtime && cargo build --release && cd ..
runtime/target/release/omg --verify-omg-vm bootstrap/compiler.omg
bootstrap/build-native-toolchain.sh

echo ';;;omg
emit square(7)' > /tmp/sq.omg
bootstrap/native/omg /tmp/sq.omg          # 49
bootstrap/native/omg --build /tmp/sq.omg /tmp/sq && /tmp/sq    # 49
```

Both paths should print `49`.

### Common mistakes

- Forgetting the OMG allowlist (Step C): the OMG compiler will compile
  `square` as a regular function call, fail to find it, and you'll see
  `UndefinedIdentError: square` — but only on the OMG-compiled path. The
  Rust frontend will work fine, masking the bug.
- Forgetting `omg_call_builtin` (Step D second part): `call_builtin("square", ...)`
  will fail in native binaries even though direct calls work.
- Forgetting `--verify-omg-vm`: silent bytecode drift.

---

## 2. Adding a new opcode

Opcodes live in five places. Adding one is more work but each spot is small.

We'll add `OP_DUP` — pushes a copy of the stack top.

### Step A — Bytecode definition

[runtime/src/bytecode.rs](../../runtime/src/bytecode.rs):

```rust
pub enum Instr {
    // ... existing variants ...
    Dup,
}

// Pick a free numeric tag — bump the existing high-water mark
pub const OP_DUP: u8 = 56;

// Add to (de)serialization tables
```

### Step B — Rust VM dispatch

[runtime/src/vm.rs](../../runtime/src/vm.rs):

```rust
Instr::Dup => {
    if let Some(top) = stack.last() {
        stack.push(top.clone());
    } else {
        break Err(RuntimeError::VmInvariant("stack underflow on Dup".into()));
    }
}
```

### Step C — Rust compiler emission

If `Dup` is going to be emitted by the parser/compiler, add the emit calls
in [compiler.rs](../../runtime/src/compiler.rs). If it's only used by
`native-c.omg`, you can skip this.

### Step D — OMG-side opcode constants

In each of these files, add `alloc OP_DUP := 56`:

- [bootstrap/compiler.omg](../../bootstrap/compiler.omg)
- [bootstrap/vm.omg](../../bootstrap/vm.omg)
- [bootstrap/native-c.omg](../../bootstrap/native-c.omg)

### Step E — OMG VM dispatch

In [vm.omg](../../bootstrap/vm.omg)'s `step_inner` switch:

```omg
if op == "DUP" {
    alloc top := vm_stack[length(vm_stack) - 1]
    vm_push(top)
    vm_pc := vm_pc + 1
    return false
}
```

(And register the opcode in vm.omg's decoder if it has its own — currently
both the OMG compiler and OMG VM share decoding logic in `compiler.omg`.)

### Step F — OMG compiler decoder

In [compiler.omg](../../bootstrap/compiler.omg)'s `decode_one`:

```omg
if op == OP_DUP { return tagged0("DUP", cursor) }
```

(Plus the matching encoder if the OMG compiler emits `Dup`.)

### Step G — Native-c emit handler

In [native-c.omg](../../bootstrap/native-c.omg)'s `emit_c_for_instr`:

```omg
if op == "DUP" {
    return "    { Value v = stack[sp - 1]; omg_inc(v); stack[sp++] = v; }\n"
}
```

Note the `omg_inc` — duplicating a refcounted value bumps the count so both
slots own a reference.

### Step H — Rebuild and verify

```sh
cd runtime && cargo build --release && cd ..
runtime/target/release/omg --verify-omg-vm bootstrap/compiler.omg
bootstrap/build-native-toolchain.sh
```

The fixed-point check is your safety net: if the OMG-side decoder doesn't
match the Rust-side serialization, this will fail.

---

## 3. Adding a new syntax form

Hardest case, but mechanical once you've got the muscle memory.

Suppose we want to add `unless cond { ... }` — the inverse of `if`.

### Step A — Lexer (if needed)

If `unless` is a new keyword, register it as such in:

- [runtime/src/lexer.rs](../../runtime/src/lexer.rs) — Rust lexer
- The token-class lookup in [bootstrap/compiler.omg](../../bootstrap/compiler.omg)

If it's a soft keyword (just an identifier with special meaning to the
parser), you can skip this — but soft keywords are usually a mistake.

### Step B — Parser

Add an `Unless(cond, body)` AST node. In each parser, recognize the
keyword and produce that node.

- Rust: [runtime/src/parser.rs](../../runtime/src/parser.rs)
- OMG: [bootstrap/compiler.omg](../../bootstrap/compiler.omg)

### Step C — Compiler

Lower `Unless` to existing bytecode. `unless cond { body }` is just
`if !cond { body }` so we can emit:

```
<compile cond>
JumpIfFalse end_label   ; jump over body if cond is FALSE — wait, that's wrong
```

Inverted, so use `Not` first:

```
<compile cond>
Not
JumpIfFalse end_label
<compile body>
end_label:
```

Or implement a `JumpIfTrue` opcode if it's worth optimizing.

Lockstep this in both `compiler.rs` and `compiler.omg`.

### Step D — Verify

The fixed-point check covers it: compile `bootstrap/compiler.omg` (which
hopefully now uses `unless` for some test) three ways and ensure they
match.

---

## Common pitfalls across all three

### Forgetting `vm.omg`

If you only update Rust + native-c, the OMG-in-OMG VM falls behind. The
fixed-point check is `--verify-omg-vm` — it runs `compiler.omg` *on*
`vm.omg`, so an out-of-date `vm.omg` will fail to interpret new opcodes.

### Bytecode versioning

`bootstrap/compiler.omg` has `BC_VERSION := 257` (and matching in `bytecode.rs`).
If you change the bytecode format incompatibly, bump this. Otherwise old
`.omgb` files will silently break.

### Refcount discipline

When emitting C for a new opcode, follow the established pattern:

- Pop with `--sp` (no `omg_dec`) when transferring ownership downstream.
- Pop with `omg_dec(stack[--sp])` when discarding.
- Push fresh values (rc=1) directly; push existing values with `omg_inc`.
- Binary ops dec both operands after computing.

See [06-runtime.md](06-runtime.md) for the full protocol.

### Don't skip `--verify-omg-vm`

Seriously. It's the single check that catches most lockstep bugs. CI should
run it.

---

## A worked example

There's a regression test for an actual compiler bug we fixed earlier:

- The bug: `return` / `break` / tail-call inside `try` didn't pop the
  exception-handler block, leaving a stale handler that caught later panics.
- The fix lived in *both* `compiler.rs` and `compiler.omg`, because both
  emit `PopBlock` instructions.
- The test: [tools/tests/control_flow_in_try.omg](../../tools/tests/control_flow_in_try.omg)

That test exercises four leak paths and should always pass on every
runtime. Use it as a template for adding regression tests when you fix
similar bugs.

## Read next

- [06-runtime.md](06-runtime.md) — the C runtime header in detail
- [07-debugging.md](07-debugging.md) — diagnosing what went wrong
