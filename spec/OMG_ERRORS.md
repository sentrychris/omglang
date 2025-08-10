# OMG Error Reference

This document lists common errors thrown by the OMG interpreter, along with example code that produces them.

Please note this list is not exhaustive, for all errors please see the original python interpreter.py implementation.

---

## 1. Missing Script Header

If the required `;;;omg` header is missing, the interpreter raises:

```
RuntimeError: OMG script missing required header ';;;omg' in .\scratchpad.omg
```

Example:

```omg
# Missing `;;;omg`
emit "Hello World"
```

---

## 2. Module and Variable Errors

### 2.1 Imported Modules Are Read-Only

```
TypeError: Imported modules are read-only
```

Example:

```omg
import "./examples/modules/math.omg" as math
math["add"] := 1
```

---

### 2.2 Wrong Number of Function Arguments

```
TypeError: Function expects 2 arguments add(1, 1, 1) on line 18 in .\scratchpad.omg
```

Example:

```omg
proc test_add(a, b) {
    return a + b
}
emit test_add(1, 1, 1)
```

---

### 2.3 Undefined Variable

```
UndefinedVariableException: Undefined variable 'undefined_var' on line 21 in .\scratchpad.omg
```

Example:

```omg
emit undefined_var
```

---

### 2.4 Invalid Assignment Operator

```
RuntimeError: Unexpected character = on line 21
```

Example:

```omg
alloc i = 1   # Should use := instead of =
```

---

### 2.5 Undeclared Variable Assignment

```
UndefinedVariableException: Undefined variable 'undeclared_list' on line 27 in .\scratchpad.omg
```

Example:

```omg
undeclared_list := [1,2,3]   # Missing alloc
```

---

### 2.6 List Index Out of Bounds

```
RuntimeError: List index out of bounds! test_list[5] On line 30 in .\scratchpad.omg
```

Example:

```omg
alloc test_list := [1,2,3]
emit test_list[5]
```

---

### 2.7 Variable is not indexable

```
TypeError: i is not indexable! i[3] on line 51 in .\scratchpad.omg
```

Example:

```omg
alloc i := 1
emit i[3]
```

---

### 2.8 Key not found in Dictionary

```
KeyError: "Key 'keyC' not found on line 8 in .\\scratchpad.omg"
```

Example:

```omg
alloc dict := {
    keyA: 1,
    keyB: 2
}
emit dict.keyC
```

---

## 3. Built-in Function Errors

### 3.1 `hex()` Argument Errors

```
TypeError: hex() expects one integer argument! on line 34 in .\scratchpad.omg
```

Examples:

```omg
hex()
hex(1,2)
```

---

### 3.2 `length()` Argument Errors

```
TypeError: length() expects one integer argument! on line 37 in .\scratchpad.omg
```

Examples:

```omg
length()
length(1,2)
```

---

## 4. Syntax Errors

### 4.1 Unsupported Token

```
SyntaxError: Unexpected token / - (DIV) on line 41 in .\scratchpad.omg
```

Example:

```omg
emit 5 // 2
```

---

### 4.2 Malformed Expression

Malformed expressions may parse incorrectly but should raise a syntax error.
Example (always evaluates to `3` incorrectly):

```omg
emit 5 -+ 2
```

---

## 5. Runtime Arithmetic Errors

### 5.1 Division by Zero

```
ZeroDivisionError: integer division or modulo by zero
```

Example:

```omg
emit 1 / 0
```