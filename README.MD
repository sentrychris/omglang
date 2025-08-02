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

The language uses playful keywords like `thingy`, `saywhat`, `maybe` and `roundabout`, but it's grounded in a well‑defined grammar. All scripts must start with a header line `;;;omg`. Execution proceeds through three stages:

1. **Lexing** – convert raw source code into tokens.
2. **Parsing** – build an Abstract Syntax Tree (AST) from tokens.
3. **Interpreting** – walk the AST to evaluate expressions and execute statements.

## Running OMG Scripts

Once you've installed or built the interpreter, the command‑line entry point is named `omg`. It can execute a script file or drop you into an interactive session if run with no arguments.

### Usage

```
omg <script.omg>
omg -h | --help
omg
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
>>> saywhat x
5
>>> maybe x < 10 {
...     saywhat x + " is smaller than 10"
... } okthen {
...     saywhat x + " is bigger than 10"
... }
x is smaller than 10
>>> exit
```

If you start a block and do not close it, the prompt switches to `...` and the interpreter waits for you to finish entering the block.

## Lexer

The lexer scans the raw text and splits it into a stream of tokens. Each token has a type, value and line number. Token types include:

* **Keywords**: `thingy`, `saywhat`, `bitchin`, `maybe`, `oractually`, `okthen`, `roundabout`.
* **Operators**: `+`, `-`, `*`, `/`, `%`, `:=`, `<<`, `==`, `<`, `>`, `<=`, `>=`.
* **Identifiers**: variable and function names.
* **Literals**: integers and strings.
* **Punctuation**: `(`, `)`, `{`, `}`, `,`.

Whitespace and comments (lines starting with `#`) are ignored. The lexer records line numbers to improve error messages.

## Parser

The parser is a hand‑written **recursive descent parser** following an LL(1) grammar. It converts the token stream into an AST using mutually recursive methods:

* `_factor()` – handles literals, variables and parenthesised expressions.

* `_term()` – handles multiplication, division and modulus.

* `_expr()` – handles addition and subtraction.

* `_comparison()` - handles comparison operators.

* `_statement()` – parses individual statements (assignments, output, control flow).

* `_block()` – parses a sequence of statements inside `{ … }`.

Each AST node is represented as a tuple encoding the operation, its operands and the originating line number. For example:

```python
('add', ('thingy', 'x', 3), ('number', 5, 3), 3)
```

## Interpreter

The interpreter walks the AST and evaluates expressions or executes statements. It maintains:

* A **variable environment** (`vars`) for storing values bound by `thingy` declarations.
* A **function environment** (`functions`) for storing user‑defined functions.

### Expression Evaluation

Handled by `eval_expr(node)`: literals return their value; variables are looked up in the current scope; binary operations are recursively evaluated (with support for both numeric addition and string concatenation); function calls evaluate their arguments, bind them to parameters, execute the function body and return the result (or `None` if no `gimme` statement is encountered).

### Statement Execution

Handled by `execute(statements)`. Supported statements include:

* **thingy** – evaluate an expression and store it in the variable environment.
* **saywhat** – evaluate an expression and print the result.
* **maybe/oractually/okthen** – evaluate a condition and execute one of several blocks conditionally.
* **roundabout** – execute a block repeatedly while a condition holds.
* **bitchin** – register a named function. Subsequent calls to that function will evaluate arguments, bind them to parameters and execute the function body in a local scope.

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
saywhat greeting("World")

# Declare some variables
thingy x := 5
thingy y := 10
thingy z := 0

saywhat "x = " + x
saywhat "y = " + y

bitchin add(a, b) {
    gimme a + b
}

# Do some arithmetic and format for stdout
saywhat x + " + " + y + " = " + add(x, y)
saywhat "(" + x + " + " + y + ") * 10 / 2 = " + (x + y) * 10 / 2

# Create a loop
saywhat "Beginning loop:"
thingy i := 0
roundabout i <= 3 {
    maybe i == 3 {
        saywhat "3 is the magic number."
    } oractually i == 2 {
        saywhat "2 is alright I guess."
    } okthen {
        saywhat i
    }
    thingy i := i + 1
}
saywhat "Finished loop."
```

## Design Notes

* **Header requirement** – scripts must begin with `;;;omg` to be valid.
* **Scope model** – variables are global except during function calls, which create a local scope.
* **Error handling** – line numbers and file names are preserved for better error messages.
* **Extensibility** – new constructs can be added by expanding the parser and interpreter.
