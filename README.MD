# OMG Language

OMG is a personal, educational programming language. It now ships with a native runtime written in Rust while retaining the original Python implementation for reference.

The language provides a minimal but expressive syntax, and supports variables, arithmetic, lists, dictionaries, functions, conditionals, loops, modules, and more.

#### Documents:
- [`OMG_SPEC.md`](./spec/OMG_SPEC.md): for the canonical language reference.
- [`OMG_LEXER.md`](./spec/OMG_LEXER.md): describes the lexical analysis stage of execution for the original Python implementation.
- [`OMG_PARSER.md`](./spec/OMG_PARSER.md): describes the parser's structure, flow, and grammar handling for the original Python implementation.

The Python-based lexer, parser, and interpreter also exist as a bootstrapped [OMG interpreter](./bootstrap/interpreter.omg), compiled to binary format and embedded directly into the virtual machine. This allows the language to execute OMG code by interpreting OMG-written interpreter logic inside the VM itself, enabling self-hosting experiments and making the runtime a testbed for metaprogramming and language evolution.


## How to Run OMG Scripts

The native runtime is located in the `runtime` directory and can be built and executed with Cargo.

### Build and Run

```sh
omg-cli runtime-run ./path/to/script.omg
```

This command builds the runtime (if needed) and executes the given `.omg` script. The script must begin with the required `;;;omg` header on the first non-empty line.

After building, the resulting binary is available at `runtime/target/debug/omg` (or `runtime/target/release/omg` when using `--release`):

```sh
./runtime/target/debug/omg path/to/script.omg
```

### Interactive REPL

Run OMG without arguments to start an interactive REPL:

```sh
omg-cli runtime-run
# or after building
./runtime/target/debug/omg
```

Example session:

```plaintext
>>> alloc x := 42
>>> emit x
42
>>> if x > 0 {
...     emit "positive"
... }
positive
>>> quit
```

---

## Syntax

### Statements

| Form             | Purpose                         |
| ---------------- | ------------------------------- |
| `alloc x := 1`   | Declare and initialize variable |
| `x := x + 1`     | Reassign existing variable      |
| `emit x`         | Output value to console         |
| `import "file.omg" as name` | Load another script under an alias |
| `facts x > 0`    | Assert a condition              |
| `if/elif/else`   | Branching control flow          |
| `loop cond {}`   | While-style loop                |
| `break`          | Exit from current loop          |
| `proc name(...)` | Define a function               |
| `return`         | Return from a function          |

Module imports run another `.omg` script and expose its top-level `proc` and `alloc` bindings under a read-only namespace.

---

## Supported Data Types

| Type       | Example                    | Notes                             |
| ---------- | -------------------------- | --------------------------------- |
| Integer    | `42`, `-1`                 | Whole numbers only                |
| Boolean    | `true`, `false`            | Lowercase literals                |
| String     | `"Hello\nWorld"`           | Double-quoted, supports escapes   |
| List       | `[1, 2, 3]`                | Supports indexing and slicing     |
| Dictionary | `{name: "Chris", age: 32}` | Access with `x.key` or `x["key"]` |

### Falsy Values

* `false`
* `""`
* `[]`
* `undefined` (unassigned variables)

---

## Language Design

### Lexer

* Uses a single-pass regex-based scanner to tokenize input.
* Skips whitespace and comments.
* Produces `Token(type, value, line)` tuples.
* Strips `;;;omg` header before processing.

### Parser

* Hand-written recursive-descent parser with LL(1)+ lookahead.
* Divided into:
  * `expressions.py`: Precedence-based expression parsing.
  * `statements.py`: Control flow and structural forms.
* Produces ASTs as tagged tuples: `(operation, operands..., line)`

### Interpreter

* Tree-walk interpreter over the AST.
* Maintains:
  * `vars`: Current scope.
  * `global_vars`: Preserved globals.
* Function calls isolate scopes, bind parameters, and restore the previous environment.
* Expression evaluator supports:
  * Arithmetic, logical, comparison, bitwise ops
  * Lists, slices, dictionaries
  * Function calls, built-ins

---

## Example Snippets

### Declaration and Assignment

```omg
alloc name := "Chris"
emit name
name := name + "!"
emit name
```

### Arithmetic

```omg
alloc x := 10
alloc y := 3
emit x + y
emit x * y
emit x % y
```

### Control Flow and Conditionals

```omg
alloc score := 75

if score >= 90 {
    emit "A"
} elif score >= 80 {
    emit "B"
} elif score >= 70 {
    emit "C"
} else {
    emit "F"
}
```

### Loops

```omg
alloc i := 0
loop i < 5 {
    emit i
    i := i + 1
}
```

### Binary Ops

```omg
emit 6 & 3      # 2
emit 6 | 3      # 7
emit 6 ^ 3      # 5
emit ~1         # -2
emit 1 << 3     # 8
```

### Function Definitions

```omg
proc square(x) {
    return x * x
}

emit square(4)

proc call_twice(f, x) {
    return f(f(x))
}

emit call_twice(square, 2)  # 16
```

### Module Imports

```omg
import "utils.omg" as utils
emit utils.greet("world")
```

---

## License and Status

OMG is an experimental educational project and not intended for production use. The implementation is stable and complete as a toy language but may evolve to support additional features like deeper type introspection.

This software is open-sourced software licensed under the [MIT license](./LICENSE).