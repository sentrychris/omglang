[![OMGlang](https://github.com/sentrychris/omglang/actions/workflows/python-package.yml/badge.svg)](https://github.com/sentrychris/omglang/actions/workflows/python-package.yml) &nbsp;&nbsp;&nbsp;[![CodeQL](https://github.com/sentrychris/omglang/actions/workflows/github-code-scanning/codeql/badge.svg)](https://github.com/sentrychris/omglang/actions/workflows/github-code-scanning/codeql)

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

The lexer performs a single pass over the source using a combined regular
expression of named groups. Each match produces a `Token` containing the
token type, its value, and the original line number for later error
reporting. Before tokenization the required header `;;;omg` is stripped so
that line counts remain accurate.

Token categories include:

* **Keywords**: `alloc`, `emit`, `proc`, `if`, `elif`, `else`, `loop`,
  `break`, `return`, `facts`.
* **Operators**:
    * Arithmetic: `+`, `-`, `*`, `/`, `%`
    * Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
    * Assignment: `:=`
    * Bitwise: `&`, `|`, `^`, `~`, `<<`, `>>`
    * Logical: `and`
* **Identifiers** for variable and function names.
* **Literals**: decimal integers, binary integers (e.g. `0b1010`), strings
  with escape sequences, and the boolean literals `true` and `false`.
* **Punctuation**: `(`, `)`, `{`, `}`, `[`, `]`, `,`, `:`.

Whitespace and comments beginning with `#` are skipped entirely. A mapping of
literal text back to token types is also generated, allowing the parser to
emit human‑readable error messages such as “expected `}`” rather than the
internal token name.

## Parser

OMG uses a hand‑written, modular **recursive‑descent parser**. The top‑level
`Parser` class (in `core/parser/parser.py`) coordinates the process while the
actual routines live in two helper modules: `expressions.py` and
`statements.py`. This separation keeps expression precedence logic and
statement forms isolated and easy to extend.

Most production rules are LL(1), meaning the next token alone is sufficient
to choose the correct parsing function. However, the statement parser
occasionally peeks one additional token ahead, such as to distinguish
`name :=` (a reassignment) from `name(` (a function call) or a bare
expression, making the grammar **sporadic LL(2)** rather than strictly LL(1).

Parsing advances through a classic `eat()` routine that verifies the expected
token type and then consumes it, raising a descriptive `SyntaxError` if the
stream does not match the grammar. The resulting AST is encoded as tuples of
the form `(operation, operands…, line)`.

### Expression Precedence (highest to lowest):

| Level | Category           | Operators                  | Parser Function |
| ----- | ------------------ | -------------------------- | --------------- |
| 1     | **Unary**          | `~`, `+`, `-`              | `_factor`       |
| 2     | **Multiplicative** | `*`, `/`, `%`              | `_term`         |
| 3     | **Additive**       | `+`, `-`                   | `_add_sub`      |
| 4     | **Shift**          | `<<`, `>>`                 | `_shift`        |
| 5     | **Bitwise AND**    | `&`                        | `_bitwise_and`  |
| 6     | **Bitwise XOR**    | `^`                        | `_bitwise_xor`  |
| 7     | **Bitwise OR**     | `\|`                        | `_bitwise_or`   |
| 8     | **Comparison**     | `==`, `!=`, `<`, `>`, `<=`, `>=` | `_comparison`   |
| 9     | **Logical AND**    | `and`                      | `_logical_and`  |

Logical AND is left-associative and evaluates with short-circuit semantics, so a chain like `a and b and c` is parsed as `(a and b) and c`, and produces a boolean result.

Each layer wraps the tighter-binding expressions from below, ensuring proper operator grouping. This model provides clear operator precedence semantics.

Each node is represented as a tuple encoding the operation, its operands and the originating line number. For example:

```python
('add', ('number', 5, 1), ('number', 1, 1), 1)
('xor_bits', ('number', 6, 3), ('number', 3, 3), 3) 
```

## Interpreter

The interpreter is a tree‑walker that evaluates the tuple‑based AST produced
by the parser. It maintains two environments:

* A **variable table** (`vars`) for current scope values. A copy of the global
  table is preserved so function calls can introduce temporary local scopes
  without losing global state.
* A **function table** (`functions`) mapping procedure names to their
  parameter lists and bodies.

### Expression Evaluation

`eval_expr(node)` recursively evaluates literals, variables, arithmetic and
bitwise operations, boolean logic, list indexing/slicing, and function calls.
Unary operators include `+`, `-`, and `~`. Built‑in functions such as
`chr`, `ascii`, `hex`, `binary`, and `length` are dispatched directly, while
user‑defined procedures evaluate arguments, bind parameters, execute the body
and return the resulting value (or `None` if no `return` is encountered).
`binary` also supports an optional width argument for fixed-width two's
complement formatting.
Logical `and` short‑circuits: the right‑hand side is evaluated only if the left‑hand side is truthy, and the operator always produces a boolean value.

### Statement Execution

`execute(statements)` walks a list of statement nodes and performs the
appropriate action. Supported statements include:

* **alloc** – declare a variable.
    * To update an existing variable, use the variable identifier followed by the assignment operator (`:=`).
        * If no local binding exists, this
  mutates the global variable
        * Attempting to update a variable that hasn't previously been declared with `alloc` will raise an `UndefinedVariableException`.
* **emit** – evaluate an expression and print the result.
* **facts** – assert that an expression is truthy.
* **if/elif/else** – conditional execution.
* **loop** and **break** – while loops with early exit support.
* **proc** – define a function.
* **return** – exit a function with a value.

Each function call creates a new local scope populated with global variables
and the call's parameters. Locals from the caller are hidden, and the previous
environment is restored when the call completes. The interpreter also verifies
that scripts start with the required `;;;omg` header before execution.

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
* **Scope model** – variables are global except during function calls, which create a local scope with access only to globals and parameters.
- **Operator precedence** – enforced explicitly in the parser via method hierarchy.
* **Error handling** – line numbers and file names are preserved for better error messages.
* **Extensibility** – new constructs can be added by expanding the parser and interpreter.
