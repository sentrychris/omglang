# OMGlang — Recommendations

Concrete, prioritised suggestions. Each links back to issue IDs from
[`03_weaknesses_and_bugs.md`](03_weaknesses_and_bugs.md).

## Tier 1 — Pick the canonical implementation

The single biggest improvement available to the project is to **decide what
"running OMG" means** and remove the divergences. Right now there are three
different answers:

* `python3 omg.py foo.omg` — tree-walk, no `raise`/`panic`, floor division.
* `omg_runtime foo.omgb` — bytecode, fast, can't handle `import`.
* `omg_runtime foo.omg` — embedded interpreter, slow, double-interpreted.

Pick one as the **canonical** runtime; ensure the others either match its
semantics exactly or are removed. My suggestion: make the bytecode path
(B) canonical and resolve imports at compile time (C3).

If imports are resolved by the compiler:

* The compiler walks each `import` and recursively compiles imported files
  into one combined function table, qualifying their names (e.g.
  `math.is_prime` → a real function entry).
* Module-level `alloc`s become `Store`s into a frozen-namespace global.
* The embedded `interpreter.omg` becomes a fun curiosity rather than the
  default execution path.

This eliminates issues C3, C4, C5, M1, H4 (since `raise`/`panic` only need
to live in one place), and most cross-implementation drift in one move.

## Tier 2 — Critical bug fixes (small, immediate)

Even without the larger restructuring above, these are easy wins:

### Fix list `+` to allocate a new list (C2)

```rust
(Value::List(la), Value::List(lb)) => {
    let mut new_vec = la.borrow().clone();
    new_vec.extend(lb.borrow().iter().cloned());
    stack.push(Value::List(Rc::new(RefCell::new(new_vec))));
}
```

### Fix `build.rs` to find Python (C1)

```rust
let python = ["python3", "python"]
    .iter()
    .find(|name| Command::new(name).arg("--version").output().is_ok())
    .copied()
    .expect("python interpreter required for build");

let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
let project_root = PathBuf::from(&manifest_dir).parent().unwrap().to_path_buf();

let status = Command::new(python)
    .env("PYTHONPATH", &project_root)
    .arg("-m")
    .arg("omglang.compiler")
    .arg(&src)
    .arg(&out_bc)
    .status()
    .expect("failed to run compiler");
```

### Make integer overflow surface a real error (H2)

Replace `unwrap_or(0)` in `handle_mul` with a proper `RuntimeError::ValueError`,
and add `checked_*` calls in `handle_add` / `handle_sub` / `handle_shl` for
consistency.

### Stop silently no-oping on `store_index` / `store_attr` / slice (H6, H7)

Replace the `_ => {}` arms with explicit `Err(RuntimeError::TypeError(...))`.

### Decide on a division semantics and document it (H3)

Either change `Div`/`Mod` in [`ops_arith.rs`](../../runtime/src/vm/ops_arith.rs)
to use `div_euclid` / `rem_euclid` for floor semantics, or change the Python
interpreter to use truncated semantics with `int(a / b)` / `math.fmod`.
Either way, write it down in the spec.

### Decide on equality semantics (H1)

Either remove the `to_string()`-based comparison in `handle_eq`/`handle_ne`
or document and have `omglang.interpreter.eval_expr` do the same. Don't
leave both implementations out of sync.

## Tier 3 — Structural improvements

### Resident in-process REPL (M5, L7)

Refactor `vm::run` to expose a `Vm` struct holding the persistent state
(`globals`, `funcs`, `code`). Drive REPL turns by appending freshly compiled
chunks of bytecode and executing them on the same VM. The `last_output`
diffing hack disappears, latency drops to < 5 ms per turn, and file handles
become consistent with the Python interpreter.

The compiler already supports incremental emission; the only missing
piece is a surface to merge new bytecode into an existing program.

### Have one source of truth for the lexer + parser (M1)

The bootstrap interpreter reimplements both. If you keep the self-hosted
interpreter as a teaching aid, build a small differential test that runs
the same `.omg` programs through it and through the Python parser, and
asserts identical AST output. This will keep the two from drifting.

### Make `parse_bytecode` return `Result` (M3, M4)

Replace `assert!`/`assert_eq!`/`unwrap()` with proper error propagation.
Treat unknown opcodes as a hard error instead of silently skipping.

### Audit panics in the VM (M6)

Search `runtime/src` for `unwrap()` and `expect()`; reserve them for true
invariants. `handle_ret`, `handle_call`, `handle_call_value`, and the file
I/O `lock().unwrap()` calls all currently panic on user-reachable failures.

## Tier 4 — Spec & documentation

### Update `spec/OMG_SPEC.md`

It is missing:

* `try` / `except` (implemented since at least the test
  [`test_try_except.py`](../../omglang/tests/test_try_except.py)).
* `panic`, `raise`, `_omg_vm_*_error_handle` builtins.
* File I/O behaviour (text vs binary mode).
* Equality semantics (string-coerced or typed?).
* Division/modulo semantics on negatives.
* Slicing rules (no negative indexing).

Add a "Cross-implementation parity" appendix that lists known divergences,
or — better — fix them and remove the appendix.

### Update `runtime/README.md`

Document the build prerequisites: a `python` (or `python3`) interpreter on
PATH and the `omglang/` directory importable. Document the `.omg`-vs-`.omgb`
distinction and the performance gap.

### Honest README

Replace "stable and complete" with something accurate, e.g.: "Functional
for educational use; the Python tree-walk interpreter is the reference; the
Rust runtime is fast for precompiled bytecode but currently relies on a
self-hosted interpreter for `.omg` source files."

## Tier 5 — Nice-to-have

* **Bool dict keys / int dict keys**: pick whether they're allowed and
  enforce it in both lexer/parsers and runtime
  ([`ops_struct.rs:52`](../../runtime/src/vm/ops_struct.rs#L52)).
* **Negative-index support** for `list[-1]`, `s[-1:]` — completes the
  "Python-style" claim in the README.
* **Numeric range built-in** (`range(0, 10)`) and `for x in xs { ... }` —
  the embedded interpreter's `loop i < n { ... i := i + 1 }` pattern is
  noisy enough that it would be worth a syntactic affordance.
* **Unicode-correct `chr()`** (L1) and **case-stable `hex()`** (L2).
* **Profile and optimise the self-hosted interpreter** — even if it's not
  the canonical runtime, a 10× speedup on `bootstrap/test_interpret.omg`
  would make it usable as a teaching tool. The two cheapest wins are
  probably (a) using a dict for environments instead of a list-of-pairs,
  and (b) caching `length()` in hot loops.
* **Drop dead opcodes** (L10) or have the compiler emit them.
* **CI matrix** in `.github/workflows/` running both `pytest` and
  `cargo test` on Linux + macOS + Windows. Right now nothing forces the
  build script to keep working off a developer's machine.

## What I would *not* do

* Don't add new language features (classes, modules, traits) until the
  three execution paths agree on the existing ones. Adding surface area
  while drift exists multiplies the divergence.
* Don't replace the bootstrap interpreter wholesale. It's a great showcase
  for what OMG can do, even if it's slow. Keep it; just demote it from
  "the way `.omg` runs" to "an example program".
