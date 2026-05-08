# OMG Runtime

The **OMG Runtime** is the execution engine for the OMG language.

OMG is **genuinely self-hosting**: the OMG compiler is written in OMG itself
([`bootstrap/compiler.omg`](../bootstrap/compiler.omg)), and reproduces its
own bootstrap byte-for-byte (verified with `omg --verify-self-hosted
bootstrap/compiler.omg`). The Rust crate is the substrate: it hosts the VM,
the built-ins, and a stage-0 frontend used to bootstrap the OMG-in-OMG
compiler at `cargo build` time.

There is **no Python dependency** at build time or run time. The legacy
Python toolchain in `reference/` is retained for reference only.

```text
.omg source ──► lexer ──► parser ──► AST ──► compiler ──► bytecode ──► stack VM
   │                                            │             ▲
   └─ Rust stage-0 (default; fast)              │             │
   └─ stage-1: bootstrap/compiler.omgb ─────────┘             │
                       (run on the VM via --self-hosted) ─────┘
```

## Quick start

```sh
# Build
cargo build --release --manifest-path runtime/Cargo.toml

# Run a script
./runtime/target/release/omg examples/prime_sieve.omg

# Compile to bytecode (Rust frontend)
./runtime/target/release/omg --compile examples/prime_sieve.omg prime.omgb

# Run precompiled bytecode
./runtime/target/release/omg prime.omgb

# Disassemble a .omg or .omgb file
./runtime/target/release/omg --disasm examples/hello_world.omg

# Compile and run via the OMG-in-OMG compiler (slower; proves self-hosting)
./runtime/target/release/omg --self-hosted examples/prime_sieve.omg

# Compile to bytecode using the OMG-in-OMG compiler
./runtime/target/release/omg --self-hosted-compile examples/prime_sieve.omg prime.omgb

# Fixed-point check: Rust and OMG-in-OMG compilers produce identical bytes
./runtime/target/release/omg --verify-self-hosted bootstrap/compiler.omg

# Interactive REPL (state persists across turns)
./runtime/target/release/omg
```

Run `omg --help` for the full CLI reference.

## What the runtime owns

| Module                  | Responsibility                                                     |
| ----------------------- | ------------------------------------------------------------------ |
| `src/lexer.rs`          | Single-pass scanner; strips `;;;omg` header; decodes escapes.      |
| `src/ast.rs`            | Tagged AST types (`Node`, `BinOp`, `UnaryOp`).                     |
| `src/parser.rs`         | Recursive-descent parser with full precedence + structured errors. |
| `src/compiler.rs`       | AST → bytecode. Resolves and caches `import`s natively, mangles per-module names, emits short-circuit `and`/`or`, supports first-class functions and closures. Also exposes `compile_source_with_globals` for the REPL. |
| `src/bytecode.rs`       | Strict binary `.omgb` parser/writer; returns `Result` (no panics). |
| `src/value.rs`          | Runtime `Value` enum, including `Closure { name, captured }`.      |
| `src/vm.rs`             | Stack VM with one-shot `run()` and resident `run_program()` entry points. |
| `src/vm/ops_arith.rs`   | Arithmetic / comparison / bitwise / boolean handlers (overflow-checked, floor division). |
| `src/vm/ops_struct.rs`  | List / dict / index / slice handlers (bounds-checked; Python-style slice clamping). |
| `src/vm/ops_control.rs` | Calls, returns, jumps, builtins, exceptions; `CallValue` accepts strings *and* closures. |
| `src/vm/builtins.rs`    | Built-ins: `length`, `chr`, `ascii`, `hex`, `binary`, `string_bytes`, `freeze`, `panic`, `raise`, `read_file`, `file_exists`, file I/O, `call_builtin`. |
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
4. Already-compiled modules are cached by canonical path, so importing the
   same module from two places runs its top-level code only once. Both the
   Rust compiler and `bootstrap/compiler.omg` apply the same caching, which
   is required for the byte-identical fixed-point check.
5. Cyclic imports raise `ModuleImportError`.

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
are little-endian. Two opcodes were added beyond the v0.1 baseline:

- **52 (`MakeFunc`)** binds a `proc` as a first-class value: at top level
  it stores `Closure { name, captured: ∅ }` into globals; inside a
  function it captures the current local environment.
- **53 (`StoreLocal`)** is the `alloc` form. It always creates a binding
  in the *innermost* scope (locals inside a function, globals at top
  level). It exists so that `alloc args := ...` inside a function can't
  clobber the runtime-injected `args` global.

Functions are emitted in **sorted name order** so the writer is
deterministic — the self-hosted fixed-point check depends on it.

## Built-in functions

| Function     | Description                                |
| ------------ | ------------------------------------------ |
| `chr(n)`     | Single-character string for byte `n`       |
| `ascii(c)`   | Codepoint of single-character string `c`   |
| `hex(n)`     | Hex string of integer `n` (lowercase)      |
| `binary(n[, width])` | Binary string, optionally masked + padded |
| `length(x)`  | Length of list or string                   |
| `string_bytes(s)` | UTF-8 byte values of `s` as a list of ints |
| `freeze(d)`  | Convert a dict to an immutable namespace   |
| `panic(msg)` / `raise(msg)` | Raise a runtime error    |
| `read_file(path)` / `file_exists(path)` | Filesystem queries |
| `file_open / file_read / file_write / file_close` | Streaming I/O |
| `call_builtin(name, args)` | Reflection / dynamic dispatch |

The runtime also injects three special globals into every program:
`args` (command-line arguments, `args[0]` = script path), `module_file`
(absolute path of the running file), and `current_dir` (the user's shell
working directory — what `read_file` and `file_open` resolve relative
paths against).

## REPL

```text
$ omg
OMG Language Interpreter - REPL
Type `exit` or `quit` to leave.
>>> alloc x := 5
>>> proc inc(n) { return n + 1 }
>>> emit inc(x)
6
```

State (`alloc`, `proc`, imports, file handles) persists across turns.
Multiline input is detected by tracking `{ }`, `( )`, and `[ ]` depth —
input isn't dispatched until all three balance. Each turn is compiled
afresh and **stitched** onto the persistent code stream: jumps and
function addresses are rebased so closures defined in earlier turns
remain callable later.

## Links

* [Top-level README](../README.MD)
* [OMG-in-OMG tools](../tools/README.md)
* [VS Code extension](../vscode/README.md)
