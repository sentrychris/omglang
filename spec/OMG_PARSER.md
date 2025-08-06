# OMG Parsing System

This document explains the internal design of the OMG language parser. It covers the parser architecture, grammar implementation, expression precedence, and the construction of Abstract Syntax Trees (ASTs). The goal is to clarify how OMG source code is transformed from a linear token stream into a structured, evaluable program.

## Overview

The OMG parser is a **modular, hand-written recursive-descent parser**. It is designed for clarity and flexibility, using explicit function calls to represent grammar rules. The parser converts a list of tokens (produced by the lexer) into a sequence of AST nodes that can be directly interpreted.

The top-level class is `Parser`, defined in `core/parser/parser.py`. Parsing logic is divided across:

- `parser.py` – coordinates parsing and manages state
- `expressions.py` – handles all expressions, including literals, operations, and precedence
- `statements.py` – handles all top-level and block-level statements

## Grammar Model

OMG is a line-oriented, block-structured language. The grammar is primarily **LL(1)** with limited **LL(2)** lookahead used to distinguish ambiguous constructs (e.g., `name :=` vs `name(...)`).

Parsing proceeds in two phases:

1. **Statement Parsing** – processes constructs like `alloc`, `if`, `loop`, `proc`, etc.
2. **Expression Parsing** – recursively resolves subexpressions according to operator precedence.


## Token Input

The parser consumes a list of `Token` objects produced by the lexer, which includes the token type, its value, and the line number. It also receives a mapping of literal strings to token types for improved error messages.

```python
Parser(tokens: list, token_map: dict, file: str)
```

Tokens are consumed using the `eat(expected_type)` method, which either advances the stream or raises a `SyntaxError`.

## AST Format

The parser emits AST nodes as tuples. Each node has the general form:

```python
(operation, operand1, operand2, ..., line_number)
```

Examples:

```python
('add', ('number', 5, 1), ('number', 1, 1), 1)
('if', cond_expr, then_block, else_block, 8)
('func_def', 'f', ['x'], body_node, 12)
```

These nodes are later walked by the interpreter.

## Statement Parsing

Statement parsing functions are defined in `statements.py`. The main dispatcher is `parse_statement()`, which examines the current token and delegates to a handler:

* `alloc x := 1` → `('decl', 'x', expr_node, line)`

* `x := 5` → `('assign', 'x', expr_node, line)`

* `emit x` → `('emit', expr_node, line)`

* `import "utils.omg" as utils` → `('import', "utils.omg", "utils", line)`

* `facts cond` → `('facts', expr_node, line)`

* `loop cond { ... }` → `('loop', cond_node, block_node, line)`

* `if cond { ... } elif ... else { ... }` → nested `('if', cond, then, else, 
line)`

* `proc f(x) { return x + 1 }` → `('func_def', name, params, body, line)`

* `return expr` → `('return', expr_node, line)`

* `break` → `('break', line)`

* `expr_stmt` (standalone expression) → `('expr_stmt', expr_node, line)`

Each statement form corresponds to a node in the interpreter's dispatcher.


## Expression Parsing

Expressions are parsed with strict operator precedence using a descending chain of functions in `expressions.py`. Each level of precedence is handled by its own function.

### Precedence Table (highest to lowest)

| Level | Category       | Operators                        | Function        |                |
| ----- | -------------- | -------------------------------- | --------------- | -------------- |
| 1     | Unary          | `~`, `+`, `-`                    | `factor()`      |                |
| 2     | Multiplicative | `*`, `/`, `%`                    | `term()`        |                |
| 3     | Additive       | `+`, `-`                         | `add_sub()`     |                |
| 4     | Shift          | `<<`, `>>`                       | `shift()`       |                |
| 5     | Bitwise AND    | `&`                              | `bitwise_and()` |                |
| 6     | Bitwise XOR    | `^`                              | `bitwise_xor()` |                |
| 7     | Bitwise OR     | `\|`                             | `bitwise_or()` |
| 8     | Comparison     | `==`, `!=`, `<`, `>`, `<=`, `>=` | `comparison()`  |                |
| 9     | Logical AND    | `and`                            | `logical_and()` |                |
| 10    | Logical OR     | `or`                             | `logical_or()`  |                |

Each function wraps calls to lower-precedence functions, creating a tightly nested expression tree.


## Expression Constructs

`factor()` handles all atomic and prefix forms:

* **Literals**: strings, booleans, numbers → `('string', val, line)`
* **Identifiers**: variables or functions → `('ident', name, line)`
* **Lists**: `[1, 2, 3]` → `('list', [expr1, expr2, ...], line)`
* **Dictionaries**: `{a: 1, b: 2}` → `('dict', [(key, value), ...], line)`
* **Function calls**: `f(x, y)` → `('func_call', callee, args, line)`
* **Indexing**: `x[0]` → `('index', base, index_expr, line)`
* **Slicing**: `x[0:3]` → `('slice', base, start, end, line)`
* **Dot access**: `x.y` → `('dot', base, 'y', line)`
* **Unary ops**: `-x` → `('unary', op, expr, line)`


## Postfix Parsing

After parsing a base expression, `factor()` enters a loop that checks for additional postfix constructs:

```python
foo(1)[0].bar.baz()
```

is parsed as nested nodes:

```python
func_call(
  dot(
    dot(
      index(
        func_call('foo', [1]),
        '0'
      ),
      'bar'
    ),
    'baz'
  ),
  []
)
```

This allows chaining of calls, indexing, slicing, and attribute access.


## Ambiguity Resolution

To distinguish:

```omg
x := 5        # assignment
x(5)          # function call
x             # expression
```

The parser performs 1–2 token lookahead where needed:

* If `ID` is followed by `:=`, it's an assignment
* If `ID` is followed by `(`, it's a function call
* Otherwise it's a bare expression

This limited peeking keeps the parser mostly LL(1) while remaining robust.


## Error Reporting

All parser errors include:

* Token value and type
* Source line number
* File name

These are included in the raised `SyntaxError`, e.g.:

```
SyntaxError: Expected ':' after variable name on line 8 in foo.omg
```

The parser maintains mappings from token values to their symbolic types (e.g. `{` → `LBRACE`) for human-readable diagnostics.


## Extending the Parser

To add a new language feature:

1. **Define syntax**: identify how it should appear in source code
2. **Update the lexer** (if needed) to add new token types
3. **Add new AST forms**: choose a clear node shape
4. **Write parser logic** in `expressions.py` or `statements.py`
5. **Update the interpreter** to evaluate the new form

Parser changes must respect:

* Consistency of the tuple AST format
* Operator precedence hierarchy
* Scope of keywords and identifiers


## Example: Parsing a Function

The following function:

```omg
proc square(x) {
    return x * x
}
```

Produces the AST node:

```python
('func_def', 'square', ['x'],
    ('block', [
        ('return',
            ('mul',
                ('ident', 'x', 3),
                ('ident', 'x', 3),
                3
            ),
         3)
    ], 2),
2)
```

## Final Notes

* The parser is intentionally minimal and transparent, with no grammar generators.
* All precedence and associativity are handled explicitly in code.
* The parser assumes the lexer has stripped the `;;;omg` header and validated line numbers.

This makes the parser easy to step through, test, and extend as OMG evolves.
