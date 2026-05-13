# OMG Runtime

The **OMG Runtime** is the execution engine for the OMG language.

OMG is **genuinely self-hosting**: the OMG compiler is written in OMG itself
([`bootstrap/src/compiler.omg`](../bootstrap/src/compiler.omg)), and reproduces its
own bootstrap byte-for-byte (verified with `omg --verify-self-hosted
bootstrap/src/compiler.omg`). The Rust crate is the substrate: it hosts the VM,
the built-ins, and a stage-0 frontend used to bootstrap the OMG-in-OMG
compiler at `cargo build` time.

There is **no Python dependency** at build time or run time. The legacy
Python toolchain in `reference/` is retained for reference only.

```text
.omg source ──► lexer ──► parser ──► AST ──► compiler ──► bytecode ──► stack VM
   │                                            │             ▲
   └─ stage-1: bootstrap/src/compiler.omgb (default)│             │
   └─ stage-0: Rust frontend (faster; via --rust)─────────────┘
                                                              │
                                          (alternative: transpile to C)
                                          bytecode ──► native-c.omg ──► .c ──► ELF
```

By default `omg <script>` compiles via the embedded OMG-in-OMG compiler
running on the VM — the language compiles itself end to end. Pass
`--rust` to use the Rust frontend instead, which is significantly faster
but bypasses the self-hosted toolchain.

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

# Run a script via the Rust frontend (faster; bypasses self-hosting)
./runtime/target/release/omg --rust examples/prime_sieve.omg

# Compile to bytecode using the OMG-in-OMG compiler
./runtime/target/release/omg --self-hosted-compile examples/prime_sieve.omg prime.omgb

# Fixed-point check: Rust and OMG-in-OMG compilers produce identical bytes
./runtime/target/release/omg --verify-self-hosted bootstrap/src/compiler.omg

# Triple-meta fixed-point check: Rust frontend vs (OMG compiler running
# on the OMG VM). Proves both stage-1 components behave like their Rust
# counterparts on the input.
./runtime/target/release/omg --verify-omg-vm examples/prime_sieve.omg

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
| `src/vm/builtins.rs`    | Built-ins: strings (`length`, `chr`, `ascii`, `string_bytes`, `bytes_to_string`), formatting (`hex`, `binary`, `float_bits`, `bits_to_float`), numeric/math (`int`, `float`, `floor`, `ceil`, `round`, `abs`, `sqrt`, `pow`, `log`, `sin`, `cos`, `tan`), collections (`freeze`, `dict_keys`), errors (`panic`, `raise`, `exit_with_error`), file I/O (`read_file`, `file_exists`, `is_dir`, `read_dir`, `make_dir`, `file_open`/`read`/`write`/`close`), process control (`subprocess`, `exit`, `getpid`), reflection (`call_builtin`). |
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
   Rust compiler and `bootstrap/src/compiler.omg` apply the same caching, which
   is required for the byte-identical fixed-point check.
5. Cyclic imports raise `ModuleImportError`.

A single `.omgb` therefore captures an entire multi-file program — there is
no separate runtime module loader.

## First-class functions and closures

Every `proc` is a first-class value. Top-level procs become non-capturing
closures in `globals`. Nested procs capture the enclosing function's
locals **by reference** — each binding is an `Rc<RefCell<Value>>` cell
shared between the enclosing frame and every closure built over it, so
mutations on either side are visible to the other (Python/JS-style).

```omg
;;;omg
proc make_adder(n) {
    proc inner(x) { return x + n }   # captures n by reference
    return inner
}
alloc add5 := make_adder(5)
emit add5(10)        # 15

# Mutation through the captured cell is visible to the closure:
proc make_counter() {
    alloc n := 0
    proc tick() {
        n := n + 1
        return n
    }
    return tick
}
alloc c := make_counter()
emit c()        # 1
emit c()        # 2
emit c()        # 3
```

The compiler chooses the right call form automatically:

* Direct call of a known top-level proc → `Call name` (fast).
* Call through a parameter, local, or alloc'd value → `Load name + ... + CallValue argc`.
* Tail-position call of a top-level proc → `TailCall name`.

## Bytecode format

Magic `OMGB` + packed version `(MAJOR<<16)|(MINOR<<8)|PATCH` (currently
`0x000200`). All multi-byte integers are little-endian. Sections appear
in this order:

```
+------------------+------------------------------+
| Magic            | "OMGB" (4 bytes)             |
+------------------+------------------------------+
| Version          | u32 = 0x000200               |
+------------------+------------------------------+
| Source-file cnt  | u32                          |
| For each file:   |   u32 len + UTF-8 path       |
+------------------+------------------------------+
| Func count       | u32                          |
| For each func:   |                              |
|   Name           |   u32 len + UTF-8 bytes      |
|   Param count    |   u32                        |
|   Params[...]    |   (Param count times)        |
|                  |     u32 len + UTF-8 bytes    |
|   Address        |   u32                        |
|   Source file ix |   u32                        |
+------------------+------------------------------+
| Code length      | u32                          |
| For each instr:  |   opcode u8 + operands       |
+------------------+------------------------------+
| Src-map length   | u32  (= code length)         |
| For each entry:  |                              |
|   Source file ix |   u32                        |
|   Line number    |   u32  (1-based; 0 = synth)  |
+------------------+------------------------------+
```

The source-file table + per-instruction source map are what give
runtime errors their `File "foo.omg", line 12, in <fn>` traceback
context. Both the Rust frontend and `bootstrap/src/compiler.omg`
absolute-normalise paths before storing them so the resulting `.omgb`
is byte-identical across implementations regardless of CWD.

Bytecode `0x0001xx` (no source map) is rejected with a clear
"recompile your .omgb" SyntaxError — there's no fallback path.

Four opcodes beyond the v0.1 baseline:

- **52 (`MakeFunc`)** binds a `proc` as a first-class value: at top level
  it stores `Closure { name, captured: ∅ }` into globals; inside a
  function it captures the current local environment **by reference**
  (the captured map shares `Rc<RefCell<Value>>` cells with the enclosing
  scope, Python/JS-style).
- **53 (`StoreLocal`)** is the `alloc` form. It always creates a binding
  in the *innermost* scope (locals inside a function, globals at top
  level). It exists so that `alloc args := ...` inside a function can't
  clobber the runtime-injected `args` global. Each `StoreLocal` installs
  a **fresh cell**, so a closure captured from one loop iteration doesn't
  see writes from the next iteration's fresh binding.
- **54 (`PushFloat`)** pushes an IEEE-754 f64 literal onto the stack
  (8-byte little-endian payload, same layout as `PushInt`).
- **55 (`FloorDiv`)** implements the `//` operator. `/` between two ints
  is still floor division (back-compat), but as soon as either operand
  is a float `/` becomes true division, so `//` exists for cases where
  the source needs to *force* the floor.

Functions are emitted in **sorted name order** so the writer is
deterministic — the self-hosted fixed-point check depends on it.

## Errors and tracebacks

Uncaught runtime errors print a Python-style traceback to stderr:

```
$ omg /tmp/bad.omg
Traceback (most recent call last):
  File "/tmp/bad.omg", line 17, in <top-level>
  File "/tmp/bad.omg", line 13, in outer
  File "/tmp/bad.omg", line 8, in middle
  File "/tmp/bad.omg", line 4, in inner
IndexError: index 5 out of range for length 0
```

The traceback is assembled from two things tracked through `execute`
in [`src/vm.rs`](src/vm.rs):

- **A call-frame stack** parallel to `env_stack`/`ret_stack`. `Call` /
  `CallValue` push a `CallFrame { name, call_pc }`; `Ret` pops; `TCall`
  rewrites the top frame's name (so tail-call elimination doesn't lose
  the caller's identity).
- **The source map** loaded from the `.omgb` (see Bytecode format above).
  `src_map.lookup(pc)` answers "which file + line is this instruction".

`SetupExcept` records the frame depth so caught exceptions don't leak
frames pushed inside the try body. The final formatter wraps any
non-`VmInvariant` error in a `RuntimeError::Traced(String)` carrying
the rendered traceback. Test code that builds programs in-memory
without a source map falls through to the raw `Kind: msg` line, so
synthetic VM tests stay readable.

## Built-in functions

| Function     | Description                                |
| ------------ | ------------------------------------------ |
| `chr(n)`     | Single-character string for byte `n`       |
| `ascii(c)`   | Codepoint of single-character string `c`   |
| `length(x)`  | Length of list or string                   |
| `string_bytes(s)` | UTF-8 byte values of `s` as a list of ints |
| `bytes_to_string(bytes)` | Inverse of `string_bytes` |
| `hex(n)`     | Hex string of integer `n` (lowercase)      |
| `binary(n[, width])` | Binary string, optionally masked + padded |
| `freeze(d)`  | Convert a dict to an immutable namespace   |
| `dict_keys(d)` | List the keys of a dict (insertion order) |
| `panic(msg)` / `raise(msg)` | Raise a catchable runtime error |
| `exit_with_error(msg)` | Print to stderr verbatim and exit 1 (uncatchable) |
| `subprocess(argv)` / `exit(code)` / `getpid()` | Process control (used by the OMG-native `omg` driver) |
| `read_file(path)` / `file_exists(path)` | Read text file / existence check |
| `is_dir(path)` / `read_dir(path)` / `make_dir(path)` | Directory ops (`mkdir -p`) |
| `file_open / file_read / file_write / file_close` | Streaming I/O |
| `int(x)` / `float(x)` | Numeric conversions (truncate / widen) |
| `floor / ceil / round` | Round float to int (banker's rounding for `round`) |
| `abs / sqrt / pow / log` | Magnitude, root, power, natural log |
| `sin / cos / tan` | Trig in radians; return float |
| `float_bits(s)` / `bits_to_float(i)` | IEEE-754 bits ↔ float; used by `bootstrap/src/{compiler,vm}.omg` to read/write float literals |
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

## Native compilation

The Rust runtime is one of two execution paths. There's also a
**native-compilation path** that turns OMG source into standalone ELF
binaries with no Rust runtime needed: `bootstrap/src/native-c.omg`
transpiles bytecode to C, which `cc -O2` compiles to a small native
binary. Both paths share this runtime's compiler and bytecode format —
they differ only in the backend that executes the bytecode. See
[`docs/native/`](../docs/native/) for the full guide.

## Links

* [Top-level README](../README.md)
* [Native compilation guide](../docs/native/)
* [Compilation pipeline](../docs/compilation-pipeline.md)
* [OMG-in-OMG tools](../tools/README.md)
* [VS Code extension](../vscode/README.md)
