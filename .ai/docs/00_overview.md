# OMGlang Evaluation — Overview

This directory contains a senior-engineer review of the **omglang** project as
of commit `f311adf` (branch `main`, May 2026). The evaluation covers the whole
repository: the original Python lexer/parser/interpreter, the Python bytecode
compiler, the Rust virtual machine that executes that bytecode, and the
self-hosted OMG interpreter that the runtime embeds at build time.

## Documents

1. [`00_overview.md`](00_overview.md) — this file. High-level summary and
   verdict.
2. [`01_architecture.md`](01_architecture.md) — components and data flow.
3. [`02_strengths.md`](02_strengths.md) — what works well and is worth keeping.
4. [`03_weaknesses_and_bugs.md`](03_weaknesses_and_bugs.md) — concrete issues,
   broken behaviour, and divergences between implementations.
5. [`04_recommendations.md`](04_recommendations.md) — prioritised suggestions.

## Headline verdict

OMGlang is a successful **educational** project. As a learning vehicle for
language implementation it is clear, well structured, and covers more ground
than most toy languages: a hand-written lexer, recursive-descent parser,
tree-walk interpreter, stack-based bytecode VM, a compiler that lowers AST to
that bytecode, and a self-hosted interpreter written in OMG itself.

It is **not** stable as a production language and several user-visible
behaviours diverge between the three concrete implementations
(`omglang.interpreter`, `omg_runtime` running `.omgb`, and `omg_runtime`
running `.omg` via the embedded interpreter). The `README` calls the
implementation "stable and complete as a toy language", which over-states
things — important paths (notably running `.omg` source via the Rust runtime
on a non-trivial program, and compiling files containing `import` to bytecode)
either fail or are dramatically slow.

| Dimension                       | Verdict                                                 |
| ------------------------------- | ------------------------------------------------------- |
| Code quality / readability      | Good. Modules are short, well documented, and idiomatic.|
| Test coverage                   | Reasonable. 57 Python tests + 26 Rust tests pass.       |
| Spec ↔ implementation alignment | Drifting. Several documented behaviours not enforced.   |
| Cross-implementation parity     | Poor. Python and Rust paths diverge in user-visible ways.|
| Performance of the "native" path | Bytecode mode is fast; `.omg` source mode is ~14× slower than the Python interpreter on `prime_sieve`, and the self-hosted interpreter is unusable on non-trivial programs (60 s timeout for `fact(5)`). |
| Build hygiene                   | Brittle. `build.rs` hard-codes `python` (not `python3`) and depends on the `omglang` package being importable at build time. |

## What runs and what doesn't (verified empirically)

Tested against `runtime/target/debug/omg_runtime` and `python3 omg.py`.

| Example                    | Python interpreter | Rust runtime (.omg source) | Notes                                                                 |
| -------------------------- | ------------------ | -------------------------- | --------------------------------------------------------------------- |
| `hello_world.omg`          | ✅                 | ✅                         | —                                                                     |
| `assignment.omg`           | ✅                 | ✅                         | —                                                                     |
| `higher_order.omg`         | ✅                 | ✅                         | Closures work in both.                                                |
| `bitwise.omg`              | ✅                 | ✅                         | —                                                                     |
| `dictionaries.omg`         | ✅                 | ✅                         | —                                                                     |
| `merge_sort.omg`           | ✅                 | ✅                         | —                                                                     |
| `prime_sieve.omg`          | ✅ (36 ms)         | ✅ (525 ms)                | 14× slower in Rust because the `.omg` path uses the embedded interpreter, not the Python compiler. |
| `matrix_ops.omg`           | ✅                 | ✅                         | —                                                                     |
| `rot_13.omg`               | ✅                 | ✅                         | —                                                                     |
| `hex_to_rgb.omg`           | ✅                 | ✅                         | —                                                                     |
| `permissions.omg`          | ✅                 | ✅                         | —                                                                     |
| `import_modules.omg`       | ✅                 | ✅                         | Both end with the *intentional* trailing `facts` failure.             |
| `tabula_recta.omg`         | ✅                 | (slow, untested fully)     | Likely works, slow via embedded interpreter.                          |
| `maze_solver.omg`          | ✅                 | (slow, untested fully)     | Likely works, slow via embedded interpreter.                          |
| `self_hosted.omg`          | ❌ `Undefined variable 'raise'` | ⏱️ 60 s timeout | The Python interpreter exposes neither `raise` nor `panic` as builtins; the self-hosted bootstrap hits a 60 s timeout for trivial code. |
| `bootstrap/test_interpret.omg` (`fact(5)` via OMG-in-OMG interp) | n/a | ⏱️ 60 s timeout | The self-hosted interpreter is too slow to be practical. |
| `file_ops.omg` (Python)    | ❌ "no such file"  | ✅ via runtime              | Path-resolution differs: the Python interpreter doesn't auto-create the `files/` dir, the Rust path resolves relative to `current_dir` global. |

### Cargo + pytest test suites

* `cargo test --manifest-path runtime/Cargo.toml` → **26 passed** (after
  ensuring `python` resolves to `python3`).
* `pytest omglang/` → **57 passed** (with `python` on `PATH`). The native VM
  tests (`test_native_*`) all need `cargo` and a `python` binary; without
  those, they fail with `CalledProcessError`.

## Key issues at a glance

The full breakdown is in [`03_weaknesses_and_bugs.md`](03_weaknesses_and_bugs.md).
The critical ones:

1. **`build.rs` hard-codes `python`**, not `python3`. On a default Ubuntu/WSL
   install (which provides only `python3`), the runtime fails to build. The
   build also requires `PYTHONPATH` to point at the `omglang/` source root,
   which is undocumented.
2. **`a + b` mutates `a` in place** when both are lists. The Rust runtime
   extends the left operand and returns it as the result. The Python
   interpreter creates a new list. This is a real bug in
   [`runtime/src/vm/ops_arith.rs:60`](../../runtime/src/vm/ops_arith.rs#L60).
3. **Integer overflow in `*` silently returns 0** (intentional `unwrap_or(0)`
   in `handle_mul`). Programs receive nonsense values instead of an error.
4. **`==` compares stringified values**, so `"5" == 5` is `true` in the Rust
   runtime. The Python interpreter delegates to Python's `==`, so `"5" == 5`
   is `false`. The two implementations disagree.
5. **Integer division semantics differ**. Python uses floor division
   (`-7 // 2 == -4`, `-7 % 2 == 1`); Rust uses truncated division
   (`-7 / 2 == -3`, `-7 % 2 == -1`). Same source program, different output.
6. **`raise` / `panic` aren't real names in the Python interpreter**. They
   only exist in the bytecode compiler as magic identifiers that lower to
   `RAISE`. Any OMG program that calls them works under `omg_runtime` but
   crashes with `UndefinedVariable` under `python3 omg.py`.
7. **The Python compiler refuses to compile any file with `import`**
   ([`compiler.py:288`](../../omglang/compiler.py)). This means there is no
   way to produce a portable `.omgb` for a multi-file program; the runtime
   *must* re-parse the source via the embedded self-hosted interpreter, which
   is the slow path.
8. **The self-hosted interpreter is unusably slow.** `fact(5)` via
   `bootstrap/test_interpret.omg` does not finish in 60 seconds.
9. **`store_index` on a list silently grows the list** with `Int(0)` fillers,
   while the Python interpreter raises `RuntimeError`. Neither writes to a
   dict key with a `Bool` index — both silently no-op
   ([`ops_struct.rs:218`](../../runtime/src/vm/ops_struct.rs#L218)).
10. **Slicing a non-string/non-list value silently returns `Int(0)`** in the
    Rust runtime ([`ops_struct.rs:187`](../../runtime/src/vm/ops_struct.rs#L187)).

The point is not that these are catastrophic — most are easy to fix — but
that they accumulate and undermine the project's claim of being "stable and
complete".
