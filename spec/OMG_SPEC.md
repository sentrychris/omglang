# OMG Language Specification

OMG is a personal, educational programming language created to explore the full stack of language implementation: lexical analysis, parsing, interpretation, and runtime semantics. It is designed with readability and simplicity in mind, while supporting essential programming constructs including variables, functions, expressions, loops, and conditionals.

The entire toolchain (lexer, parser, and interpreter) is implemented in Python. This document defines the core language features, syntax, semantics, built-in functions, and interpreter behavior.

---

## Script Structure

Every OMG script must begin with the following header:

```omg
;;;omg
```

If this header is missing, the interpreter will raise an error and refuse to execute the program.

---

## Statements

| Statement      | Purpose                            | Example                           |
| -------------- | ---------------------------------- | --------------------------------- |
| `alloc`        | Declare and initialize variable    | `alloc x := 5`                    |
| `:=`           | Reassign an existing variable      | `x := x + 1`                      |
| `emit`         | Output value to console            | `emit "Hello"`                    |
| `facts`        | Assert a condition (like `assert`) | `facts x > 0`                     |
| `if/elif/else` | Conditional branches               | See below                         |
| `loop`         | While-style loop                   | `loop x < 5 { ... }`              |
| `break`        | Exit from current loop             | `break`                           |
| `proc`         | Define function                    | `proc add(a, b) { return a + b }` |
| `return`       | Return value from function         | `return result`                   |

### Conditional Example

```omg
if x > 0 {
    emit "positive"
} elif x == 0 {
    emit "zero"
} else {
    emit "negative"
}
```

### Loop Example

```omg
loop x > 0 {
    emit x
    x := x - 1
}
```

---

## Expressions

Expressions support full operator precedence and associativity, including unary, binary, logical, bitwise, and comparison operations.

### Precedence Table

| Level | Category       | Operators                        | Parser Function |   |
| ----- | -------------- | -------------------------------- | --------------- | - |
| 1     | Unary          | `~`, `+`, `-`                    | `_factor`       |   |
| 2     | Multiplicative | `*`, `/`, `%`                    | `_term`         |   |
| 3     | Additive       | `+`, `-`                         | `_add_sub`      |   |
| 4     | Shift          | `<<`, `>>`                       | `_shift`        |   |
| 5     | Bitwise AND    | `&`                              | `_bitwise_and`  |   |
| 6     | Bitwise XOR    | `^`                              | `_bitwise_xor`  |   |
| 7     | Bitwise OR             | `\|`                               | `_bitwise_or`   |   |
| 8     | Comparison     | `==`, `!=`, `<`, `>`, `<=`, `>=` | `_comparison`   |   |
| 9     | Logical AND    | `and`                            | `_logical_and`  |   |
| 10    | Logical OR     | `or`                             | `_logical_or`   |   |

Logical `and` and `or` use short-circuit evaluation and always return a boolean.

---

## Data Types

| Type    | Example         | Notes                                       |
| ------- | --------------- | ------------------------------------------- |
| Integer | `1`, `-1`       | Whole numbers only                          |
| String  | `"Hello"`       | Double-quoted, supports escape sequences    |
| Boolean | `true`, `false` | Lowercase literals                          |
| List    | `[1, 2, 3]`     | Python-style indexing and slicing supported |
| Dictionary | `{name: "Chris"}` | Key/value pairs accessed via `x.key` or `x["key"]` |

### Falsy Values

The following evaluate to false in a boolean context:

* `false`
* `""`
* `[]`
* `undefined` (unassigned variables)

---

## Built-in Functions

| Function           | Description                               |
| ------------------ | ----------------------------------------- |
| `binary(n)`        | Binary string of `n`, preserving sign     |
| `binary(n, width)` | Masked and zero-padded to given width     |
| `hex(n)`           | Hexadecimal representation of integer `n` |
| `ascii(char)`      | ASCII code of single character            |
| `chr(n)`           | Character corresponding to ASCII code `n` |
| `length(x)`        | Length of a list or string                |

---

## First-Class Functions and Closures

Functions in OMG are values. Defining a function assigns it to a
variable of the same name. Functions can be stored in variables, passed
as arguments, and returned from other functions.

```omg
proc call_twice(f, x) { return f(f(x)) }
proc inc(n) { return n + 1 }
emit call_twice(inc, 3)   ; prints 5
```

Nested functions capture variables from the scope where they are
defined, forming lexical closures. Captured values are preserved even if
the inner function is returned or stored elsewhere.

```omg
proc make_adder(n) {
    proc inner(x) { return x + n }
    return inner
}

alloc add5 := make_adder(5)
emit add5(10)   ; prints 15
```

---

## Interpreter Semantics

### Environments

* `vars`: The current variable environment (scope)
* `global_vars`: A preserved copy of global variables for function call isolation

### Expression Evaluation

* Handled by `eval_expr(node)`
* Operates recursively over AST tuples
* Enforces type correctness and handles short-circuit logic

### Statement Execution

* Handled by `execute(statements)`
* Supports `decl`, `assign`, `emit`, `facts`, `if`, `loop`, `break`, `func_def`, `return`, `expr_stmt`, and `block`
* Function calls isolate scope, bind parameters, and restore state afterward
* Scripts must begin with `;;;omg` header

---

## Example Program

```omg
;;;omg

proc greeting(name) {
    return "Hello " + name
}

emit greeting("World")

alloc x := 5
alloc y := 10
emit "x + y is " + (x + y)

proc add(a, b) {
    return a + b
}

emit add(3, 4)
```

---

## Status

OMG is a fully working educational language with complete support for:

* Lexing (via named regex groups)
* Parsing (recursive descent with full precedence handling)
* Interpretation (tree-walk runtime with isolated scopes)
