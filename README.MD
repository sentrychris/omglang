# OMGlang

**OMG** is a toy programming language with a silly syntax and basic support for variables, arithmetic, conditionals, loops, and functions. It’s implemented in Python using a hand-written recursive descent parser and tree-walk interpreter.

## Table of Contents

* [Overview](#overview)
* [Lexer](#lexer)
* [Parser](#parser)
* [Interpreter](#interpreter)
* [Example](#example)
* [Design Notes](#design-notes)

## Overview

The language uses playful keywords like `thingy`, `saywhat`, and `hamsterwheel`, but is structured with a serious underlying grammar. It enforces a header line (`;;;omg`) and processes code in the following pipeline:

1. **Lexing** – Convert raw source code into tokens.
2. **Parsing** – Build an Abstract Syntax Tree (AST) from tokens.
3. **Interpreting** – Walk the AST to evaluate expressions and execute statements.

## Lexer

The lexer (tokenizer) scans the raw text and splits it into a stream of tokens. Each token includes a type, value, and line number. Token types include:

* **Keywords**: `thingy`, `saywhat`, `bitchin`, `maybe`, `okthen`, `hamsterwheel`
* **Operators**: `+`, `-`, `*`, `/`, `%`, `=`, `==`, `<`, `>`, `<=`, `>=`
* **Identifiers**: variable and function names
* **Literals**: integers and strings
* **Punctuation**: `(`, `)`, `{`, `}`, `,`

Whitespace and comments are ignored. The lexer is line-aware to provide precise error reporting.

## Parser

The parser is a **recursive descent parser**, manually written to follow an LL(1)-style grammar. It builds an AST from tokens using mutually recursive methods like:

* `expr()` — handles addition and subtraction
* `term()` — handles multiplication, division, modulus
* `factor()` — handles literals, variables, and parenthesized expressions
* `statement()` — parses individual statements (assignments, output, control flow)
* `block()` — parses a sequence of statements inside `{ ... }`
* `function_def()` and `function_call()` — parse user-defined function declarations and invocations

Each node in the AST is a tuple that encodes the operation, its operands, and the line number.

**Example AST node**:

```python
('add', ('thingy', 'x', 3), ('number', 5, 3), 3)
```

---

## Interpreter

The interpreter walks the AST and evaluates expressions or executes statements. It uses a variable environment (`self.vars`) and a function environment (`self.functions`).

### Expression Evaluation

Handled by `eval_expr(node)`:

* Literals return their value.
* Variables are looked up in the environment.
* Binary operations are recursively evaluated.
* Function calls evaluate arguments, bind them to parameters, and execute the function body.

### Statement Execution

Handled by `execute(statements)`:

* **thingy**: evaluates and stores a value.
* **saywhat**: evaluates and prints an scalar value, term, variable or expression.
* **maybe/okthen**: conditionally executes blocks.
* **hamsterwheel**: executes a block repeatedly while a condition holds.
* **bitchin (func\_def)**: registers a named function.
    * **(func\_call)**: invokes a registered function with evaluated arguments.

Each function call creates a new scope for its parameters, and the previous variable state is restored afterward (no closures or return values yet).

## Example

Examples can be found in the **examples** directory.

```php
;;;omg

# Define a function
bitchin greeting(name) {
    gimme "Hello " + name
}

# Call a function and print to stdout
saywhat << greeting("World")

# Declare some variables
thingy x := 5
thingy y := 10
thingy z := 0

saywhat << "x = " + x
saywhat << "y = " + y

bitchin add(a, b) {
    gimme a + b
}

# Do some arithmetic and format for stdout
saywhat << x + " + " + y + " = " + add(x, y)
saywhat << "(" + x + " + " + y + ") * 10 / 2 = " + (x + y) * 10 / 2

# Create a loop
saywhat << "Beginning loop:"
thingy i := 0
hamsterwheel i <= 3 {
    maybe i == 3 {
        saywhat << "3 is the magic number."
    } oractually i == 2 {
        saywhat << "2 is alright I guess."
    } okthen {
        saywhat << i
    }
    thingy i := i + 1
}
saywhat << "Finished loop."
```

Output:

```
> py .\bin\oli.exe .\examples\example1.omg
Hello World
x = 5
y = 10
5 + 10 = 15
(5 + 10) * 10 / 2 = 75
Beginning loop:
0
1
2 is alright I guess.
3 is the magic number.
Finished loop.
```

---

## Design Notes

* **Header requirement**: Scripts must begin with `;;;omg` to be valid.
* **Scope model**: Variables are global except during function calls, which create a local scope.
* **Error handling**: Line numbers and file names are preserved for better error messages.
* **Extensibility**: New constructs can be added by expanding the parser and interpreter tuples.