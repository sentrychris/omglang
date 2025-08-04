# OMGlang

OMG is a toy programming language with a minimal, custom syntax and basic support for variables, arithmetic, conditionals, loops, and functions. It is implemented in Python using a hand‑written lexer, a recursive‑descent parser, and a tree‑walk interpreter.

## Table of Contents

* [Overview](#overview)
* [Running OMG Scripts](#running-omg-scripts)
  * [Usage](#usage)
  * [Interactive REPL](#interactive-repl)
* [Lexer](#lexer)
* [Parser](#parser)
* [Interpreter](#interpreter)
* [Example](#example)
* [Design Notes](#design-notes)

## Overview

The language uses keywords like `alloc`, `emit`, `if` and `loop` and is grounded in a well‑defined grammar. All scripts must start with a header line `;;;omg`. Execution proceeds through three stages:

1. **Lexing** – convert raw source code into tokens.
2. **Parsing** – build an Abstract Syntax Tree (AST) from tokens.
3. **Interpreting** – walk the AST to evaluate expressions and execute statements.

## Running OMG Scripts

Once you've installed or built the interpreter, the command‑line entry point is named `omg`. It can execute a script file or drop you into an interactive session if run with no arguments.

### Usage

```
OLI - OMG Language Interpreter

Usage:
    oli <script.omg>

Arguments:
    <script.omg>
        Path to an OMG language source file to execute. The file must
        include the required header ';;;omg' on the first non-empty line.

Example:
    oli hello.omg

Or run with no arguments to enter interactive mode (REPL).

Options:
    -h, --help
        Show this help message and exit.
```

* `omg <script.omg>` – run the specified OMG program. The file must include the required `;;;omg` header on the first non‑empty line. Relative paths are resolved from your current working directory.

* `omg` with no arguments – start a Read–Eval–Print Loop (REPL) where you can type OMG statements interactively. Multi‑line constructs such as function definitions or `if … { … }` blocks are supported; simply continue entering lines until the braces balance. Type `exit` or `quit` to leave the REPL.

* `-h` or `--help` – print a short usage summary and exit.

If you prefer not to install the `omg` script, you can also run programs directly via Python:

```sh
py ./omg.py <script>.omg
```

### Interactive REPL

Running `omg` with no arguments launches an interactive session. This REPL lets you experiment with the language one statement or block at a time. For example:

```plaintext
>>> alloc x := 5
>>> emit x
5
>>> if x < 10 {
...     emit x + " is smaller than 10"
... } else {
...     emit x + " is bigger than 10"
... }
x is smaller than 10
>>> exit
```

If you start a block and do not close it, the prompt switches to `...` and the interpreter waits for you to finish entering the block.

## Lexer

The lexer scans the raw text and splits it into a stream of tokens. Each token has a type, value and line number. Token types include:

* **Keywords**: `alloc`, `emit`, `proc`, `if`, `elif`, `else`, `loop`, `break`, `return`.
* **Operators**:
    * Arithmetic: `+`, `-`, `*`, `/`, `%`
    * Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
    * Assignment: `:=`
    * Bitwise: `&`, `|`, `^`, `~`, `<<`, `>>`
    * Logical: `and`
* **Identifiers**: variable and function names.
* **Literals**: integers and strings.
* **Punctuation**: `(`, `)`, `{`, `}`, `[`, `]`, `,`, `:`.

Whitespace and comments (lines starting with `#`) are ignored. The lexer records line numbers to improve error messages.

## Parser

The parser is a hand‑written **recursive descent parser** following an LL(1) grammar. It converts the token stream into an AST using mutually recursive methods that enforce operator precedence.

### Expression Precedence (highest to lowest):

| Level | Category           | Operators                  | Parser Function |
| ----- | ------------------ | -------------------------- | --------------- |
| 1     | **Unary**          | `~`                        | `_factor`       |
| 2     | **Multiplicative** | `*`, `/`, `%`              | `_term`         |
| 3     | **Additive**       | `+`, `-`                   | `_add_sub`      |
| 4     | **Shift**          | `<<`, `>>`                 | `_shift`        |
| 5     | **Bitwise AND**    | `&`                        | `_bitwise_and`  |
| 6     | **Bitwise XOR**    | `^`                        | `_bitwise_xor`  |
| 7     | **Bitwise OR**     | `\|`                        | `_bitwise_or`   |
| 8     | **Comparison**     | `==`, `!=`, `<`, `>`, `<=`, `>=` | `_comparison`   |
| 9     | **Logical AND**    | `and`                      | `_logical_and`  |

Each layer wraps the tighter-binding expressions from below, ensuring proper operator grouping. This model gives OMG clear operator precedence semantics.

Each AST node is represented as a tuple encoding the operation, its operands and the originating line number. For example:

```python
('add', ('number', 5, 1), ('number', 1, 1), 1)
('xor_bits', ('number', 6, 3), ('number', 3, 3), 3) 
```

## Interpreter

The interpreter walks the AST and evaluates expressions or executes statements. It maintains:

* A **variable environment** (`vars`) for storing values bound by `alloc` declarations.
* A **function environment** (`functions`) for storing user‑defined functions.

### Expression Evaluation

Handled by `eval_expr(node)`: literals return their value; variables are looked up in the current scope; binary operations are recursively evaluated (with support for `+` for both numeric addition and string concatenation); function calls evaluate their arguments, bind them to parameters, execute the function body and return the result (or `None` if no `return` statement is encountered).

### Statement Execution

Handled by `execute(statements)`. Supported statements include:

* **alloc** – evaluate an expression and store it in the variable environment.

* **emit** – evaluate an expression and print the result.

* **if/elif/else** – evaluate a condition and execute one of several blocks conditionally.

* **loop** – execute a block repeatedly while a condition holds.

* **proc** – define a named function with parameters and body.

* **return** – return a value from a function.

Each function call creates a new scope for its parameters, and the previous variable state is restored afterward (there are no closures or first‑class functions yet).

## Example

Examples can be found in the examples directory.

```python
;;;omg

# Define a function
proc greeting(name) {
    return "Hello " + name
}

# Call a function and print to stdout
emit greeting("World")

# Declare some variables
alloc x := 5
alloc y := 10
alloc z := 0

emit "x = " + x
emit "y = " + y

proc add(a, b) {
    return a + b
}

# Do some arithmetic and format for stdout
emit x + " + " + y + " = " + add(x, y)
emit "(" + x + " + " + y + ") * 10 / 2 = " + (x + y) * 10 / 2

# Create a loop
emit "Beginning loop:"
alloc i := 0
loop i <= 3 {
    if i == 3 {
        emit "3 is the magic number."
    } elif i == 2 {
        emit "2 is alright I guess."
    } else {
        emit i
    }
    i := i+1
}
emit "Finished loop."
```

## Design Notes

* **Header requirement** – scripts must begin with `;;;omg` to be valid.
* **Scope model** – variables are global except during function calls, which create a local scope.
- **Operator precedence** – enforced explicitly in the parser via method hierarchy.
* **Error handling** – line numbers and file names are preserved for better error messages.
* **Extensibility** – new constructs can be added by expanding the parser and interpreter.
