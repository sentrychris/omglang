# OMGlang — Strengths

Things the project does well, in roughly descending order of impressiveness.

## 1. Scope of the language stack

For an "educational" project this covers a remarkable amount of ground:

* Hand-written regex-based lexer.
* Recursive-descent parser with full operator precedence (10 levels).
* Tree-walk interpreter, including closures and module imports.
* AST-to-bytecode compiler.
* Stack-based VM.
* Custom binary bytecode format with magic header and version.
* Self-hosted interpreter written in OMG, embedded into the runtime.
* REPL.
* Built-in test infrastructure (pytest + cargo test).

Most toy languages stop at one or two of these. Doing all of them — even with
rough edges — demonstrates strong engagement with the full lifecycle of
language implementation.

## 2. The bytecode VM is well structured

[`runtime/src/vm.rs`](../../runtime/src/vm.rs) is small, readable, and split
along the right axes:

* `vm/ops_arith.rs` — arithmetic / comparison / bitwise / boolean.
* `vm/ops_control.rs` — calls, returns, jumps, exceptions, I/O.
* `vm/ops_struct.rs` — list / dict / index / slice / attribute.
* `vm/builtins.rs` — first-party built-in functions.

The fetch-decode-execute loop uses an `advance_pc` flag that control-flow
ops can clear, which is a clean way to implement jumps and calls without
duplicate `pc += 1` sites. Exception unwinding via the `Block` stack
(handler PC + saved stack/env/ret depths) is a textbook-correct
implementation of structured exception handling.

The VM tests in [`runtime/src/vm/tests.rs`](../../runtime/src/vm/tests.rs) are
well-targeted error-path tests (raise, frozen-dict writes, out-of-bounds
slices, etc.) — the kind of tests that catch real regressions.

## 3. Clear separation of compile-time and runtime concerns

The bytecode format is properly defined ([`bytecode.rs`](../../runtime/src/bytecode.rs)),
versioned, and decoded once at startup. The Python compiler builds a
function table during a deferred pass so that forward references inside
function bodies resolve correctly
([`compiler.py:188-204`](../../omglang/compiler.py#L188)). This is the right
shape for a real compiler.

`Op` is centralised in [`omglang/operations.py`](../../omglang/operations.py)
as a `str`-valued enum, which keeps the parser and interpreter in sync via a
single source of truth — better than scattering string constants.

## 4. The Python reference implementation is idiomatic and clear

[`omglang/interpreter.py`](../../omglang/interpreter.py) reads like a textbook
tree-walk interpreter. Highlights:

* `BreakLoop` / `ReturnControlFlow` as exception-based control flow keeps the
  AST evaluator small and avoids ad-hoc state flags.
* `FunctionValue` captures the closure environment by `dict.copy()` at
  definition time and stores a separate reference to the global environment,
  so that recursive calls to module-level functions resolve correctly even
  when the call site is inside a different scope. (See
  [`interpreter.py:813-819`](../../omglang/interpreter.py#L813).)
* `FrozenNamespace` is a clever subclass of `dict` that overrides every
  mutation method to raise `TypeError` — gives module imports immutability
  with minimal code.

## 5. Documentation discipline

The `spec/` directory documents the lexer, parser, language, errors, and
development workflow. Module docstrings in `omglang/*.py` and `runtime/src/*`
are detailed and accurate. The Rust source in particular has thorough
top-of-file rustdoc explaining the design intent, not just the mechanics.

## 6. Self-hosting attempt

[`bootstrap/interpreter.omg`](../../bootstrap/interpreter.omg) (1229 lines) is
a non-trivial program: it implements its own tokenizer, parser, and
tree-walk evaluator entirely in OMG. The fact that it runs at all — even
slowly — and can execute multi-file imports against the Rust VM is a
meaningful milestone.

This is an ideal vehicle for stress-testing the language: it exercises
recursion, closures, lists-as-AST-tuples, dictionaries-as-environments,
string manipulation, and file I/O all at once.

## 7. Tests and CI hooks

* 57 Python tests plus 26 Rust tests.
* Several Python tests round-trip programs through the compiler and Rust VM
  via `cargo run`, so the bytecode encoding/decoding path is exercised
  end-to-end.
* `.github/`, `.flake8`, `.pylintrc`, `setup.cfg`, and `pyproject.toml`
  indicate intent to run the project with hygiene tooling.

## 8. Small, deliberate language design

OMG is intentionally minimal:

* One assignment operator (`:=`) with explicit `alloc` for declaration,
  which sidesteps Python's "is this an assignment or a binding?" ambiguity.
* No `null` / `nil`; falsy is exhaustively defined (`false`, `""`, `[]`,
  unset).
* `emit` for output, `facts` for assertion — distinct keywords avoid the
  Python/JS habit of using the same symbol for too many things.
* Logical `and`/`or` with short-circuit, returning **boolean** rather than
  the operand value (different from Python; arguably cleaner).

The minimalism makes the implementation tractable. A larger surface area
would expose far more cross-implementation drift than already exists.
