# OMG

[![CodeQL](https://github.com/sentrychris/omglang/actions/workflows/github-code-scanning/codeql/badge.svg)](https://github.com/sentrychris/omglang/actions/workflows/github-code-scanning/codeql)
[![CI](https://github.com/sentrychris/omglang/actions/workflows/ci.yml/badge.svg)](https://github.com/sentrychris/omglang/actions/workflows/ci.yml)

A small programming language made for fun and for learning how
languages get put together end-to-end.

OMG has the usual stuff every language has: variables, math, strings, lists,
dictionaries, conditionals, loops, functions, files etc. There's nothing flashy 
or industrial about it. It exists so I could see what it takes to build a working programming language, all the way up to writing OMG's own compiler **in OMG**.

## Contents

- [Get it running](#get-it-running)
  - [Hello, world](#hello-world)
- [A tour of the language](#a-tour-of-the-language)
  - [Variables](#variables)
  - [Numbers and math](#numbers-and-math)
  - [Floats](#floats)
  - [Strings](#strings)
  - [Booleans and comparisons](#booleans-and-comparisons)
  - [Conditionals](#conditionals)
  - [Loops](#loops)
  - [Lists](#lists)
  - [Dictionaries](#dictionaries)
  - [Functions](#functions)
  - [Multi-file programs: `import`](#multi-file-programs-import)
  - [Files](#files)
  - [Errors and `try` / `except`](#errors-and-try--except)
- [Built-in functions](#built-in-functions)
- [The REPL](#the-repl)
- [Editor support (VS Code)](#editor-support-vs-code)
- [What's in this repo](#whats-in-this-repo)
- [A small piece of trivia](#a-small-piece-of-trivia)
- [More reading](#more-reading)
- [License](#license)

---

## Get it running

You need [Rust](https://rustup.rs/) installed. Then:

```sh
git clone https://github.com/sentrychris/omglang
cd omglang
cargo build --release --manifest-path runtime/Cargo.toml
```

That builds the OMG runtime as `runtime/target/release/omg`. From here
on this README assumes you have it on your `PATH` as `omg`. Either
copy/symlink the binary somewhere on your `PATH`, or alias it:

```sh
alias omg=$(pwd)/runtime/target/release/omg
```

### Hello, world

Make a file called `hello.omg`:

```omg
;;;omg

emit "Hello, world!"
```

Run it:

```sh
omg hello.omg
```

You should see:

```
Hello, world!
```

A few things to know up front:

- Every `.omg` file **must** start with `;;;omg` on its first non-empty
  line. That's the "this is an OMG program" marker. If you forget it, the
  compiler will refuse it.
- `emit` prints something to the screen.
- `#` starts a comment. Comments run to the end of the line.

---

## A tour of the language

Each section is a few lines you can paste into a `.omg` file (don't
forget the `;;;omg` header) and run with `omg yourfile.omg`.

### Variables

You introduce a brand-new variable with `alloc`. You change an existing
one with `:=`:

```omg
;;;omg

alloc name := "Chris"        # introduce a new variable
emit name                    # → Chris

name := "Bob"                # update it (no `alloc` the second time)
emit name                    # → Bob
```

Forget the `alloc` for a brand-new name and OMG complains:

```
UndefinedIdentError: name
```

The full rules — they're worth reading once because they save you from
typos and accidental shadowing:

1. **Every new binding needs `alloc`.** Bare `:=` only works on names
   that already exist somewhere in scope. `cont := 5` when you meant
   `count := 5` is an error, not a silent new variable.
2. **Re-assignment uses `:=`** — no `alloc` the second time:
   ```omg
   alloc x := 1
   x := 2          # fine
   ```
3. **Bindings declared inside a function are local to that function.**
   They aren't visible outside:
   ```omg
   proc round2(n) { alloc r := 2 }
   round2(0)
   emit r          # UndefinedIdentError: r
   ```
4. **Globals can be read *and updated* from inside a function** with
   plain `:=`:
   ```omg
   alloc r := 1
   proc bump() { r := r + 1 }
   bump()
   emit r          # → 2
   ```
   If you instead want a fresh local that *shadows* an outer name, use
   `alloc` again inside the function — `alloc r := 2` would create a
   new local without touching the global.

### Numbers and math

OMG has two kinds of numbers: integers (whole numbers) and floats
(decimals). Plain integer math returns integers:

```omg
;;;omg

emit 1 + 2          # 3
emit 10 - 3         # 7
emit 4 * 5          # 20
emit 10 / 3         # 3      ← integer division when both sides are ints
emit 10 % 3         # 1      ← remainder ("modulo")
emit -7 / 2         # -4     ← rounds toward minus infinity
```

Bitwise operators exist (integers only):

```omg
;;;omg

emit 6 & 3          # 2      bitwise AND
emit 6 | 3          # 7      bitwise OR
emit 6 ^ 3          # 5      bitwise XOR
emit ~1             # -2     bitwise NOT
emit 1 << 3         # 8      left shift
emit 16 >> 2        # 4      right shift
emit 0b1010         # 10     binary literal
```

### Floats

Write a float by including a decimal point or an exponent:

```omg
;;;omg

emit 1.5            # 1.5
emit 2.0 + 3.5      # 5.5
emit 1.0e3          # 1000.0  (scientific notation)
emit 6.022e23       # 6.022e23
```

`/` returns a float as soon as either operand is a float (true division);
between two integers it stays as integer floor division. Use `//` when
you want explicit floor division regardless of type:

```omg
;;;omg

emit 10 / 3         # 3        ← int / int → int
emit 10 / 3.0       # 3.3333…  ← any float → true division
emit 10 // 3        # 3        ← explicit floor division
emit 10.5 // 3      # 3.0      ← floor div on float still rounds toward -∞
```

Other things to know:

- `5 == 5.0` is `true`. Cross-type numeric comparisons compare values.
- Bitwise operators (`&`, `|`, `^`, `~`, `<<`, `>>`) reject floats with a
  TypeError. Same for indexing a list with a float (`xs[1.5]`).
- Float math is IEEE-754 double precision, so `0.1 + 0.2 == 0.3` is
  `false`. That's not an OMG bug — it's how floats work everywhere.
- `int(x)` truncates toward zero; `float(x)` widens an int to a float.

The standard math kit is built in: `floor`, `ceil`, `round` (banker's
rounding), `abs`, `sqrt`, `pow`, `log` (natural), `sin`, `cos`, `tan`.
See the [built-ins table](#built-in-functions) for the full list.

### Strings

Strings are written between double quotes:

```omg
;;;omg

alloc greeting := "Hello"
alloc name := "world"
emit greeting + ", " + name + "!"   # Hello, world!

emit length(greeting)               # 5
emit greeting[0]                    # H
emit greeting[1:4]                  # ell    ← slice from 1 up to (not including) 4
emit greeting[-1]                   # o      ← negative index counts from the end
```

Inside a string these escapes work: `\n` (newline), `\t` (tab), `\r`,
`\\`, `\"`, `\0`.

### Booleans and comparisons

```omg
;;;omg

emit 5 > 3              # true
emit 5 == 5             # true
emit 5 != 4             # true
emit "abc" < "abd"      # true     ← strings compare alphabetically
emit true and false     # false
emit true or false      # true
```

Things considered **falsy** (treated as false in `if` and `loop`):

- `false`
- `0` (the integer zero)
- `""` (the empty string)
- `[]` (the empty list)
- `{}` (the empty dictionary)

Everything else is truthy.

### Conditionals

```omg
;;;omg

alloc score := 75

if score >= 90 {
    emit "A"
} elif score >= 80 {
    emit "B"
} elif score >= 70 {
    emit "C"
} else {
    emit "F"
}
```

`elif` is short for "else if". You can have any number of `elif`
branches, and `else` is optional.

### Loops

OMG has one looping construct: `loop <condition> { ... }`. It keeps
running the body as long as the condition is truthy. Same idea as a
`while` loop in other languages:

```omg
;;;omg

alloc i := 0
loop i < 5 {
    emit i
    i := i + 1
}
```

Output:

```
0
1
2
3
4
```

`break` exits the innermost loop early:

```omg
;;;omg

alloc n := 1
loop true {
    if n > 100 {
        break
    }
    n := n * 2
}
emit n              # 128: the first power of 2 greater than 100
```

There's no `for` loop. Use `loop` with a counter, like the example above.

### Lists

```omg
;;;omg

alloc xs := [10, 20, 30]
emit xs                 # [10, 20, 30]
emit length(xs)         # 3
emit xs[0]              # 10
emit xs[-1]             # 30          ← negative indices count from the end
emit xs[1:3]            # [20, 30]    ← slicing also works on lists

xs[0] := 99             # replace one element
emit xs                 # [99, 20, 30]

xs := xs + [40]         # append (creates a new list)
emit xs                 # [99, 20, 30, 40]
```

Two variables holding the "same" list share it, changing one is
visible through the other:

```omg
;;;omg

alloc a := [1, 2, 3]
alloc b := a
b[0] := 99
emit a              # [99, 2, 3]: `a` and `b` point at the same list
```

### Dictionaries

A dictionary maps keys to values. Keys are strings. You can read or
write entries either with dot notation (`d.name`) or with brackets
(`d["name"]`):

```omg
;;;omg

alloc person := {name: "Chris", age: 32}

emit person.name        # Chris
emit person["age"]      # 32

person.age := 33                # change a value
person["job"] := "engineer"     # add a new key

emit person                     # {name: Chris, age: 33, job: engineer}
emit length(person)             # 3
```

### Functions

`proc` (short for "procedure") defines a function. `return` returns a
value:

```omg
;;;omg

proc square(x) {
    return x * x
}

emit square(4)          # 16
emit square(7)          # 49
```

Functions are **first class**, you can pass them as arguments, store
them in variables, and return them from other functions:

```omg
;;;omg

proc apply_twice(f, x) {
    return f(f(x))
}

proc inc(n) {
    return n + 1
}

emit apply_twice(inc, 5)        # 7   (5 → 6 → 7)
```

A function defined inside another function remembers the variables it
saw at the time it was defined. That's called a **closure**:

```omg
;;;omg

proc make_adder(n) {
    proc add(x) {
        return x + n        # `n` was captured from `make_adder`
    }
    return add
}

alloc add5 := make_adder(5)
alloc add100 := make_adder(100)

emit add5(10)           # 15
emit add100(7)          # 107
```

Each call to `make_adder` produces its own `add` that remembers its
own `n`.

### Multi-file programs: `import`

Save this as `mathlib.omg`:

```omg
;;;omg

proc square(x) {
    return x * x
}

proc cube(x) {
    return x * x * x
}
```

Then in another file in the same folder:

```omg
;;;omg

import "mathlib.omg" as math

emit math.square(5)     # 25
emit math.cube(3)       # 27
```

The path inside the quotes is **relative to the importing file's
directory**. The name after `as` is what you'll call it from the other
file. Imports run the imported file once, and capture its top-level
`proc` and `alloc` definitions under that name.

### Files

Read a file as text in one shot:

```omg
;;;omg

alloc text := read_file("notes.txt")
if text == false {
    emit "couldn't read notes.txt"
} else {
    emit "got " + length(text) + " characters"
}
```

Or open a handle for finer control:

```omg
;;;omg

# write text to a file
alloc h := file_open("greeting.txt", "w")
file_write(h, "hi from OMG\n")
file_close(h)

# and read it back
alloc h2 := file_open("greeting.txt", "r")
emit file_read(h2)              # hi from OMG
file_close(h2)
```

Binary mode (`"rb"`, `"wb"`, `"ab"`) reads and writes lists of bytes
(integers 0–255):

```omg
;;;omg

alloc h := file_open("photo.jpg", "rb")
alloc bytes := file_read(h)
file_close(h)

emit length(bytes)              # how many bytes long the file is
emit bytes[0]                   # the first byte, as an integer
```

Relative paths in `read_file` and `file_open` are resolved against
**your shell's current directory**, the same way `cat`, `wc`,
`python`, etc. behave.

### Errors and `try` / `except`

Things sometimes go wrong. A bad index or a missing key, dividing by zero... By
default the program stops with an error message. To recover, wrap the
risky bit in `try` / `except`:

```omg
;;;omg

try {
    alloc xs := [1, 2, 3]
    emit xs[99]                 # IndexError: index 99 out of range
} except err {
    emit "oops: " + err
}
emit "still running"
```

You can raise your own errors with `panic`:

```omg
;;;omg

proc divide(a, b) {
    if b == 0 {
        panic("division by zero")
    }
    return a / b
}

try {
    emit divide(10, 0)
} except err {
    emit "caught: " + err       # caught: RuntimeError: division by zero
}
```

`facts` is shorthand for "assert this is true; if not, error out". It's
useful inside tests:

```omg
;;;omg

alloc x := 1 + 1
facts x == 2            # silent (passes)
facts x == 3            # AssertionError: assertion failed
```

---

## Built-in functions

Always available, no import needed.

| Function                       | What it does                                          |
| ------------------------------ | ----------------------------------------------------- |
| `length(x)`                    | length of a list or string                            |
| `chr(n)`                       | one-character string for byte value `n`               |
| `ascii(c)`                     | code point of a one-character string `c`              |
| `hex(n)`                       | lowercase hex string for integer `n`                  |
| `binary(n)` / `binary(n, w)`   | binary string for `n`, optionally `w` bits wide       |
| `freeze(d)`                    | turn a dict into a read-only one                      |
| `panic(msg)` / `raise(msg)`    | raise a runtime error (catchable with `try`/`except`) |
| `read_file(path)`              | read a text file in one shot, or `false` on error     |
| `file_exists(path)`            | does the file exist?                                  |
| `file_open(path, mode)`        | open and return a handle (`r`, `rb`, `w`, `wb`, `a`, `ab`) |
| `file_read(handle)`            | read everything remaining from a handle               |
| `file_write(handle, data)`     | write to a handle                                     |
| `file_close(handle)`           | close a handle                                        |
| `string_bytes(s)`              | UTF-8 byte values of `s` as a list of integers        |
| `int(x)` / `float(x)`          | convert between int and float (or parse from string)  |
| `floor(x)` / `ceil(x)`         | round a float toward `-∞` / `+∞`, returns int         |
| `round(x)`                     | round-half-to-even (banker's rounding), returns int   |
| `abs(x)`                       | absolute value (preserves int/float type)             |
| `sqrt(x)`                      | square root, returns float                            |
| `pow(a, b)`                    | `a` to the power of `b` (int^int stays int)           |
| `log(x)`                       | natural log, returns float                            |
| `sin(x)` / `cos(x)` / `tan(x)` | trigonometry in radians, returns float                |
| `call_builtin(name, args)`     | call a builtin by name (advanced)                     |

The runtime also hands you three special globals every program can read:

- `args`: a list of strings: the command-line arguments. `args[0]` is
  the script's path; `args[1]`, `args[2]` … are user-supplied arguments.
- `module_file`: the path of the running script.
- `current_dir`: the directory the user ran `omg` from.

---

## The REPL

Run `omg` with no arguments to drop into an interactive shell:

```
$ omg
OMG Language Interpreter - REPL
Type `exit` or `quit` to leave.
>>> alloc x := 21
>>> emit x * 2
42
>>> proc greet(name) {
...     return "Hello " + name
... }
>>> emit greet("OMG")
Hello OMG
>>> quit
```

Variables, functions, and imports persist across lines until you `quit`.

---

## Editor support (VS Code)

The [`vscode/`](vscode/) directory holds a VS Code extension that adds:

- syntax highlighting for `.omg` files (a TextMate grammar in
  `vscode/syntaxes/`),
- file icons for `.omg` source and `.omgb` bytecode,
- a small **language server** (LSP) under `vscode/server/` that powers
  autocompletion, hover info, and go-to-definition on built-ins and
  user-defined `proc`s/`alloc`s in the open file.

The extension isn't published on the marketplace, you build it from
this repo and install the resulting `.vsix` directly:

```sh
cd vscode
npm install                                             # first time only, pulls dependencies
npm run compile                                         # compile the TypeScript client + server
npx vsce package -o omg-language-server.vsix           # package into a .vsix bundle
code --install-extension omg-language-server.vsix --force
```

What each step does:

- `npm install`: downloads the dev dependencies (TypeScript compiler,
  `vsce` packager, the VS Code LSP libraries). Only needed the first
  time, or after the dependency list changes.
- `npm run compile`: runs `tsc` on the client (the bit that loads into
  VS Code) and the server (the bit that answers LSP requests). Outputs
  go to `client/out/` and `server/out/`.
- `npx vsce package`: bundles the compiled output, the grammar, the
  icons, and `package.json` into a single installable `.vsix` archive.
- `code --install-extension … --force`: installs that bundle into your
  local VS Code, replacing any previous version of the extension.

After installing, open any `.omg` file and you should see syntax
highlighting and the OMG file icon. Start typing `proc`, `loop`,
`emit`, etc. to see completions.

---

## What's in this repo

```
omglang/
├── runtime/         the Rust implementation: lexer, parser, compiler, VM, REPL
├── bootstrap/
│   ├── compiler.omg   the OMG compiler, written in OMG
│   └── compiler.omgb  its compiled bytecode (re-built on `cargo build`)
├── examples/        small standalone programs
├── tools/           command-line utilities written in OMG (wc, grep, sort, etc.)
├── reference/       the legacy Python implementation + a tree-walk OMG-in-OMG interpreter
└── vscode/          VS Code extension (syntax highlighting + LSP completion)
```

Some interesting starting points:

- [`examples/prime_sieve.omg`](examples/prime_sieve.omg): finds primes up to 100.
- [`examples/maze_solver.omg`](examples/maze_solver.omg): breadth-first search over a grid.
- [`examples/higher_order.omg`](examples/higher_order.omg): closures + first-class functions.
- [`tools/wc.omg`](tools/wc.omg), [`tools/grep.omg`](tools/grep.omg),
  [`tools/json.omg`](tools/json.omg): Unix-style utilities,
  written in OMG. See [`tools/README.md`](tools/README.md) for the
  full list.

---

## A small piece of trivia

OMG's compiler is itself written in OMG. The file
[`bootstrap/compiler.omg`](bootstrap/compiler.omg) reads OMG source
code and produces the bytecode the runtime executes, which is exactly
what the Rust frontend in `runtime/` does.

By default, `omg <script>` runs your code through that OMG-written
compiler — the language compiles itself end-to-end on every run. If you
want the faster Rust frontend (e.g. while iterating, or to avoid the
~1 second compile overhead on larger programs), pass `--rust`:

```sh
omg foo.omg              # self-hosted (default)
omg --rust foo.omg       # Rust frontend
```

To verify both compilers agree byte-for-byte on the compiler's own
source — the fixed-point check — run:

```sh
omg --verify-self-hosted bootstrap/compiler.omg
```

The runtime compiles `compiler.omg` two different ways, once with the
Rust frontend, once with the OMG-written compiler running on the VM,
and confirms the two byte streams are identical.

---

## More reading

- [`docs/compilation-pipeline.md`](docs/compilation-pipeline.md): how
  `omg foo.omg` actually runs your script — the two-stage compiler, the
  VM-on-VM dance, what `--rust` does, and the fixed-point check.
- [`runtime/README.md`](runtime/README.md): runtime architecture and CLI flags.
- [`tools/README.md`](tools/README.md): the OMG-in-OMG tools.
- [`vscode/README.md`](vscode/README.md): VS Code extension.

---

## License

MIT.

Educational project, not intended for production use.
