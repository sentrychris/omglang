# OMGlang

**OMG** is a toy programming language with a light‑hearted syntax and basic support for variables, arithmetic, conditionals, loops and functions. It's implemented in Python using a hand‑written lexer, a recursive‑descent parser and a tree‑walk interpreter. The language is intended as an educational exercise rather than a production tool.

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

The language uses playful keywords like `thingy`, `woah`, `maybe` and `roundabout`, but it's grounded in a well‑defined grammar. All scripts must start with a header line `;;;omg`. Execution proceeds through three stages:

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

* `omg` with no arguments – start a Read–Eval–Print Loop (REPL) where you can type OMG statements interactively. Multi‑line constructs such as function definitions or `maybe … { … }` blocks are supported; simply continue entering lines until the braces balance. Type `exit` or `quit` to leave the REPL.

* `-h` or `--help` – print a short usage summary and exit.

If you prefer not to install the `omg` script, you can also run programs directly via Python:

```sh
py ./omg.py <script>.omg
```

### Interactive REPL

Running `omg` with no arguments launches an interactive session. This REPL lets you experiment with the language one statement or block at a time. For example:

```plaintext
>>> thingy x := 5
>>> woah x
5
>>> maybe x < 10 {
...     woah x + " is smaller than 10"
... } okthen {
...     woah x + " is bigger than 10"
... }
x is smaller than 10
>>> exit
```

If you start a block and do not close it, the prompt switches to `...` and the interpreter waits for you to finish entering the block.

## Lexer

The lexer scans the raw text and splits it into a stream of tokens. Each token has a type, value and line number. Token types include:

* **Keywords**: `thingy`, `woah`, `bitchin`, `maybe`, `oractually`, `okthen`, `roundabout`.
* **Operators**:
    * Arithmetic: `+`, `-`, `*`, `/`, `%`
    * Comparison: `==`, `<`, `>`, `<=`, `>=`
    * Assignment: `:=`
    * Output: `<<`
    * Bitwise: `&`, `|`, `^`, `~`, `<<`, `>>`
* **Identifiers**: variable and function names.
* **Literals**: integers and strings.
* **Punctuation**: `(`, `)`, `{`, `}`, `[`, `]`, `,`, `:`.

Whitespace and comments (lines starting with `#`) are ignored. The lexer records line numbers to improve error messages.

## Parser

The parser is a hand‑written **recursive descent parser** following an LL(1) grammar. It converts the token stream into an AST using mutually recursive methods that enforce operator precedence.

### Expression Precedence (highest to lowest):

```
_expr
  └── _bitwise_or
        └── _bitwise_xor
              └── _bitwise_and
                    └── _shift
                          └── _add_sub
                                └── _term
                                      └── _factor
```

| Level | Category           | Operators                  | Parser Function |
| ----- | ------------------ | -------------------------- | --------------- |
| 1     | **Unary**          | `~`                        | `_factor`       |
| 2     | **Multiplicative** | `*`, `/`, `%`              | `_term`         |
| 3     | **Additive**       | `+`, `-`                   | `_add_sub`      |
| 4     | **Shift**          | `<<`, `>>`                 | `_shift`        |
| 5     | **Bitwise AND**    | `&`                        | `_bitwise_and`  |
| 6     | **Bitwise XOR**    | `^`                        | `_bitwise_xor`  |
| 7     | **Bitwise OR**     | `\|`                        | `_bitwise_or`   |
| 8     | **Comparison**     | `==`, `<`, `>`, `<=`, `>=` | `_comparison`   |

Each layer wraps the tighter-binding expressions from below, ensuring proper operator grouping. This model gives OMG clear operator precedence semantics.


### Parsing Methods:

* `_factor()` – handles literals, variables, function calls, indexing, slicing, and unary `~`.

* `_term()` – handles multiplication, division and modulus.

* `_add_sub()` – handles addition and subtraction.

* `_shift()` – handles `<<`, `>>`.

* `_bitwise_and()` – handles `&`.

* `_bitwise_xor()` – handles `^`.

* `_bitwise_or()` – handles `|`.

* `_expr()` – entry point for expression parsing.

* `_comparison()` – handles relational comparisons.

* `_statement()` – parses assignments, output, control flow, etc.

* `_block()` – parses a sequence of statements inside { … }.

Each AST node is represented as a tuple encoding the operation, its operands and the originating line number. For example:

```python
('xor_bits', ('number', 6, 3), ('number', 3, 3), 3)
```

## Interpreter

The interpreter walks the AST and evaluates expressions or executes statements. It maintains:

* A **variable environment** (`vars`) for storing values bound by `thingy` declarations.
* A **function environment** (`functions`) for storing user‑defined functions.

### Expression Evaluation

Handled by `eval_expr(node)`: literals return their value; variables are looked up in the current scope; binary operations are recursively evaluated (with support for `+` for both numeric addition and string concatenation); function calls evaluate their arguments, bind them to parameters, execute the function body and return the result (or `None` if no `gimme` statement is encountered).

### Statement Execution

Handled by `execute(statements)`. Supported statements include:

* **thingy** – evaluate an expression and store it in the variable environment.

* **woah** – evaluate an expression and print the result.

* **maybe/oractually/okthen** – evaluate a condition and execute one of several blocks conditionally.

* **roundabout** – execute a block repeatedly while a condition holds.

* **bitchin** – define a named function with parameters and body.

* **gimme** – return a value from a function.

Each function call creates a new scope for its parameters, and the previous variable state is restored afterward (there are no closures or first‑class functions yet).

## Example

Examples can be found in the examples directory.

```php
;;;omg

# Define a function
bitchin greeting(name) {
    gimme "Hello " + name
}

# Call a function and print to stdout
woah greeting("World")

# Declare some variables
thingy x := 5
thingy y := 10
thingy z := 0

woah "x = " + x
woah "y = " + y

bitchin add(a, b) {
    gimme a + b
}

# Do some arithmetic and format for stdout
woah x + " + " + y + " = " + add(x, y)
woah "(" + x + " + " + y + ") * 10 / 2 = " + (x + y) * 10 / 2

# Create a loop
woah "Beginning loop:"
thingy i := 0
roundabout i <= 3 {
    maybe i == 3 {
        woah "3 is the magic number."
    } oractually i == 2 {
        woah "2 is alright I guess."
    } okthen {
        woah i
    }
    thingy i := i + 1
}
woah "Finished loop."
```

## Design Notes

* **Header requirement** – scripts must begin with `;;;omg` to be valid.
* **Scope model** – variables are global except during function calls, which create a local scope.
- **Operator precedence** – enforced explicitly in the parser via method hierarchy.
* **Error handling** – line numbers and file names are preserved for better error messages.
* **Extensibility** – new constructs can be added by expanding the parser and interpreter.
