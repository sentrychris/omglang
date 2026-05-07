# OMGlang — Weaknesses and Bugs

Issues are grouped by severity. Each one cites the file and line it lives in,
plus a minimal reproducer where useful.

## Critical

### C1. Build script hard-codes `python`

[`runtime/build.rs:15`](../../runtime/build.rs#L15) calls
`Command::new("python")`. On a default Ubuntu/WSL/Debian install only
`python3` exists; the build panics:

```
thread 'main' panicked at build.rs:21:10:
failed to run compiler: Os { code: 2, kind: NotFound }
```

The script also assumes that `python -m omglang.compiler` finds `omglang`
on `sys.path`, which only happens if the parent of `runtime/` is on
`PYTHONPATH` or the user happens to run cargo from the repo root with the
right shell environment. Neither requirement is documented in
[`runtime/README.md`](../../runtime/README.md).

**Fix sketch**: try `python3` first, fall back to `python`; explicitly set
`PYTHONPATH` to `CARGO_MANIFEST_DIR/..`.

### C2. List `+` mutates its left operand

[`runtime/src/vm/ops_arith.rs:59-65`](../../runtime/src/vm/ops_arith.rs#L59):

```rust
(Value::List(la), Value::List(lb)) => {
    {
        let mut la_mut = la.borrow_mut();
        la_mut.extend(lb.borrow().iter().cloned());
    }
    stack.push(Value::List(la));
}
```

This **mutates** `la` rather than producing a new list. Because lists are
`Rc<RefCell<...>>`, every alias of `a` is now extended.

Repro:

```omg
;;;omg
alloc a := [1, 2]
alloc b := [3, 4]
alloc c := a + b
emit a   # prints [1, 2, 3, 4]  (should be [1, 2])
emit c   # prints [1, 2, 3, 4]  (correct)
```

The Python interpreter creates a new list (`lhs + rhs` via Python's `list.__add__`),
so the same program prints `[1, 2]` followed by `[1, 2, 3, 4]`. This is a
serious semantic divergence that will cause subtle bugs in any program that
holds onto an "old" list reference.

**Fix**: clone `la.borrow()` into a fresh `Vec` before extending, or
explicitly construct a new `Rc<RefCell<Vec<Value>>>`.

### C3. `python -m omglang.compiler` cannot compile programs with `import`

[`omglang/compiler.py:287-290`](../../omglang/compiler.py#L287):

```python
elif kind == "import":
    raise NotImplementedError(
        "Module imports are resolved by the interpreter and cannot be compiled",
    )
```

Result: the bytecode path (Path B in `01_architecture.md`) cannot be used
for any multi-file program. Users who want to ship a `.omgb` for
`examples/import_modules.omg` simply cannot. The Rust runtime falls back to
re-running the source through the embedded interpreter (Path C), which is
the slow, divergent path.

Either the compiler should resolve imports at compile time (inline
modules, build a function table per module, or emit module-load bytecode),
or the documentation must be honest that bytecode is single-file only.

### C4. Self-hosted interpreter is unusably slow

`runtime/target/debug/omg_runtime bootstrap/test_interpret.omg` does not
finish in 60 seconds. The program is `fact(5)` defined in a string and then
executed via `OMGInterpreter.run(source)`.

Likely culprits in [`bootstrap/interpreter.omg`](../../bootstrap/interpreter.omg):

* Lists are used as both AST nodes and environment frames; lookup is linear.
* `env_set` is structurally append-only on a list-of-pairs (a typical
  signature for these implementations) — every variable read scans an O(n)
  list.
* The interpreter has no tail-call optimisation of its own; recursion in the
  user program multiplies the per-recursion cost by the embedded
  interpreter's evaluator.

This makes Path C (the *only* way to run multi-file programs through the
Rust runtime) impractical for anything beyond the simplest loops. It also
means the headline claim — "native runtime written in Rust" — is misleading:
non-trivial OMG programs run via a 1000-line OMG interpreter on top of that
Rust VM, not directly on it.

### C5. `prime_sieve.omg` is 14× slower under the Rust runtime than under Python

Measured (debug build):

```
.omg via Rust runtime  : 525 ms
.omg via Python omg.py :  36 ms
.omgb via Rust runtime :   3 ms
```

The Rust VM is fast in isolation (the bytecode path is 12× faster than
Python). The .omg path is slow because, again, it's running the user
program through the embedded self-hosted interpreter.

The combined effect of C3 + C5 is that the bytecode advantage is invisible
to users: simple programs run as fast bytecode, but the moment a program
needs `import`, performance falls off a cliff.

## High

### H1. `==` and `!=` compare stringified values

[`runtime/src/vm/ops_arith.rs:117-128`](../../runtime/src/vm/ops_arith.rs#L117):

```rust
let b = pop(stack)?.to_string();
let a = pop(stack)?.to_string();
stack.push(Value::Bool(a == b));
```

So `5 == "5"` is `true` in the Rust VM but `false` in the Python interpreter.
This is a documented "feature" only in the source comments, not in
[`spec/OMG_SPEC.md`](../../spec/OMG_SPEC.md). Either:

* The spec should explicitly state "type-coerced equality"; or
* The VM should compare typed values (the cheaper, more conventional path).

### H2. Integer overflow silently returns `0`

[`runtime/src/vm/ops_arith.rs:89`](../../runtime/src/vm/ops_arith.rs#L89):

```rust
stack.push(Value::Int(a.checked_mul(b).unwrap_or(0)));
```

Repro: `emit 1000000000 * 10000000000` prints `0`. The Python interpreter
prints `10000000000000000000` (Python ints are arbitrary precision).

Choosing 0 as the "fallback" value is worse than panicking or wrapping; it
silently corrupts results and there is no way for the program to detect it.
At minimum this should `Err(RuntimeError::ValueError("integer overflow"))`.

Note `Add`, `Sub`, `Shl`, etc. don't even use checked arithmetic — they
use `+`, `-`, `<<` directly, which panics in debug and wraps in release.

### H3. Integer division semantics differ between implementations

| Expression | Python interpreter | Rust runtime |
| ---------- | ------------------ | ------------ |
| `-7 / 2`   | `-4`               | `-3`         |
| `-7 % 2`   | `1`                | `-1`         |

Python uses floor division (`//`), Rust uses truncated division (the `i64`
default). Same source program → different output depending on which path
the user takes. The spec doesn't say which should win.

### H4. `raise` and `panic` aren't real Python builtins

[`omglang/interpreter.py`](../../omglang/interpreter.py) has no handlers for
`raise(...)` or `panic(...)`. The compiler treats them magically:

```python
ERROR_NAME_TO_KIND = {
    "panic": "Generic",
    "raise": "Generic",
    "_omg_vm_syntax_error_handle": "Syntax",
    ...
}
```
([`compiler.py:96`](../../omglang/compiler.py#L96))

So `examples/self_hosted.omg` and `bootstrap/interpreter.omg` — which both
call `raise(...)` — work under the Rust runtime path but fail under
`python3 omg.py` with `Undefined variable 'raise'`. The Python interpreter
should add real `raise` / `panic` builtins (or stop pretending these names
exist).

### H5. `store_index` past end auto-grows lists with zeros

[`runtime/src/vm/ops_struct.rs:201-208`](../../runtime/src/vm/ops_struct.rs#L201):

```rust
if idx_usize >= l.len() {
    l.resize(idx_usize + 1, Value::Int(0));
}
l[idx_usize] = val;
```

Repro:

```omg
;;;omg
alloc a := [1, 2]
a[5] := 99
emit a   # Rust:    [1, 2, 0, 0, 0, 99]
         # Python:  RuntimeError: List index out of bounds
```

Two problems: (a) silent divergence from the Python interpreter; (b)
zero-fill is a poor sentinel — it conflates "I deliberately stored 0" with
"this slot was auto-created".

### H6. `store_index` and `store_attr` fall through silently on type mismatch

[`runtime/src/vm/ops_struct.rs:218`](../../runtime/src/vm/ops_struct.rs#L218):

```rust
_ => {}   // <-- silently swallow
```

Repro:

```omg
;;;omg
alloc a := [1, 2, 3]
a["foo"] := 99    # silently no-ops; should be a TypeError
alloc d := {a: 1}
d[true] := 99     # silently no-ops; Bool isn't handled
emit a            # [1, 2, 3]
emit d            # {a: 1}
```

These should emit `RuntimeError::TypeError`. The same issue applies to
`store_attr` ([line 266](../../runtime/src/vm/ops_struct.rs#L266)).

### H7. Slicing a non-list/non-string silently returns `Int(0)`

[`runtime/src/vm/ops_struct.rs:186-188`](../../runtime/src/vm/ops_struct.rs#L186):

```rust
// Invalid base → push dummy 0 (VM design choice)
_ => stack.push(Value::Int(0)),
```

The Python interpreter raises `TypeError`. The "design choice" comment
documents the divergence but doesn't justify it.

### H8. The Python parser disallows numeric dict keys; the embedded interpreter accepts them

* `omglang/parser/expressions.py:94-104` only accepts `STRING` or `ID` for
  dict keys — `{1: "a"}` raises `SyntaxError: Invalid dict key 1`.
* `bootstrap/interpreter.omg` has its own parser that accepts numeric
  keys, and `BuildDict` stringifies all keys
  ([`ops_struct.rs:52`](../../runtime/src/vm/ops_struct.rs#L52)) so it works
  via the embedded interpreter.

The same source code parses in one tool and not the other.

## Medium

### M1. Two parsers, two lexers, no shared grammar

[`bootstrap/interpreter.omg`](../../bootstrap/interpreter.omg) reimplements
the lexer and parser in OMG. There is no shared grammar definition; any
change to one is a manual port to the other. Already drifting (M1):
the embedded lexer disallows `=` (line 92) where Python's lexer doesn't even
mention it; the embedded lexer skips C-style `/* */` while Python supports
docblock `/** */`.

### M2. Builtin shadowing semantics differ

If a user writes `proc length(x) { return 999 }`, then calls `length([...])`:

* The **Python interpreter** still calls the builtin (because the builtin
  check in [`interpreter.py:467`](../../omglang/interpreter.py#L467) runs
  before user-function lookup).
* The **Rust runtime** calls the user function, because the compiler decides
  at compile time based on a hard-coded set
  ([`compiler.py:133`](../../omglang/compiler.py#L133)).

This is a real divergence. Either pick "builtins win" or "user wins" and
enforce it in both paths.

### M3. Unknown bytecode opcodes are silently skipped

[`runtime/src/bytecode.rs:355-356`](../../runtime/src/bytecode.rs#L355):

```rust
// Unknown opcode: no-op decode (advance already consumed 1 byte).
_ => {}
```

This fails open. A corrupted or version-mismatched `.omgb` will load and
execute as if those bytes weren't there. The version check just before this
asserts a strict equality, but unknown opcode handling contradicts that
spirit. Recommend `panic!()` or returning a parse error.

### M4. `assert!(version == BC_VERSION)` panics instead of returning an error

[`runtime/src/bytecode.rs:208`](../../runtime/src/bytecode.rs#L208) uses
`assert_eq!` and `assert!` for header validation. A malformed file will
abort with a Rust panic message rather than a clean
`RuntimeError`. `parse_bytecode` should return `Result`.

### M5. `repl.rs` re-spawns the binary per turn

[`runtime/src/repl.rs:117-119`](../../runtime/src/repl.rs#L117): every input
serializes the accumulated history to a temp file and re-invokes
`current_exe()`. This is correct in spirit (state is the program text, not
in-memory) but slow and visible: each REPL turn pays a full process startup
cost plus parse + compile + execute. It also explains the `last_output`
diffing hack — every prior `emit` has to re-print and be filtered out.

A real REPL would keep an interpreter resident; the repl module already
imports nothing from `vm` so this would not be a major refactor.

### M6. `Ret` and call frames panic on misuse

[`runtime/src/vm/ops_control.rs:198-201`](../../runtime/src/vm/ops_control.rs#L198):

```rust
*pc = ret_stack.pop().unwrap();
*env = env_stack.pop().unwrap();
```

If the bytecode is malformed (e.g., a stray `Ret` outside a call), the VM
panics. Same shape in `handle_call` / `handle_call_value`. Should be a
`RuntimeError::VmInvariant`.

### M7. `as_int()` truthiness is a footgun

[`runtime/src/value.rs:66-78`](../../runtime/src/value.rs#L66) coerces
*everything* to an integer, including lists/dicts (returns length) and `None`
(returns `0`). This means `5 + [1,2,3]` computes `5 + 3 = 8` instead of
erroring. The Python interpreter raises `TypeError` for the same expression.

This makes type errors invisible until they produce wrong answers far
downstream.

### M8. File-handle table is process-wide across REPL turns

[`runtime/src/vm/builtins.rs:60`](../../runtime/src/vm/builtins.rs#L60) uses
a `Lazy<Mutex<...>>` static. Combined with the REPL's per-turn process
spawning (M5), this means handles do **not** persist across REPL turns —
each turn starts in a fresh process with an empty table. Likewise, the
Python interpreter uses a class-level `Interpreter.file_handles` table that
*does* persist. Confusion is guaranteed.

### M9. `comparison()` is non-associative but not enforced

The grammar produces left-associative trees for `<` and `==`, but doesn't
forbid `1 < 2 < 3`, which parses as `((1 < 2) < 3)` and computes
`(true) < 3` → `1 < 3` → `true`. Most languages either disallow this with a
parser rule or interpret it as conjunction. OMG silently does the worst of
both. Worth at least documenting.

## Low

### L1. `chr` truncates to one byte, ignoring Unicode

[`runtime/src/vm/builtins.rs:103`](../../runtime/src/vm/builtins.rs#L103):

```rust
"chr" => match args {
    [Value::Int(i)] => Ok(Value::Str((*i as u8 as char).to_string())),
```

`chr(0x1F600)` returns the byte `0x00` truncated, not 😀. The Python
interpreter calls Python's `chr()`, which handles the full Unicode range.

### L2. `hex()` differs in case

* Python: `hex(255)` → `"FF"` (upper-cased after stripping `0x`)
  ([`interpreter.py:491`](../../omglang/interpreter.py#L491))
* Rust: `hex(255)` → `"ff"` (`format!("{:x}", i)`)
  ([`builtins.rs:122`](../../runtime/src/vm/builtins.rs#L122))

The `examples/hex_to_rgb.omg` script prints `FF8800` because it composes
hex on its own; if it had used `hex()` directly, the two paths would
disagree.

### L3. `binary()` width math may overflow at boundary

`binary(n, 64)` computes `(1 << 64) - 1` as an `i64`, which overflows.
Python builds `mask` with arbitrary-precision integers and is fine. Rust
silently wraps.

### L4. `Slice` behaves differently for negative inputs vs Python

Negative indices are allowed in Python's `s[-1:]`. OMG uniformly errors on
negatives in both implementations. Worth documenting in
[`spec/OMG_SPEC.md`](../../spec/OMG_SPEC.md), which says "Python-style
indexing and slicing supported" — that's not quite true.

### L5. `expressions.py` parses unary `+` and `-` as separate paths from `_factor`

[`omglang/parser/expressions.py:48-54`](../../omglang/parser/expressions.py#L48)
treats unary `+`/`-` at factor level and emits `('unary', Op.ADD/SUB, ...)`
nodes. The interpreter then has to special-case unary ADD/SUB to mean
`+x` and `-x` rather than binary addition. This is harmless but
counter-intuitive — unary nodes carry the same `Op` enum as binary ones,
distinguished only by the `'unary'` tag.

### L6. Tests that shell out to `cargo run` are slow and brittle

`omglang/tests/test_native_*.py` depend on `cargo run` succeeding, which in
turn depends on the build environment having `python` (issue C1) and the
cargo tree being built. They can't be run in isolation from the build
toolchain. A small `omg_runtime --bytecode-stdin` mode would let them
just `popen` the runtime binary instead.

### L7. `repl.rs` echo is fragile

The "diff stdout against last run" logic in
[`repl.rs:142-146`](../../runtime/src/repl.rs#L142) silently breaks if a
previous run prints something nondeterministic (e.g., a wall-clock time)
or if `print` ordering changes between runs. The REPL will then re-print
old output. Resident in-process state (M5) would obviate this.

### L8. `comparison` with mixed types via `as_int()`

`"abc" < 5` returns `false` because `"abc".as_int()` errors and... actually
that's `RuntimeError::TypeError` propagated, OK. But the chain is
non-obvious from reading the code.

### L9. Documentation drift

The README claims "implementation is stable and complete as a toy language
but may evolve to support additional features like deeper type
introspection". This understates the divergences between the three execution
paths and the broken bytecode-with-imports path. The spec at `spec/OMG_SPEC.md`
also doesn't mention `try`/`except` or the file I/O builtins, despite both
being implemented.

### L10. Dead code in `bytecode.rs`

The Rust parser handles opcodes 47–51 as `Raise(Kind)` short variants that
no compiler ever emits ([`bytecode.rs:350-354`](../../runtime/src/bytecode.rs#L350)).
Either remove the cases or have the compiler use them and shave one byte
per `Raise`.

### L11. `expression statement` parser does best-effort backtracking

[`omglang/parser/statements.py:121-138`](../../omglang/parser/statements.py#L121)
saves `position`/`curr_token`, tries to parse an lvalue, and on
`except Exception` rewinds and parses an expression-statement instead.
That's fine, but the `except Exception` is over-broad and will swallow
real bugs (e.g., a typo in the parser itself).
