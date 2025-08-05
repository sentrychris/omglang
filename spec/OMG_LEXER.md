# OMG Lexical Analysis

This document describes the lexical analysis stage of the OMG language toolchain. The lexer converts raw source code into a sequence of typed tokens suitable for parsing.

## Overview

The OMG lexer is implemented in `core/lexer.py`. It performs a **single-pass scan** over the source code using a **combined regular expression** with named groups. Each match is transformed into a `Token(type, value, line)` object.

Key features:
- Skips whitespace and comments
- Supports numeric, string, and boolean literals
- Distinguishes keywords, identifiers, and operators
- Strips the `;;;omg` header and tracks line numbers for accurate diagnostics
- Produces a token stream and a literal-to-type mapping for human-readable error reporting

## Header Handling

All OMG scripts must begin with the header:

```omg
;;;omg
````

This header is stripped during lexing. Line numbering begins at `2` to reflect the first line of actual code.

If the header is missing, the interpreter will raise an error during execution (not during lexing).

## Token Format

Each token is an instance of the `Token` class:

```python
class Token:
    def __init__(self, type_: str, value: Any, line: int)
```

Attributes:

* `type_`: symbolic name of the token (e.g. `NUMBER`, `PLUS`, `IF`)
* `value`: decoded literal value or matched symbol
* `line`: line number from original source

Example:

```python
Token('NUMBER', 42, line=3)
Token('PLUS', '+', line=3)
Token('ID', 'x', line=3)
```

## Token Specification

Tokens are defined by ordered regex patterns grouped by category:

### Literals

| Type             | Pattern          | Example         |
| ---------------- | ---------------- | --------------- |
| `BINARY`         | `0b[01]+`        | `0b1010` → `10` |
| `NUMBER`         | `\d+`            | `42`            |
| `STRING`         | `"..."`          | `"hi\nthere"`   |
| `TRUE` / `FALSE` | `true` / `false` | Booleans        |

String literals support Python-style escape sequences via `unicode_escape`.

### Keywords

Recognized with word boundaries (`\b`) to avoid false positives:

* `if`, `elif`, `else`
* `loop`, `break`
* `alloc`, `emit`, `facts`, `proc`, `return`
* `and`, `or`

### Identifiers

| Type | Pattern             | Examples          |
| ---- | ------------------- | ----------------- |
| `ID` | `[A-Za-z_][\w\d_]*` | `my_var`, `_tmp1` |

Used for variable and function names. Keywords take precedence over identifiers in the regex ordering.

### Operators

| Category   | Tokens                           |                           |
| ---------- | -------------------------------- | ------------------------- |
| Arithmetic | `+`, `-`, `*`, `/`, `%`          |                           |
| Comparison | `==`, `!=`, `<`, `>`, `<=`, `>=` |                           |
| Bitwise    | `&`, \`                          | `, `^`, `\~`, `<<`, `>>\` |
| Assignment | `:=`                             |                           |
| Logical    | `and`, `or`                      |                           |

### Delimiters & Punctuation

| Token      | Symbol |
| ---------- | ------ |
| `LBRACE`   | `{`    |
| `RBRACE`   | `}`    |
| `LPAREN`   | `(`    |
| `RPAREN`   | `)`    |
| `LBRACKET` | `[`    |
| `RBRACKET` | `]`    |
| `COMMA`    | `,`    |
| `COLON`    | `:`    |
| `DOT`      | `.`    |

## Special Tokens

* `COMMENT`: `# ...` (skipped entirely)
* `SKIP`: spaces and tabs (ignored)
* `NEWLINE`: triggers line number increment and emits a `NEWLINE` token
* `MISMATCH`: catches unexpected characters and raises a `RuntimeError`


## Output

The `tokenize()` function returns:

```python
tokens, token_map = tokenize(code: str)
```

* `tokens`: list of `Token` instances, ending with an `EOF` marker
* `token_map`: maps literal strings (e.g., `'{'`, `'=='`) to their token types (e.g., `LBRACE`, `EQ`)

Example:

```python
Token('IF', 'if', 2)
Token('ID', 'x', 2)
Token('GT', '>', 2)
Token('NUMBER', 10, 2)
Token('LBRACE', '{', 2)
```

## Error Handling

If an unexpected character is encountered (does not match any token pattern), the lexer raises:

```python
RuntimeError: Unexpected character '$' on line 3
```

The parser also uses `token_map` to reverse-map token types back to symbols, enabling messages like:

```
Expected `}` but found `]` on line 10
```

## Example

Input script:

```omg
;;;omg
alloc x := 10
emit x + 5
```

Yields tokens:

```
Token(ALLOC, alloc, line=2)
Token(ID, x, line=2)
Token(ASSIGN, :=, line=2)
Token(NUMBER, 10, line=2)
Token(EMIT, emit, line=3)
Token(ID, x, line=3)
Token(PLUS, +, line=3)
Token(NUMBER, 5, line=3)
Token(EOF, None, line=4)
```

## Extending the Lexer

To add a new token:

1. Insert a `(name, pattern)` entry into `token_specification` in the appropriate order.
2. Ensure it's above any patterns it might conflict with.
3. Optionally add to `token_map_literals` if the literal will appear in diagnostics.

For example, to add a `not` keyword:

```python
('NOT', r'\bnot\b')
```

## Notes

* Line numbers start at 2 to account for the stripped header.
* Token types are case-sensitive and consistent with parser expectations.
* Strings are decoded using Python's `unicode_escape` to support escape sequences.

## Summary

The OMG lexer is designed for clarity, precision, and extensibility. It bridges human-readable source code and parser-friendly token streams while retaining detailed positional information for downstream error reporting.