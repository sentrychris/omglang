# OMG Runtime

The **OMG Runtime** is the execution engine for the OMG language.

OMG is **genuinely self-hosting**: the OMG compiler is written in OMG itself
([`bootstrap/compiler.omg`](../bootstrap/compiler.omg)), and reproduces its
own bootstrap byte-for-byte (verified with `omg --verify-self-hosted
bootstrap/compiler.omg`). The Rust crate is the substrate: it hosts the VM,
the built-ins, and a stage-0 frontend used to bootstrap the OMG-in-OMG
compiler at `cargo build` time.

There is **no Python dependency** at build time or run time. The legacy
Python toolchain in `omglang/` is retained for reference only.

```text
.omg source ──► lexer ──► parser ──► AST ──► compiler ──► bytecode ──► stack VM
   │                                            │             ▲
   └─ Rust stage-0 (default; fast)              │             │
   └─ stage-1: bootstrap/compiler.omgb ─────────┘             │
                       (run on the VM via --self-hosted) ─────┘
```

See [`.ai/docs/06_actual_self_hosting.md`](../.ai/docs/06_actual_self_hosting.md)
for the full self-hosting story.

## Quick start

```sh
# Build
cargo build --release --manifest-path runtime/Cargo.toml

# Run a script
./runtime/target/release/omg_runtime examples/prime_sieve.omg

# Compile to bytecode
./runtime/target/release/omg_runtime --compile examples/prime_sieve.omg prime.omgb

# Run precompiled bytecode
./runtime/target/release/omg_runtime prime.omgb

# Disassemble a .omg or .omgb file
./runtime/target/release/omg_runtime --disasm examples/hello_world.omg

# Interactive REPL (state persists across turns)
./runtime/target/release/omg_runtime
```

## What the runtime owns

| Module                  | Responsibility                                                     |
| ----------------------- | ------------------------------------------------------------------ |
| `src/lexer.rs`          | Single-pass scanner; strips `;;;omg` header; decodes escapes.      |
| `src/ast.rs`            | Tagged AST types (`Node`, `BinOp`, `UnaryOp`).                     |
| `src/parser.rs`         | Recursive-descent parser with full precedence + structured errors. |
| `src/compiler.rs`       | AST → bytecode. Resolves `import` natively, mangles per-module names, emits short-circuit `and`/`or`, supports first-class functions and closures. |
| `src/bytecode.rs`       | Strict binary `.omgb` parser/writer; returns `Result` (no panics). |
| `src/value.rs`          | Runtime `Value` enum, including `Closure { name, captured }`.      |
| `src/vm.rs`             | Stack VM with one-shot `run()` and resident `run_program()` entry points. |
| `src/vm/ops_arith.rs`   | Arithmetic / comparison / bitwise / boolean handlers (overflow-checked, floor division). |
| `src/vm/ops_struct.rs`  | List / dict / index / slice handlers (bounds-checked; Python-style slice clamping). |
| `src/vm/ops_control.rs` | Calls, returns, jumps, builtins, exceptions; `CallValue` accepts strings *and* closures. |
| `src/vm/builtins.rs`    | Built-ins: `length`, `chr`, `ascii`, `hex`, `binary`, `freeze`, `panic`, `raise`, `read_file`, file I/O. |
| `src/repl.rs`           | In-process REPL with persistent globals + function table.          |

## Native imports

```omg
;;;omg
import "./modules/math.omg" as math
emit math.is_prime(97)
```

The compiler resolves imports recursively at compile time:

1. The imported file is lexed, parsed, and compiled inline at the import
   site.
2. All top-level names from the imported file are mangled with a unique
   per-module prefix (`__mod_N__name`) so different modules never collide.
3. After the imported module's top-level code executes, the compiler emits
   code to build a frozen-namespace dict of its exports (top-level `proc`
   and `alloc` bindings) and assigns it to the alias.
4. Cyclic imports raise `ModuleImportError`.

A single `.omgb` therefore captures an entire multi-file program — there is
no separate runtime module loader.

## First-class functions and closures

Every `proc` is a first-class value. Top-level procs become non-capturing
closures in `globals`. Nested procs capture a snapshot of the enclosing
function's locals at the point of definition.

```omg
;;;omg
proc make_adder(n) {
    proc inner(x) { return x + n }   # captures n
    return inner
}
alloc add5 := make_adder(5)
emit add5(10)        # 15
```

The compiler chooses the right call form automatically:

* Direct call of a known top-level proc → `Call name` (fast).
* Call through a parameter, local, or alloc'd value → `Load name + ... + CallValue argc`.
* Tail-position call of a top-level proc → `TailCall name`.

## Bytecode format

Magic `OMGB` + packed version `(MAJOR<<16)|(MINOR<<8)|PATCH` (currently
`0x000101`). Function table → instruction stream. All multi-byte integers
are little-endian. The format is unchanged from v0.1 except for the
addition of opcode 52 (`MakeFunc`) which binds a `proc` as a first-class
value.

## Built-in functions

| Function     | Description                                |
| ------------ | ------------------------------------------ |
| `chr(n)`     | Single-character string for byte `n`       |
| `ascii(c)`   | Codepoint of single-character string `c`   |
| `hex(n)`     | Hex string of integer `n` (lowercase)      |
| `binary(n[, width])` | Binary string, optionally masked + padded |
| `length(x)`  | Length of list or string                   |
| `freeze(d)`  | Convert a dict to an immutable namespace   |
| `panic(msg)` / `raise(msg)` | Raise a runtime error    |
| `read_file(path)` / `file_exists(path)` | Filesystem queries |
| `file_open / file_read / file_write / file_close` | Streaming I/O |
| `call_builtin(name, args)` | Reflection / dynamic dispatch |

## REPL

```text
$ omg_runtime
OMG Language Interpreter - REPL
Type `exit` or `quit` to leave.
>>> alloc x := 5
>>> proc inc(n) { return n + 1 }
>>> emit inc(x)
6
```

State (`alloc`, `proc`, file handles) persists across turns. Multiline
input is detected by tracking brace depth — inputs aren't executed until
the braces balance.

## What's new in v0.2

* The runtime is fully self-hosted. `build.rs` no longer invokes Python.
* New CLI flags: `--compile`, `--disasm`.
* The embedded `bootstrap/interpreter.omgb` is no longer used to run `.omg`
  files. The bootstrap interpreter remains in `bootstrap/` as a sample OMG
  program.
* `import` now works from compiled `.omgb` files — the compiler resolves
  modules at compile time instead of refusing to compile them.
* First-class functions and closures via the `MakeFunc` opcode and
  `Value::Closure`.
* `and` / `or` short-circuit at the bytecode level.
* List `+` allocates a new list instead of mutating the LHS.
* Integer arithmetic is overflow-checked; division/modulus use floor
  semantics (matching Python).
* `==` / `!=` use typed structural equality (no string-coerced compares).
* Negative indices supported on lists and strings; out-of-range reads/
  writes error instead of silently expanding or returning 0.
* Bytecode parser returns `Result` instead of panicking on malformed input.
* REPL is in-process and persistent — no per-turn process spawn.

## License

Licensed under the [MIT License](../LICENSE).

## Links

* [OMG Language Specification](../spec/OMG_SPEC.md)
* [Lexer Documentation](../spec/OMG_LEXER.md)
* [Parser Documentation](../spec/OMG_PARSER.md)
* [Development Guide](../spec/DEVELOPMENT.md)
