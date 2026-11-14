# OMG

[![CodeQL](https://github.com/sentrychris/omglang/actions/workflows/codeql.yml/badge.svg)](https://github.com/sentrychris/omglang/actions/workflows/codeql.yml)
[![CI](https://github.com/sentrychris/omglang/actions/workflows/ci.yml/badge.svg)](https://github.com/sentrychris/omglang/actions/workflows/ci.yml)

OMG is a small programming language made for fun and for learning how
languages get put together end-to-end.

OMG has the usual stuff every language has: variables, math, strings, lists,
dictionaries, conditionals, loops, functions, files etc. There's nothing flashy 
or industrial about it. It exists so I could see what it takes to build a working programming language, all the way up to writing OMG's own compiler **in OMG**.

You can try it without installing anything: the
[playground](https://versyx.digital/omglang/playground/) runs OMG in
your browser, and shows what the compiler makes of your program at
every stage.

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
- [Compile to a native binary](#compile-to-a-native-binary)
- [Editor support (VS Code)](#editor-support-vs-code)
- [What's in this repo](#whats-in-this-repo)
- [A small piece of trivia](#a-small-piece-of-trivia)
- [More reading](#more-reading)
- [License](#license)

---

## Get it running

On Linux and Windows (x86-64) you need nothing installed. `omg` is
checked into the repository, and it is the whole toolchain:

```sh
git clone https://github.com/sentrychris/omglang
cd omglang
bin/omg examples/hello_world.omg        # Windows: bin\omg.exe examples\hello_world.omg
```

From here on this README assumes you have it on your `PATH` as `omg`.
The simplest way is an alias:

```sh
alias omg=$(pwd)/bin/omg
```

```powershell
# Windows (PowerShell)
Set-Alias omg "$PWD\bin\omg.exe"
```

`omg` is written in OMG and builds itself. It runs programs, turns
them into executables and rebuilds itself from its own source, with no
C compiler, no linker and no other language involved.

### Platform support

|                                              | Linux x86-64 | Windows x86-64 | macOS, Linux on ARM |
| -------------------------------------------- | :---: | :---: | :-----: |
| `omg` ready to run after a clone             | ✓ (`bin/omg`) | ✓ (`bin\omg.exe`) | build it once, see below |
| Run programs, REPL, `--compile`, `--emit-c`, `--emit-js` | ✓ | ✓ | ✓ |
| `omg --build` (make an executable)           | ✓, nothing needed | ✓, nothing needed | ✓, through a C compiler |
| Build for the other system (`--target`)      | ✓ | ✓ | ✓ |
| Raw-mode keyboard input (`stdin_set_raw`, `stdin_read_key`), so the games run | ✓ | ✓ | ✓ |
| Pseudo-terminals and `fork` (`pty_*`, `fork`) | ✓ | raise a catchable `ValueError` | ✓ |

OMG writes x86-64 machine code for Linux and Windows itself. Anywhere
else it goes through C: `omg` there is built from one generated C file
with `cc`, and `--build` hands its output to `cc` too.
[`docs/01-quickstart.md`](docs/01-quickstart.md#other-systems) has the
three commands, and [`docs/08-windows.md`](docs/08-windows.md) the
Windows specifics. CI builds and tests all three systems on every
push, and on Linux and Windows it does so on machines with no compiler
installed at all.

[`docs/toolchain.md`](docs/toolchain.md) is one page that says what
each part of the toolchain is for.

The tests are bash scripts (`tests/run.sh`); on Windows run them from
Git Bash.

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

- OMG files conventionally start with `;;;omg` on the first non-empty
  line. The lexer strips it if present. It's optional but recommended —
  editors and tools key off it.
- `emit` prints something to the screen.
- `#` starts a comment. Comments run to the end of the line.

---

## A tour of the language

Each section is a few lines you can paste into a `.omg` file and run with `omg yourfile.omg`.

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

If you forget the `alloc` for a brand-new name, OMG complains with a
Python-style traceback:

```
Traceback (most recent call last):
  File "/path/to/script.omg", line 4, in <top-level>
UndefinedIdentError: name
```

The full rules — they're worth reading once because they save you from
typos and accidental shadowing:

1. **Every new binding needs `alloc`.** `:=` only works on names
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
   `alloc` again inside the function. `alloc r := 2` would create a
   new local without touching the global.
5. **`alloc` twice for the same name at the top level is an error**:
   ```omg
   alloc count := 0
   alloc count := 5    # → SyntaxError: 'count' is already declared at the top level
   ```

### Numbers and math

OMG has two kinds of numbers: integers (whole numbers) and floats
(decimals). Most integer math returns integers, but `/` is **true
division** and always returns a float — use `//` when you want integer
(floor) division:

```omg
;;;omg

emit 1 + 2          # 3
emit 10 - 3         # 7
emit 4 * 5          # 20
emit 10 / 3         # 3.3333…  ← `/` is true division, always a float
emit 10 // 3        # 3        ← `//` is integer (floor) division
emit 10 % 3         # 1        ← remainder ("modulo")
emit -7 // 2        # -4       ← floor rounds toward minus infinity
emit 7 // -2        # -4       ← still floors when the divisor is negative
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

`/` is always true division and always returns a float, matching
Python 3. `//` is floor division — it keeps the integer type when both
operands are integers, and rounds toward minus infinity for floats:

```omg
;;;omg

emit 10 / 3         # 3.3333…  ← `/` always returns a float
emit 10 / 3.0       # 3.3333…
emit 10 // 3        # 3        ← `//` keeps the integer type
emit 10.5 // 3      # 3.0      ← floor div on float still rounds toward -∞
emit 7 % -2         # -1       ← `%` carries the sign of the divisor
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
emit not true           # false
emit not 5 > 3          # false    ← `not` applies to the whole comparison
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

`continue` skips the rest of the body and goes back to the test:

```omg
;;;omg

alloc i := 0
loop i < 6 {
    i := i + 1
    if i % 2 == 0 {
        continue
    }
    emit i          # 1, 3, 5
}
```

To walk a list or the characters of a string, use `for ... in`:

```omg
;;;omg

for name in ["ada", "grace", "linus"] {
    emit "hello, " + name
}

for ch in "omg" {
    emit ch         # o, m, g
}
```

`break` and `continue` work in a `for` loop too. The sequence is
evaluated once, before the first turn. To walk a dictionary, walk its
keys: `for k in dict_keys(d) { ... }`.

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

xs := xs + [40]         # joins two lists into a new one
emit xs                 # [99, 20, 30, 40]

append(xs, 50)          # adds to the end of this list, in place
emit xs                 # [99, 20, 30, 40, 50]
```

Use `append` when you build a list one item at a time: `xs := xs + [v]`
copies the whole list on every step.

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
own `n`. The capture is **by reference**, not a snapshot — if the
inner proc mutates the captured name, the next call sees the new
value:

```omg
;;;omg

proc make_counter() {
    alloc n := 0
    proc tick() {
        n := n + 1
        return n
    }
    return tick
}

alloc c := make_counter()
emit c()        # 1
emit c()        # 2
emit c()        # 3
```

Same semantics as Python and JavaScript — closures share storage with
their enclosing scope, not just the values that were there at
definition time.

A nested proc can call itself, and still sees everything it captured:

```omg
;;;omg

proc sum_to(limit, step) {
    proc walk(k) {
        if k > limit {
            return 0
        }
        return k + walk(k + step)
    }
    return walk(0)
}

emit sum_to(10, 5)      # 15   (0 + 5 + 10)
```

One rule to remember: a nested proc can only use variables that the
enclosing proc has *already declared* at the point where the nested proc
is written. Using one that is declared further down is a `SyntaxError`
(`'x' is used in a nested proc before the enclosing proc declares it`).
Move the `alloc` above the nested proc. Variables declared at the top
level of a file are globals and can be used from anywhere.

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
default the program prints a Python-style traceback to stderr and exits
non-zero — for example:

```
Traceback (most recent call last):
  File "/path/to/main.omg", line 17, in <top-level>
  File "/path/to/main.omg", line 13, in outer
  File "/path/to/main.omg", line 4, in inner
IndexError: index 5 out of range for length 0
```

To recover, wrap the risky bit in `try` / `except` — the traceback is
suppressed when the error is caught:

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
| **Strings & chars**            |                                                       |
| `length(x)`                    | length of a list or string, or number of entries in a dict |
| `chr(n)`                       | one-character string for byte value `n`               |
| `ascii(c)`                     | code point of a one-character string `c`              |
| `string_bytes(s)`              | UTF-8 byte values of `s` as a list of integers        |
| `bytes_to_string(bytes)`       | inverse of `string_bytes`: list of bytes → string     |
| **Numeric & math**             |                                                       |
| `int(x)` / `float(x)`          | convert between int and float (or parse from string)  |
| `floor(x)` / `ceil(x)`         | round a float toward `-∞` / `+∞`, returns int         |
| `round(x)`                     | round-half-to-even (banker's rounding), returns int   |
| `abs(x)`                       | absolute value (preserves int/float type)             |
| `sqrt(x)`                      | square root, returns float                            |
| `pow(a, b)`                    | `a` to the power of `b` (int^int stays int)           |
| `log(x)`                       | natural log, returns float                            |
| `sin(x)` / `cos(x)` / `tan(x)` | trigonometry in radians, returns float                |
| **Formatting**                 |                                                       |
| `hex(n)`                       | lowercase hex string for integer `n` (`hex(-255)` is `-ff`) |
| `binary(n)` / `binary(n, w)`   | binary string for `n`; with `w`, the low `w` two's-complement bits, zero-padded |
| `float_bits(s)`                | parse a float literal to its IEEE-754 i64 bit pattern |
| `bits_to_float(i)`             | inverse: i64 bits → float                             |
| **Collections**                |                                                       |
| `freeze(d)`                    | turn a dict into a read-only one                      |
| `dict_keys(d)`                 | list of a dict's keys (strings, in insertion order)   |
| `has_key(d, k)`                | `true` iff `d` has key `k` (non-dict `d` → `false`)   |
| `append(list, item)`           | add `item` to the end of `list`, in place             |
| `list_repeat(item, n)`         | a new list holding `item` `n` times                   |
| **File I/O**                   |                                                       |
| `read_file(path)`              | read a text file in one shot, or `false` on error     |
| `file_open(path, mode)`        | open and return a handle (`r`, `rb`, `w`, `wb`, `a`, `ab`) |
| `file_read(handle)`            | read everything remaining from a handle               |
| `file_read(handle, max)`       | read at most `max` bytes: for going through a large file a piece at a time |
| `file_write(handle, data)`     | write to a handle                                     |
| `file_close(handle)`           | close a handle                                        |
| `file_exists(path)`            | does the file exist?                                  |
| `file_size(path)`              | a file's length in bytes                              |
| `file_mtime(path)`             | when a file was last changed, in ms since 1970        |
| `is_dir(path)`                 | is the path a directory?                              |
| `read_dir(path)`               | list of entry names (sorted, no `.`/`..`)             |
| `make_dir(path)`               | create directories (mkdir -p semantics)               |
| `chmod(path, mode)`            | set a file's permission bits (`493` is `rwxr-xr-x`); on Windows it only checks the file exists |
| **Standard input**             |                                                       |
| `stdin_readline()`             | read one line from stdin, or `false` on EOF           |
| `stdin_read()`                 | slurp all of stdin to EOF as a string (empty on no input) |
| `stdin_read_bytes()`           | slurp all of stdin to EOF as a list of bytes (0–255)  |
| **Real-time terminal I/O**     |                                                       |
| `time_ms()`                    | current time in ms (epoch-based; suitable for elapsed-time) |
| `sleep_ms(n)`                  | pause the process for `n` milliseconds                |
| `stdin_set_raw(on)`            | toggle cbreak / no-echo mode (Unix terminals and the Windows console) |
| `stdin_read_key()`             | non-blocking one-byte read; returns char or `false`   |
| **Errors**                     |                                                       |
| `panic(msg)` / `raise(msg)`    | raise a runtime error (catchable with `try`/`except`) |
| `exit_with_error(msg)`         | print `msg` to stderr verbatim and exit 1 (uncatchable) |
| **Networking**                 | (see [docs/09-http-server.md](docs/09-http-server.md)) |
| `tcp_listen(host, port)`       | a listening TCP socket                                |
| `tcp_accept(listener)`         | the next connection to it                             |
| `tcp_connect(host, port)`      | a connection to somewhere else                        |
| `tcp_read(h, max)`             | up to `max` bytes as a list; `[]` once the other side has closed |
| `tcp_write(h, data)` / `tcp_write(h, data, offset)` | send a string or bytes (from `offset` on); returns the number of bytes sent |
| `tcp_close(h)`                 | close a socket                                        |
| `tcp_peer(h)`                  | the other end's address, as `"203.0.113.9:53012"`     |
| `tcp_nonblocking(h, on)`       | with it on, `tcp_accept` and `tcp_read` return `false` rather than wait, and `tcp_write` sends what fits |
| `tcp_poll(reads, writes, ms)`  | wait until one of these sockets can be read or written, or `ms` pass; returns `[readable, writable]` |
| **A window**                   | (programs run by `omg` or built to machine code, on Linux and Windows; see [docs/10-video.md](docs/10-video.md)) |
| `window_open(title, w, h)`     | open a window `w` x `h` pixels inside; returns a handle |
| `window_draw(win, pixels, width)` | show a picture: a list with one int per pixel (`red * 65536 + green * 256 + blue`), in rows of `width`; scaled to fit the window |
| `window_poll(win)`             | what happened to the window: `false`, `["key", name]`, `["resize", w, h]` or `["close"]`; never waits |
| `window_close(win)`            | close the window                                      |
| **Sound**                      | (as the window; see [docs/10-video.md](docs/10-video.md#sound)) |
| `audio_open(rate, channels)`   | open the loudspeakers for 16-bit sound, 1 or 2 channels; returns a handle |
| `audio_write(a, samples, from)` | hand over ints from -32768 to 32767, starting at `samples[from]`; takes what there is room for and returns how many; never waits |
| `audio_played(a)`              | how many moments of sound have been heard: the clock to show pictures by |
| `audio_pause(a, on)`           | hold the sound, or let it go on                       |
| `audio_close(a)`               | stop at once and let go of the device                 |
| **Process control**            |                                                       |
| `subprocess([cmd, args...])`   | run a child process; returns its exit code (a program that cannot be started raises `ValueError`) |
| `exit(code)`                   | exit the current process with the given status       |
| `getpid()`                     | current process ID (useful for unique tempfile names) |
| `interrupted()`                | how many times the program has been asked to stop (Ctrl-C, SIGTERM); from the first call on, those are counted instead of ending it |
| `executable_path()`            | path of the native OMG executable running the program, or `false` where there is none (in the browser) |
| **Reflection**                 |                                                       |
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

## Compile to a native binary

`omg --build` turns a program into a standalone executable:

```sh
omg --build foo.omg foo      # a native executable
./foo
```

On Linux and Windows (x86-64) `omg` writes the executable itself. The
machine-code back end at
[`src/backends/native-x64.omg`](src/backends/native-x64.omg) turns
bytecode into x86-64 and wraps it, with a runtime that is also written
in OMG ([`src/runtime/x64/`](src/runtime/x64/)), as an ELF or a PE
file. No C compiler, assembler or linker takes part. The result is one
small file (about 23 KB for hello world) that depends on nothing: no
libc on Linux, only `kernel32.dll` on Windows.

Any `omg` can build for either system:

```sh
omg --build foo.omg foo --target windows-x64   # foo.exe, from Linux
omg --build foo.omg foo --target linux-x64     # an ELF, from Windows
omg --build foo.omg foo --target c             # through a C compiler instead
```

`--target c` uses the C back end
([`src/backends/native-c.omg`](src/backends/native-c.omg)) and `cc`.
It is how OMG reaches macOS and ARM, where it is the default.

You can also stop at the generated source:

```sh
omg --emit-c  foo.omg foo.c    # the C that --target c compiles
omg --emit-js foo.omg foo.js   # JavaScript; run it with node or in a page
```

All of this is one program, `bin/omg` (`omg.exe` on Windows): the
compiler, a VM and the three back ends, written in OMG and built by
themselves. After changing anything under `src/`, rebuild it with:

```sh
scripts/build.sh               # Windows PowerShell: scripts\build.ps1
```

The full guide — architecture, language tour, pipeline, extension guide,
runtime internals, debugging, Windows — lives in [`docs/`](docs/).

---

## Editor support (VS Code)

The [`vscode/`](vscode/) directory holds a VS Code extension:

- **documentation on hover** for every builtin, keyword and operator, and
  for your own procs and variables (it shows the comment above them),
- **completion** and **signature help**, including the members of imported
  modules and dicts,
- **errors as you type**, worded as `omg` words them: the language server
  runs a port of the compiler's lexer and parser, plus checks for what the
  compiler leaves to run time (wrong argument counts, undefined names,
  assignment without `alloc`, broken imports),
- go to definition, find references and rename across files, an outline,
  and a formatter for indentation,
- syntax and semantic highlighting, two colour themes (OMG Dark and OMG
  Light, in the playground's colours), snippets, and file icons,
- commands to run the open file, build it, show its bytecode and open
  the REPL.

The extension isn't published on the marketplace; you build it from
this repo and install the resulting `.vsix` directly:

```sh
cd vscode
npm install                 # first time only
npm run package             # bundles the extension into omg-language-support.vsix
code --install-extension omg-language-support.vsix --force
```

It needs nothing else installed: the language server is part of the
extension. Only the run and build commands need the `omg` binary, which
they find in `bin/` or on your `PATH`.

Its tests (`npm test` in `vscode/`) compare the language server with the
compiler: the builtin list, the syntax errors, and every example shown
in a hover. See [`vscode/README.md`](vscode/README.md).

---

## What's in this repo

```
omglang/
├── src/             the OMG toolchain, written in OMG
│   ├── omg.omg          the `omg` command; imports the modules below
│   ├── compiler.omg     the compiler: source to bytecode
│   ├── bytecode.omg     reads a .omgb file back in
│   ├── vm.omg           the bytecode interpreter (`omg foo.omg`, the REPL)
│   ├── backends/        bytecode to something that runs without the VM
│   │   ├── backend.omg      what the back ends share (scope analysis)
│   │   ├── native-x64.omg   x86-64 back end: machine code, ELF and PE files (`omg --build`)
│   │   ├── native-c.omg     C back end (`omg --build --target c`, `omg --emit-c`)
│   │   └── native-js.omg    JS back end (`omg --emit-js`, the browser builds)
│   ├── runtime/         what a built program carries with it
│   │   ├── x64/             the runtime for machine code, written in OMG
│   │   │                    (core, float, builtins, and linux / windows)
│   │   ├── omg_rt.h         C runtime, pasted into generated C
│   │   └── omg_rt.js        JS runtime, pasted into generated JavaScript
│   └── browser/         entry points of the two web bundles
│       ├── omg-web.omg      the playground: compiler + VM
│       └── omg-explorer.omg the compiler explorer: + the C and JS back ends
├── bin/             the built toolchain, checked in: omg (Linux), omg.exe (Windows),
│                    and the runtime files they read (x64/, omg_rt.h, omg_rt.js)
├── scripts/         build and check scripts
│   ├── build.sh         builds bin/omg and bin/omg.exe from src/
│   ├── build.ps1        the same build for Windows PowerShell (builds omg.exe)
│   ├── build-web.sh     builds the browser bundles from src/, into build/web/
│   └── update-site.sh   builds them, tests them, and copies them into the site
├── rust/            the Rust runtime OMG started from. Frozen: kept as the way to
│                    rebuild OMG from source alone, no longer built or needed
├── examples/        programs to read and run: one language feature each
│   └── games/           interactive terminal games (snake, pong, tetris, rogue)
├── tests/           shell-driven test suite (conformance, parity, regression, REPL, builtins)
├── tools/           programs written in OMG (wc, grep, sort, omgdb, an editor, a web server, a video player)
├── web/             two pages that forward old playground links to the site
├── docs/            the manual: quickstart, language tour, architecture, extending, Windows
└── vscode/          VS Code extension (highlighting, themes, language server)
```

Some interesting starting points:

- [`examples/prime_sieve.omg`](examples/prime_sieve.omg): finds primes up to 100.
- [`examples/maze_solver.omg`](examples/maze_solver.omg): breadth-first search over a grid.
- [`examples/higher_order.omg`](examples/higher_order.omg): closures + first-class functions.
- [`examples/donut.omg`](examples/donut.omg): Andy Sloane's spinning 3D
  ASCII donut, ported to OMG. Exercises the float kit and ANSI-escape
  animation — build it with `omg --build` for a smooth spin.
- [`examples/games/`](examples/games/): interactive terminal games (Snake, Pong, Tetris,
  a small roguelike) driven by `time_ms` / `sleep_ms` /
  `stdin_set_raw` / `stdin_read_key`. See
  [`examples/games/README.md`](examples/games/README.md).
- [`tools/unix/wc.omg`](tools/unix/wc.omg), [`tools/unix/grep.omg`](tools/unix/grep.omg),
  [`tools/json.omg`](tools/json.omg): Unix-style utilities,
  written in OMG. See [`tools/README.md`](tools/README.md) for the
  full list.
- [`tools/lib/http.omg`](tools/lib/http.omg): an **HTTP server** written in
  OMG: one loop serving thousands of connections, with routing, static
  files, keep-alive, timeouts and a clean stop on Ctrl-C. About 45,000
  requests a second when built. See
  [`docs/09-http-server.md`](docs/09-http-server.md).
- [`tools/db/`](tools/db/): **omgdb**, a small SQL database written in OMG —
  a paged on-disk format, a recursive-descent SQL parser, and a SQLite-style
  REPL. See [`tools/db/README.md`](tools/db/README.md).
- [`tools/video/`](tools/video/): a **video player** written in OMG: an MP4
  reader, an H.264 decoder that matches ffmpeg bit for bit, an AAC
  decoder for the sound, a window opened by speaking X11 (Linux) or
  calling user32 (Windows) directly, and sound sent to PulseAudio or
  PipeWire (Linux) or winmm (Windows) the same way, with no library in
  between. See [`docs/10-video.md`](docs/10-video.md).

---

## A small piece of trivia

OMG is written in OMG, all the way down.

It started the usual way: a runtime in another language (Rust, still
in [`rust/`](rust/), now frozen), and on top of it a compiler written
in OMG, [`src/compiler.omg`](src/compiler.omg), that could compile
itself. Then a VM in OMG, then a back end that wrote C, so that the
whole thing could become an ordinary program with a C compiler's help.

The last step removed the help. `omg` now writes machine code itself
([`src/backends/native-x64.omg`](src/backends/native-x64.omg)): its
own instruction encoder, its own linker, its own ELF and PE writers.
The runtime every program needs (memory, strings, floats, dictionaries,
files, sockets) is OMG as well, down to the system calls. So when
`omg` builds itself, nothing but OMG is running, and nothing but OMG
is in the result:

```sh
bin/omg  --build src/omg.omg bin/omg2    # omg builds its own source (about 1.5 s)
bin/omg2 --build src/omg.omg bin/omg3    # and what it built does it again
cmp bin/omg2 bin/omg3                    # the same file, byte for byte
```

That fixed point is checked on every push, on Linux and on Windows, on
machines with no compiler installed. It works across systems too: the
Windows `omg.exe` that is checked in was written by the Linux `omg`.

---

## More reading

- [`docs/toolchain.md`](docs/toolchain.md): **what each part is for**,
  on one page. The `omg` command, the compiler, the VM, the three back
  ends, and where the Rust runtime went.
- [`docs/`](docs/): the manual. Quickstart, architecture, language
  tour, the pipeline from source to executable, the runtimes, how to
  add a feature in every place it has to go, debugging, Windows, the
  HTTP server.
- [`docs/compilation-pipeline.md`](docs/compilation-pipeline.md): how
  `omg foo.omg` actually runs your script, step by step.
- [`rust/README.md`](rust/README.md): the frozen Rust runtime, and how
  to bootstrap OMG from it.
- [`tools/README.md`](tools/README.md): the OMG-in-OMG tools.
- [The playground](https://versyx.digital/omglang/playground/), in your browser. It
  is part of the site, which is kept in another repository;
  [`docs/toolchain.md`](docs/toolchain.md#the-browser-builds) says how the two meet.
- [`vscode/README.md`](vscode/README.md): VS Code extension.

---

## License

MIT.

Educational project, not intended for production use.
