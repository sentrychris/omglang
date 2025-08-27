# OMG Error Reference

This document lists common errors thrown by the OMG interpreter, along with example code that produces them.

Please note this list is not exhaustive, for all errors please see the original python interpreter.py implementation.

---

### VM Error Kinds

The native runtime uses a single `RAISE <kind>` instruction where `<kind>` maps to a specific `RuntimeError` variant. Supported kinds are:

- `Generic`
- `Syntax`
- `Type`
- `UndefinedIdent`
- `Value`
- `ModuleImport`

These cover the common error categories raised by the VM.

---

## 1. Missing Script Header

If the required `;;;omg` header is missing, the interpreter raises:

```sh
RuntimeError: OMG script missing required header ';;;omg' in .\scratchpad.omg
```

Example:

```sh
# Missing `;;;omg`
emit "Hello World"
```

---

## 2. Module and Variable Errors

### 2.1 Imported Modules Are Read-Only

```sh
TypeError: Imported modules are read-only
```

Example:

```sh
import "./examples/modules/math.omg" as math
math["add"] := 1
```

---

### 2.2 Wrong Number of Function Arguments

```sh
TypeError: Function expects 2 arguments add(1, 1, 1) on line 18 in .\scratchpad.omg
```

Example:

```php
proc test_add(a, b) {
    return a + b
}
emit test_add(1, 1, 1)
```

---

### 2.3 Undefined Variable

```sh
UndefinedVariableException: Undefined variable 'undefined_var' on line 21 in .\scratchpad.omg
```

Example:

```php
emit undefined_var
```

---

### 2.4 Invalid Assignment Operator

```sh
RuntimeError: Unexpected character = on line 21
```

Example:

```php
alloc i = 1   # Should use := instead of =
```

---

### 2.5 Undeclared Variable Assignment

```sh
UndefinedVariableException: Undefined variable 'undeclared_list' on line 27 in .\scratchpad.omg
```

Example:

```php
undeclared_list := [1,2,3]   # Missing alloc
```

---

### 2.6 List Index Out of Bounds

```sh
RuntimeError: List index out of bounds! test_list[5] On line 30 in .\scratchpad.omg
```

Example:

```php
alloc test_list := [1,2,3]
emit test_list[5]
```

---

### 2.7 Variable is not indexable

```sh
TypeError: i is not indexable! i[3] on line 51 in .\scratchpad.omg
```

Example:

```php
alloc i := 1
emit i[3]
```

---

### 2.8 Key not found in Dictionary

```sh
KeyError: "Key 'keyC' not found on line 8 in .\\scratchpad.omg"
```

Example:

```php
alloc dict := {
    keyA: 1,
    keyB: 2
}
emit dict.keyC
```

---

## 3. Built-in Function Errors

### 3.1 `hex()` Argument Errors

```sh
TypeError: hex() expects one integer argument! on line 34 in .\scratchpad.omg
```

Examples:

```php
hex()
hex(1,2)
```

---

### 3.2 `length()` Argument Errors

```sh
TypeError: length() expects one integer argument! on line 37 in .\scratchpad.omg
```

Examples:

```php
length()
length(1,2)
```

---

## 4. Syntax Errors

### 4.1 Unsupported Token

```sh
SyntaxError: Unexpected token / - (DIV) on line 41 in .\scratchpad.omg
```

Example:

```php
emit 5 // 2
```

---

### 4.2 Malformed Expression

Malformed expressions may parse incorrectly but should raise a syntax error.
Example (always evaluates to `3` incorrectly):

```php
emit 5 -+ 2
```

---

## 5. Runtime Arithmetic Errors

### 5.1 Division by Zero

```sh
ZeroDivisionError: integer division or modulo by zero
```

Example:

```php
emit 1 / 0
```