# OMG Runtime

The **OMG Runtime** is the execution engine for the OMG programming language — a personal, educational language project designed to explore the full stack of compiler and interpreter implementation.  

This crate provides:
- A **stack-based virtual machine** for executing OMG bytecode
- A **tree-walk REPL** for interactive exploration
- A **bytecode loader and function table** format
- A set of **built-in functions** for data, math, and filesystem operations
- A well-defined **error system** for reporting runtime failures

## Features

### 🎛️ Virtual Machine
- Operand stack evaluation model
- Global and local variable environments
- Function call frames (with tail call optimization)
- Exception-style error handling (`setup_except`, `raise`, `pop_block`)

### ⚙️ Instruction Set
Supports all major categories of instructions:
- **Literals**: integers, strings, booleans, `None`
- **Arithmetic & Bitwise**: `+ - * / % & | ^ << >> ~`
- **Logical & Comparison**: `== != < <= > >= and or`
- **Structures**: list/dict building, indexing, slicing, attributes
- **Control Flow**: jumps, conditional jumps, function calls, returns
- **Exceptions**: `assert`, `raise`, `setup_except`
- **I/O**: `emit` to stdout

### 🛠 Built-in Functions
Out-of-the-box helpers for everyday use:
- Conversion: `chr`, `ascii`, `hex`, `binary`
- Introspection: `length`
- Data: `freeze` (immutable dicts)
- Errors: `panic`, `raise`
- Filesystem: `read_file`, `file_exists`
- File I/O (descriptor-based): `file_open`, `file_read`, `file_write`, `file_close`
- Meta: `call_builtin`

### ⚡ REPL
Interactive interpreter for OMG source code:
- Prompts with `>>>` and supports multiline input (`...`)
- Tracks brace depth for entering whole blocks
- Preserves state across commands
- Exits cleanly with `exit` / `quit`

### 🧩 Error System
Two-layer error handling:
- **`ErrorKind`** – compact categories (for bytecode raise ops)
- **`RuntimeError`** – detailed structured errors (with `Display` messages)

Examples:
- `AssertionError`
- `TypeError("expected integer")`
- `IndexError("out of bounds")`
- `VmInvariant("stack underflow")`

## Example

```omg
;;;omg

proc greeting(name) {
    return "Hello " + name
}

emit greeting("World")

alloc xs := [1, 2, 3]
emit "length(xs) = " + length(xs)

# Assertions and loops
facts 2 + 2 == 4

alloc i := 0
loop i < 3 {
    emit "i = " + i
    i := i + 1
}
````

Compile to bytecode (`.omgb`) or run directly through the embedded interpreter.

## Using the REPL

```bash
$ cargo run
OMG Language Interpreter - REPL
Type `exit` or `quit` to leave.
>>> alloc x := 5
>>> emit x * 2
10
```

## Installation

Add to your project:

```toml
[dependencies]
omg_runtime = "0.1.2"
```

Or install the runtime directly:

```bash
cargo install omg_runtime
```

## License

Licensed under the [MIT License](../LICENSE).

## Project Status

OMG is an educational project. The runtime is stable enough to run example scripts and bytecode, but the language is not serious. Expect breaking changes as new features are added.

## Links

* [OMG Language Specification](../spec/OMG_SPEC.md)
* [Lexer Documentation](../spec/OMG_LEXER.md)
* [Parser Documentation](../spec/OMG_PARSER.md)
* [Development Guide](../spec/DEVELOPMENT.md)