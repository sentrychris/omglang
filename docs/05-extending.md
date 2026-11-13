# 05 · Extending the compiler

Three kinds of changes, in increasing difficulty:

1. **New builtin** — easiest. Pure addition.
2. **New opcode** — medium. Touches every layer but the surface is small.
3. **New syntax form** — biggest. Parser + compiler + opcode + runtime.

The golden rule: **anything in the Rust toolchain has a counterpart in the
OMG toolchain**. Touch one, touch the other. Otherwise the fixed-point
check fails.

## The lockstep rule

| Rust layer             | OMG layer               | C and JS layers (for native and the web) |
| ---------------------- | ----------------------- | ---------------------------- |
| `lexer.rs`             | first half of `compiler.omg` | (n/a — pre-bytecode)    |
| `parser.rs`            | second half `compiler.omg`   | (n/a — pre-bytecode)    |
| `compiler.rs`          | rest of `compiler.omg`  | (n/a — pre-bytecode)         |
| `bytecode.rs` (opcodes) | constants in `compiler.omg` + `vm.omg` + `bytecode.omg` | constants in `omg_rt.h` if user-visible |
| `vm.rs` + `vm/ops_*.rs`| `vm.omg`                | inline emission in `native-c.omg` and `native-js.omg`, helpers in `omg_rt.h` and `omg_rt.js` |
| `vm/builtins.rs`       | `cc_builtins` list in `compiler.omg` | `omg_call_builtin` switch in `omg_rt.h`, `emit_builtin` in `native-c.omg` and `native-js.omg`, the function itself in `omg_rt.h` and `omg_rt.js` |

Two things are *not* duplicated and need changing in one place only:
how names resolve in generated code (`backend.omg`, shared by both
back ends) and how a `.omgb` is read back for them (`bytecode.omg`).

`cargo test` compares the opcode tables and the format version in the
three OMG files with `bytecode.rs`, so a missed or mistyped constant
fails there before it can produce bad bytecode.

After any change, run:

```sh
cd rust && cargo build --release && cd ..
rust/target/release/omg --verify-omg-vm src/compiler.omg
scripts/build.sh
```

If the fixed-point check passes and the toolchain rebuilds, you're aligned.

---

## 1. Adding a new builtin

We'll add `square(x)` — returns `x * x`.

### Step A — Rust runtime

Add a match arm in [rust/src/vm/builtins.rs](../rust/src/vm/builtins.rs):

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

Add to [rust/src/compiler.rs](../rust/src/compiler.rs), in `builtin_names()`:

```rust
"square",
```

This tells the compiler that calls to `square(...)` should compile to
`OP_BUILTIN("square", argc)` rather than a regular function call.

### Step C — OMG compiler allowlist

Add to the `cc_builtins` list in [src/compiler.omg](../src/compiler.omg)
(don't forget the comma if it's not the last element):

```omg
alloc cc_builtins := [
    "chr", "ascii", "hex", "binary", "length", "read_file",
    ...
    "exit_with_error", "square"
]
```

### Step D — C runtime

Add the implementation to [src/runtime/omg_rt.h](../src/runtime/omg_rt.h):

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

Add a case to `emit_builtin` in [src/backends/native-c.omg](../src/backends/native-c.omg):

```omg
if name == "square" and argc == 1 { return emit_builtin1("omg_builtin_square") }
```

The JS back end needs the same two things, or the builtin will be
missing in the playground and from `omg --emit-js`: a function in
[src/runtime/omg_rt.js](../src/runtime/omg_rt.js) and a case in
`emit_builtin` in [src/backends/native-js.omg](../src/backends/native-js.omg).

### Step F — Rebuild and verify

```sh
cd rust && cargo build --release && cd ..
rust/target/release/omg --verify-omg-vm src/compiler.omg
scripts/build.sh

echo ';;;omg
emit square(7)' > /tmp/sq.omg
bin/omg /tmp/sq.omg          # 49
bin/omg --build /tmp/sq.omg /tmp/sq && /tmp/sq    # 49
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

[rust/src/bytecode.rs](../rust/src/bytecode.rs):

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

[rust/src/vm.rs](../rust/src/vm.rs):

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
in [compiler.rs](../rust/src/compiler.rs). If it's only used by
`native-c.omg`, you can skip this.

### Step D — OMG-side opcode constants

In each of these files, add `alloc OP_DUP := 56`:

- [src/compiler.omg](../src/compiler.omg) (it writes the opcode)
- [src/vm.omg](../src/vm.omg) (it dispatches on it)
- [src/bytecode.omg](../src/bytecode.omg) (it reads it back for the C and JS back ends)

`cargo test` fails until all three match `bytecode.rs`.

### Step E — OMG VM dispatch

In [vm.omg](../src/vm.omg)'s `step_inner` switch. `step_inner`
pre-advances `vm_pc` before dispatching, so a normal-flow opcode skips
its own pc-increment; only `JUMP` / `JUMP_IF_FALSE` / `CALL` / `RET`
override the pre-advanced pc:

```omg
if op == OP_DUP {
    alloc top := vm_stack[vm_stack_top - 1]
    vm_push(top)
    return false
}
```

Opcodes are dispatched on integer tags (the `OP_*` constants at the top
of vm.omg) — the OMG-in-OMG compiler emits string-tagged tuples and
`vm_extend_code` normalises them to ints on ingest, so step_inner only
ever sees ints. Also register a new entry in vm.omg's `OPNAME_TO_INT`
table if the OMG compiler emits this opcode.

### Step F — OMG VM decoder

In [vm.omg](../src/vm.omg)'s `decode_one`:

```omg
if op == OP_DUP { return tagged0(OP_DUP, cursor) }
```

(Plus the matching encoder in [compiler.omg](../src/compiler.omg)
if the OMG compiler emits `Dup`.)

The back ends read bytecode through a second decoder, `decode_one` in
[bytecode.omg](../src/bytecode.omg), which keeps opcode
names as strings. Add the opcode there as well:

```omg
if op == OP_DUP { return tagged0("DUP", cursor) }
```

### Step G — Native-c emit handler

In [native-c.omg](../src/backends/native-c.omg)'s `emit_c_for_instr`:

```omg
if op == "DUP" {
    return "    { Value v = stack[sp - 1]; omg_inc(v); stack[sp++] = v; }\n"
}
```

Note the `omg_inc` — duplicating a refcounted value bumps the count so both
slots own a reference.

And the same opcode in [native-js.omg](../src/backends/native-js.omg)'s
`emit_js_for_instr`, where there is no reference counting to do.

### Step H — Rebuild and verify

```sh
cd rust && cargo build --release && cd ..
rust/target/release/omg --verify-omg-vm src/compiler.omg
scripts/build.sh
```

The fixed-point check is your safety net: if the OMG-side decoder doesn't
match the Rust-side serialization, this will fail.

---

## 3. Adding a new syntax form

Hardest case, but mechanical once you've got the muscle memory.

Suppose we want to add `unless cond { ... }` — the inverse of `if`.

### Step A — Lexer (if needed)

If `unless` is a new keyword, register it as such in:

- [rust/src/lexer.rs](../rust/src/lexer.rs) — Rust lexer
- The token-class lookup in [src/compiler.omg](../src/compiler.omg)

If it's a soft keyword (just an identifier with special meaning to the
parser), you can skip this — but soft keywords are usually a mistake.

### Step B — Parser

Add an `Unless(cond, body)` AST node. In each parser, recognize the
keyword and produce that node.

- Rust: [rust/src/parser.rs](../rust/src/parser.rs)
- OMG: [src/compiler.omg](../src/compiler.omg)

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

The fixed-point check covers it: compile `src/compiler.omg` (which
hopefully now uses `unless` for some test) three ways and ensure they
match.

---

## Common pitfalls across all three

### Forgetting `vm.omg`

If you only update Rust + native-c, the OMG-in-OMG VM falls behind. The
fixed-point check is `--verify-omg-vm` — it runs `compiler.omg` *on*
`vm.omg`, so an out-of-date `vm.omg` will fail to interpret new opcodes.

### Bytecode versioning

`src/compiler.omg` has `BC_VERSION := 512` (`0x000200` —
`(MAJOR<<16)|(MINOR<<8)|PATCH`), matching `BC_VERSION` in
`rust/src/bytecode.rs` and the same constant in
`src/{vm,bytecode}.omg`. If you change the
bytecode format incompatibly, bump this in **all four** places
(`cargo test` fails if you miss one). Old
`.omgb` files are rejected at load time with a clear "recompile your
.omgb" SyntaxError — there's no silent breakage, but also no fallback
to v1.

The current format carries a source-file table and a per-instruction
source map (parallel to the code blob) that the VMs use to render
Python-style tracebacks. Both new sections were added at the v1 → v2
bump.

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
- The test: [tests/regression/control_flow_in_try.omg](../tests/regression/control_flow_in_try.omg)

That test exercises four leak paths and should always pass on every
runtime. Use it as a template for adding regression tests when you fix
similar bugs.

## Read next

- [06-runtime.md](06-runtime.md) — the C runtime header in detail
- [07-debugging.md](07-debugging.md) — diagnosing what went wrong
