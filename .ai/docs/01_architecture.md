# OMGlang — Architecture

This document maps the moving parts and the data flow between them.

## Components

```
                               ┌──────────────────────┐
                               │ Source: *.omg        │
                               │  ;;;omg              │
                               │  ...code...          │
                               └─────────┬────────────┘
                                         │
                            ┌────────────┴────────────┐
                            │                         │
              (Python path) │                         │ (Rust path)
                            ▼                         ▼
                    omglang/lexer.py          runtime/src/main.rs
                    omglang/parser/*          (chooses .omg vs .omgb)
                            │                         │
                            ▼                         │
                    omglang/parser/Parser → AST       │
                            │                         │
       ┌────────────────────┴───────────────┐         │
       │                                    │         │
       ▼                                    ▼         │
omglang/interpreter.py                omglang/compiler.py
(tree-walk; reference impl)           (AST → bytecode .omgb)
       │                                    │         │
       │                                    ▼         │
       │                           runtime/src/bytecode.rs
       │                           (decode .omgb to Vec<Instr>)
       │                                    │         │
       │                                    ▼         │
       │                           runtime/src/vm.rs  │
       │                           (stack-based VM)   │
       │                                    │         │
       │                                    │         │  .omg only
       │                                    │         ▼
       │                                    │  Embedded interpreter.omgb
       │                                    │  (bootstrap/interpreter.omg
       │                                    │  compiled at cargo build time)
       │                                    │         │
       │                                    │         │ runs in same VM
       │                                    │         ▼
       │                                    │  reads .omg, lexes/parses/
       │                                    │  evaluates in OMG itself
       │                                    │         │
       └────────────────────────────────────┴─────────┘
                              │
                              ▼
                          stdout
```

## Three execution paths

OMGlang has **three** distinct ways to run a program. They share parts but
diverge in user-visible behaviour:

### Path A — Python tree-walk (`python3 omg.py script.omg`)

[`omglang/lexer.py`](../../omglang/lexer.py) →
[`omglang/parser/`](../../omglang/parser/) →
[`omglang/interpreter.py`](../../omglang/interpreter.py).

* Reference / canonical implementation.
* Implements `import` natively.
* Has no `raise` / `panic` builtins (they exist only as compiler magic).
* Uses Python's arithmetic (arbitrary-precision, floor division).

### Path B — Precompiled bytecode (`omg_runtime script.omgb`)

[`omglang/compiler.py`](../../omglang/compiler.py) (offline) →
[`runtime/src/bytecode.rs`](../../runtime/src/bytecode.rs) →
[`runtime/src/vm.rs`](../../runtime/src/vm.rs).

* Fast (3 ms for `prime_sieve` vs 36 ms in path A).
* The compiler **refuses** to compile programs containing `import`
  ([`compiler.py:288`](../../omglang/compiler.py)).
* Uses Rust `i64` arithmetic (overflow → `0` in `*`, truncated division).

### Path C — Source via embedded self-hosted interpreter (`omg_runtime script.omg`)

[`runtime/src/main.rs`](../../runtime/src/main.rs) loads
`bootstrap/interpreter.omgb` (the OMG-implemented interpreter, compiled at
build time) into the VM, hands it the path of the user's `.omg` file, and the
embedded interpreter then re-implements lex/parse/eval in OMG itself.

* Required whenever a `.omg` file uses `import` (because path B cannot
  produce a `.omgb` for it).
* Effectively double interpretation: every operation in user code runs as a
  walk over OMG-built AST nodes, which themselves execute as bytecode in the
  Rust VM.
* Substantially slower than even Path A on non-trivial programs;
  `fact(5)` via `bootstrap/test_interpret.omg` does not finish within 60
  seconds.
* Uses **its own** lexer/parser, [`bootstrap/interpreter.omg`](../../bootstrap/interpreter.omg).
  It does *not* share the Python lexer/parser, so any feature added to
  `omglang/lexer.py` must be hand-mirrored into `interpreter.omg` or it will
  silently desync.

## Bytecode format

Magic `OMGB` + packed version `(MAJOR<<16)|(MINOR<<8)|PATCH` (currently
`0x000101`, i.e. v0.1.1).

* Function table: name (length-prefixed UTF-8), param count, params, address.
* Code stream: `u8` opcode + opcode-specific operands.
* Opcodes 0–46 emitted by the Python compiler
  ([`compiler.py:OPCODES`](../../omglang/compiler.py)).
* Rust parser additionally accepts opcodes 47–51 as short-form `Raise(Kind)`
  variants ([`bytecode.rs:350-354`](../../runtime/src/bytecode.rs#L350)) —
  these are dead code; nothing emits them.
* Unknown opcodes are silently skipped
  ([`bytecode.rs:356`](../../runtime/src/bytecode.rs#L356)). This is permissive
  forward-compat behaviour but masks corruption.

## Runtime data model

[`runtime/src/value.rs`](../../runtime/src/value.rs) defines the universal
`Value` type:

* `Int(i64)` — 64-bit signed
* `Str(String)`
* `Bool(bool)`
* `List(Rc<RefCell<Vec<Value>>>)` — interior-mutable, shared
* `Dict(Rc<RefCell<HashMap<String, Value>>>)` — keys are always strings
* `FrozenDict(Rc<HashMap<String, Value>>)` — used for imported namespaces
* `None`

Note that **dictionary keys are always stringified**: `BuildDict` calls
`pop().to_string()` on each key
([`ops_struct.rs:52`](../../runtime/src/vm/ops_struct.rs#L52)). This means
`{1: "a"}` from the embedded interpreter ends up keyed on `"1"`. The Python
parser doesn't even allow numeric keys in literals — it errors with
`SyntaxError: Invalid dict key`.

## Build pipeline

[`runtime/build.rs`](../../runtime/build.rs):

1. Spawns `python -m omglang.compiler bootstrap/interpreter.omg
   $OUT_DIR/interpreter.omgb`.
2. `main.rs` embeds that `.omgb` via `include_bytes!`.

Failure modes:

* The system has only `python3`, not `python` — build panics with `Os { code:
  2, kind: NotFound }`.
* `omglang` package isn't on `PYTHONPATH` (the script doesn't `cd` to the
  parent of the `runtime/` dir before invoking the Python module) — build
  panics with `ModuleNotFoundError`.

In practice, builds work only when invoked through the helper script
[`scripts/cli.py`](../../scripts/cli.py) which sets up paths, *or* when the
user happens to have `python` aliased to `python3` and the project root on
`PYTHONPATH`.

## Module / import semantics

`import "foo.omg" as foo` exposes `foo`'s top-level `proc` and `alloc`
bindings under a frozen namespace.

* In Path A, `Interpreter.import_module` parses and executes the imported
  file recursively, then returns a `FrozenNamespace`
  ([`interpreter.py:148`](../../omglang/interpreter.py#L148)).
* In Path B, the Python compiler refuses outright.
* In Path C, the embedded interpreter implements its own
  `import_module` over heap data structures; it shares no code with Path A.

This three-way split is the root cause of most of the divergences listed in
[`03_weaknesses_and_bugs.md`](03_weaknesses_and_bugs.md).
