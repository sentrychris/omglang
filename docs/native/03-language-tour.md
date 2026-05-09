# 03 · Language tour

A whirlwind tour of OMG. If you've used Python or JS, most of it will feel
familiar — with a few quirks (declaration vs assignment, the `;;;omg` header,
keywords like `proc` and `facts`).

## The header

Every OMG file starts with:

```omg
;;;omg
```

It's required. The compiler refuses to read a file without it.

## Types

| Type    | Examples                            |
| ------- | ----------------------------------- |
| Int     | `42`, `-7`, `0xff`, `0b1010`        |
| Float   | `3.14`, `1.0e-3`, `nan`, `inf`      |
| String  | `"hello"`, `"line\n"`               |
| Bool    | `true`, `false`                     |
| None    | (no literal — `emit_no_value` etc.) |
| List    | `[1, 2, 3]`, `[]`, `["a", 5, true]` |
| Dict    | `{name: "Ada", age: 36}`            |
| Closure | from `proc` definitions             |

Lists and dicts are heterogeneous. Equality is structural (`[1,2] == [1,2]`).

## Variables

OMG distinguishes **declaration** (`alloc`) from **assignment** (`:=`).

```omg
alloc x := 5     # introduces a new binding
x := 6           # reassigns the existing binding
y := 7           # ERROR: y is not declared
```

This catches typos that would otherwise silently shadow:

```omg
alloc count := 0
loop count < 10 {
    cont := count + 1   # typo — would have been a silent bug
    count := count + 1
}
```

Use `alloc` once per name per scope. Re-declaring inside a loop body is fine
(each iteration is a fresh scope).

## Output

```omg
emit "hello"        # prints, with newline
emit 42 + 8         # 50
emit [1, 2, 3]      # [1, 2, 3]
```

`emit` is a statement, not a function. It always takes one expression.

## Control flow

```omg
if x > 0 {
    emit "positive"
} elif x < 0 {
    emit "negative"
} else {
    emit "zero"
}

loop x < 100 {
    x := x + 1
    if x == 50 { break }
}
```

There's no `for`. Use `loop` with a counter. There's no `while` either —
`loop` is the while.

## Functions

```omg
proc add(a, b) {
    return a + b
}

proc make_adder(n) {
    proc add_n(x) {       # nested procs capture from enclosing scope
        return x + n
    }
    return add_n
}

alloc add5 := make_adder(5)
emit add5(7)              # 12
```

Procs can be passed around as values, returned, called via the captured
result, etc. Closures work as you'd expect.

Tail calls inside `proc` are optimized — recursion of any depth doesn't
blow the stack:

```omg
proc count_down(n) {
    if n == 0 { return "done" }
    return count_down(n - 1)   # tail call — no stack growth
}
emit count_down(1000000)       # works
```

## Strings

```omg
alloc s := "hello"
emit length(s)             # 5
emit s[0]                  # h
emit s[1:4]                # ell
emit s + " world"          # hello world
emit chr(65)               # A
emit ascii("A")            # 65
```

Strings are UTF-8. Indexing and slicing operate on **code points**, not bytes:

```omg
alloc s := "héllo"
emit length(s)             # 5 (not 6)
emit s[1]                  # é
```

For raw byte access use `string_bytes(s)` and `bytes_to_string(list)`.

## Lists

```omg
alloc xs := [1, 2, 3]
emit length(xs)            # 3
emit xs[0]                 # 1
emit xs[-1]                # 3 (negative indices count from end)
emit xs[0:2]               # [1, 2]
emit xs + [4, 5]           # [1, 2, 3, 4, 5] (fresh list)
xs[0] := 99                # in-place mutation
emit xs                    # [99, 2, 3]
```

## Dicts

```omg
alloc d := {name: "Ada", age: 36}
emit d.name                # Ada
emit d["age"]              # 36
d.email := "ada@example.com"
emit dict_keys(d)          # [name, age, email]
```

`d.x` and `d["x"]` are interchangeable; the key is always a string.

## Try / except

```omg
try {
    alloc result := 10 / 0
} except err {
    emit "caught: " + err   # caught: ZeroDivisionError: integer division or modulo by zero
}
```

`err` is a string with the formatted error: `"<Kind>: <message>"`.

The error kinds you'll see: `RuntimeError`, `TypeError`, `ValueError`,
`IndexError`, `KeyError`, `ZeroDivisionError`, `AssertionError`,
`UndefinedIdentError`, `ModuleImportError`, `FrozenWriteError`.

## Errors

```omg
panic("something went wrong")         # → RuntimeError: something went wrong
raise("bad input")                    # same as panic, but conventionally for user errors
facts x > 0                           # like `assert x > 0` — fails as AssertionError
```

`exit_with_error("msg")` prints to stderr without a kind prefix and exits 1.
Used by `vm.omg` itself; you probably won't need it.

## Imports

```omg
import "./modules/math.omg" as math

emit math.is_prime(97)                # true
```

The imported file's top-level bindings become attributes of the alias.
Imports are resolved at compile time and inlined into the bytecode.

## Built-in functions

The full list, grouped by area. Argument counts in parens.

**Strings & chars**
`length(x)`, `chr(i)`, `ascii(s)`, `string_bytes(s)`, `bytes_to_string(list)`

**Numeric**
`int(x)`, `float(x)`, `floor(x)`, `ceil(x)`, `round(x)`, `abs(x)`, `pow(a,b)`,
`sqrt(x)`, `log(x)`, `sin(x)`, `cos(x)`, `tan(x)`

**Formatting**
`hex(i)`, `binary(i)` or `binary(i, width)`, `float_bits(s)`, `bits_to_float(i)`

**Collections**
`freeze(d)` (makes a dict immutable), `dict_keys(d)`

**File I/O**
`read_file(path)` (returns string),
`file_open(path, mode)` (returns int handle), `file_read(h)`, `file_write(h, data)`,
`file_close(h)`,
`file_exists(p)`, `is_dir(p)`, `read_dir(p)` (sorted list of names),
`make_dir(p)` (mkdir -p)

**Errors**
`panic(msg)`, `raise(msg)`, `exit_with_error(msg)`

**Reflection**
`call_builtin(name, args_list)` — dispatches to another builtin. Used by
the OMG-in-OMG interpreter; rarely useful in user code.

## CLI args

```omg
;;;omg
emit "args: " + args
emit "first: " + args[0]   # the script path or binary
emit "count: " + length(args)
```

When run via `omg foo.omg a b c`, `args` is `[foo.omg, a, b, c]`.
When run as a native binary `./foo a b c`, `args` is `[./foo, a, b, c]`.
`args[0]` is your "argv[0]"; user-supplied args start at `args[1]`.

## Two more globals

| Global         | What it is                                       |
| -------------- | ------------------------------------------------ |
| `module_file`  | path to the running script (or binary)           |
| `current_dir`  | working directory at startup; consulted by `read_file`, `file_open`, etc. for relative paths. Mutating it changes path resolution. |

## Idioms

### Read a file as lines

```omg
alloc src := read_file("data.txt")
alloc lines := []
alloc start := 0
alloc i := 0
loop i < length(src) {
    if src[i] == "\n" {
        lines := lines + [src[start:i]]
        start := i + 1
    }
    i := i + 1
}
if start < length(src) {
    lines := lines + [src[start:]]
}
```

### Sort a list (insertion sort, in-place)

```omg
proc sort(xs) {
    alloc i := 1
    loop i < length(xs) {
        alloc j := i
        loop j > 0 and xs[j] < xs[j - 1] {
            alloc t := xs[j]
            xs[j] := xs[j - 1]
            xs[j - 1] := t
            j := j - 1
        }
        i := i + 1
    }
}
```

### Map (no built-in `map()` — write the loop)

```omg
proc map(f, xs) {
    alloc out := []
    alloc i := 0
    loop i < length(xs) {
        out := out + [f(xs[i])]
        i := i + 1
    }
    return out
}
```

## Read next

- [04-pipeline.md](04-pipeline.md) — what happens between `.omg` and `./foo`
- [examples/](../../examples/) — 19 working programs of varying size
