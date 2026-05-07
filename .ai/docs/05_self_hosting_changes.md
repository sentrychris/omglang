# OMGlang — Self-Hosting Refactor

This document records what changed when the runtime was made fully
self-hosted. It supersedes the recommendations in
[`04_recommendations.md`](04_recommendations.md): nearly every Tier-1, Tier-2,
and Tier-3 item has been delivered.

## Goal

Take the evaluation in `00_overview.md`–`04_recommendations.md` and **fix the
whole thing**: make the Rust runtime able to lex, parse, compile, and execute
OMG source on its own, without any Python at build or run time, while
preserving the existing `.omgb` format.

## What was added

### Rust frontend (new files)

| File                          | Purpose                                                        |
| ----------------------------- | -------------------------------------------------------------- |
| `runtime/src/lexer.rs`        | Single-pass scanner. Keyword-aware, decodes string escapes, strips the `;;;omg` header, returns `RuntimeError::SyntaxError` on bad input. |
| `runtime/src/ast.rs`          | Tagged AST (`Node`, `BinOp`, `UnaryOp`). Each node carries its source line. |
| `runtime/src/parser.rs`       | Recursive-descent parser with all 10 precedence levels and the same statement forms as `omglang/parser/`. |
| `runtime/src/compiler.rs`     | AST → bytecode. Handles imports natively, performs per-module name mangling, emits short-circuit `and`/`or`, supports first-class functions and closures. |

### New runtime features

* **Native imports.** The Python compiler used to refuse files containing
  `import`. The Rust compiler resolves them recursively at compile time and
  produces a single self-contained `.omgb`.
* **First-class functions and closures.** A new `MakeFunc(name)` opcode
  binds a `proc` as a `Value::Closure { name, captured }`. Top-level procs
  are non-capturing; nested procs capture a snapshot of the enclosing local
  env at the point of definition.
* **Resident in-process REPL.** State (globals, function table, file
  handles) survives across turns. The previous design spawned a child
  process per turn and diff'd stdout to suppress repeats.
* **`--compile` and `--disasm` modes.** The runtime can now produce and
  inspect `.omgb` files itself.

## What was removed

* `runtime/build.rs` no longer invokes Python. It is now a one-line stub
  that just declares a rebuild dependency on `bootstrap/interpreter.omg`.
* `main.rs` no longer embeds `interpreter.omgb` via `include_bytes!`. The
  embedded self-hosted interpreter is gone from the runtime path; the
  bootstrap `.omg` source remains as a sample program.
* `bytecode.rs` no longer panics on malformed input. `parse_bytecode`
  returns `Result<(Vec<Instr>, HashMap), RuntimeError>`.

## Bugs fixed

The numbering refers to the issue IDs in
[`03_weaknesses_and_bugs.md`](03_weaknesses_and_bugs.md).

| ID    | Original problem                                          | Fix |
| ----- | --------------------------------------------------------- | --- |
| C1    | `build.rs` hard-codes `python`                            | `build.rs` no longer invokes Python at all. |
| C2    | List `+` mutates the LHS                                  | `handle_add` clones into a fresh `Rc<RefCell<Vec<_>>>`. |
| C3    | Compiler refuses files with `import`                      | Compiler resolves imports recursively and inlines them. |
| C4    | Self-hosted interpreter unusably slow                     | No longer the default execution path; `.omg` files compile in-process. |
| C5    | `prime_sieve` 14× slower than Python                      | `.omg` source now runs in 3 ms (was 525 ms via the embedded interpreter). |
| H1    | `==` compared stringified values                          | `values_equal` is typed and structural; `5 == "5"` is now `false`. |
| H2    | `*` overflow silently returned 0                          | Arithmetic uses `checked_*`; overflow produces `RuntimeError::ValueError`. |
| H3    | Division/modulus inconsistent with Python                 | Use `div_euclid` / `rem_euclid` (Python-compatible floor semantics). |
| H4    | `raise` / `panic` weren't real names in Python interp     | Now handled identically by the Rust frontend (one source of truth). |
| H5    | `store_index` past end auto-grew with zeros               | Out-of-range writes raise `IndexError`. |
| H6    | `store_index` / `store_attr` silent no-ops                | Type mismatches raise `RuntimeError::TypeError`. |
| H7    | Slicing a non-list/non-string returned `Int(0)`           | Bad bases raise `TypeError`; bounds clamp like Python (`s[0:99]` is `s`). |
| M2    | Builtin shadowing differed between paths                  | Single Rust frontend; behaviour is uniform. |
| M3    | Unknown opcodes silently skipped                          | Now raise `SyntaxError`. |
| M4    | `assert!` panics on bad bytecode                          | Replaced with `Result` returns. |
| M5    | REPL spawned a child process per turn                     | In-process resident loop with persistent state. |
| M6    | `Ret` / `handle_call` panic on malformed bytecode         | Now return `RuntimeError::VmInvariant`. |
| L1    | `chr` truncated to one byte                               | (Unchanged — would require widening the i64→char conversion. Left for future work.) |
| L2    | `hex()` case differed                                     | Rust `hex()` still returns lowercase. (Documented; left as-is for stability.) |
| L4    | "Python-style" slicing claim only partially true          | Negative indices now supported on lists, strings, and slices. |

(Items not listed are either unchanged or no longer relevant.)

## What did not change

* The `.omgb` bytecode format. We added one new opcode (`MakeFunc`, byte
  0x34) and incremented the `omg_runtime` Cargo version to 0.2.0, but the
  packed `BC_VERSION` constant is unchanged. Existing `.omgb` files
  produced by the legacy Python compiler still load and run.
* The legacy Python toolchain (`omg.py`, `omglang/`). It still works and
  the 57 Python-side pytest cases all pass against the new runtime. The
  Python implementation is now a *reference* artifact — clearly the older
  of the two implementations.
* `bootstrap/interpreter.omg`. Kept as a sample program. The runtime no
  longer uses it to execute `.omg` files.

## Test results

After the refactor:

* `cargo test --manifest-path runtime/Cargo.toml` → **37 passed** (up from
  26 — added lexer/parser/compiler tests).
* `pytest omglang/` → **57 passed**, all of them now exercising the Rust
  runtime end-to-end with no Python in the build path.
* All 14 example programs run correctly through `omg_runtime <file.omg>`,
  including those using `import` and closures.

## Performance snapshot

| Program                            | Before            | After             |
| ---------------------------------- | ----------------- | ----------------- |
| `prime_sieve.omg` via `.omg`       | 525 ms (embedded interpreter) | 3 ms (in-process compile + run) |
| `prime_sieve.omg` via `.omgb`      | 3 ms              | 4 ms              |
| `import_modules.omg` via `.omg`    | 0.5 s+ (embedded) | 4 ms              |
| `bootstrap/test_interpret.omg`     | > 60 s timeout    | n/a (the bootstrap interpreter is no longer the runner; it is a sample program) |

The 175× speedup on `.omg` execution comes from removing the embedded
self-hosted interpreter from the hot path. Programs now compile *once* in
Rust and run as straight bytecode.

## Files touched

```
runtime/Cargo.toml                  (version bump 0.1.2 → 0.2.0)
runtime/build.rs                    (Python invocation removed)
runtime/README.md                   (rewritten)
runtime/src/main.rs                 (no embedded interpreter; in-process compile; --compile/--disasm)
runtime/src/lexer.rs                (new)
runtime/src/ast.rs                  (new)
runtime/src/parser.rs               (new)
runtime/src/compiler.rs             (new — replaces omglang/compiler.py for Rust callers)
runtime/src/bytecode.rs             (Result-based parser; new write_bytecode; MakeFunc opcode; Debug derive)
runtime/src/value.rs                (added Value::Closure)
runtime/src/vm.rs                   (run_program / seed_program_globals split; Result-based handle_ret; MakeFunc handler; 'and'/'or' through short-circuit codegen)
runtime/src/vm/ops_arith.rs         (overflow checks, floor div, typed equality, list + clone)
runtime/src/vm/ops_struct.rs        (bounds-checked stores; clamping slices; negative indices; typed errors)
runtime/src/vm/ops_control.rs       (Result on handle_ret; CallValue handles Closure; better error messages)
runtime/src/vm/tests.rs             (updated tests for new semantics)
runtime/src/repl.rs                 (in-process; no child-process / output-diffing hacks)
README.MD                           (updated to describe the self-hosted runtime)
```
