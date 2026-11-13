# 06 · The runtimes

A built program carries a runtime: the code for values, memory,
strings, lists, dictionaries, builtins and errors. OMG has three, one
per back end.

| Back end | Runtime | Written in |
| -------- | ------- | ---------- |
| x86-64 (`omg --build`) | `src/runtime/x64/*.omg` | OMG |
| C (`--target c`, `--emit-c`) | `src/runtime/omg_rt.h` | C |
| JavaScript (`--emit-js`) | `src/runtime/omg_rt.js` | JavaScript |

They implement the same language and are held to the same answers by
the conformance corpus. This page covers the first two: the
machine-code runtime, which is the one you are using on Linux and
Windows, and then the C runtime in detail.

# Part 1: the machine-code runtime

## The files

| File | Lines | What is in it |
| ---- | ----: | ------------- |
| `core.omg`     | ~1900 | Memory, reference counts, strings, output, errors and tracebacks, closures, lists, dicts, equality, arithmetic, indexing, start-up |
| `float.omg`    | ~750  | Floats to text and back, exactly, using its own big-number arithmetic |
| `builtins.omg` | ~1500 | Every builtin, one `bi_` proc each, and the table `call_builtin` uses |
| `linux.omg`    | ~850  | The `os_*` procs on Linux system calls |
| `windows.omg`  | ~1000 | The same `os_*` procs on `kernel32.dll` and `ws2_32.dll` |

`omg` joins `core`, `float`, `builtins` and one of the system files
into a single unit, compiles it with the ordinary compiler, and hands
the bytecode to the back end along with your program. Only the procs
your program can reach end up in the executable.

## Raw mode

The runtime cannot be written in ordinary OMG, because ordinary OMG
*needs* the runtime: you cannot implement lists with lists. So the
back end compiles these files under different rules, called raw mode:

- Every value is one 64-bit word with no tag: an integer, a pointer,
  or the bits of a float. `true` is 1; `false` and `none` are 0.
- `+ - *` wrap around; `//` and `%` are the machine's signed division.
- A string literal is the address of a static string object.
- A top-level `alloc NAME := 123` in capitals is a compile-time
  constant; any other top-level name is a 64-bit global.
- The name of a proc, used as a value, is its address.
- Memory, the operating system and float arithmetic are reached
  through reserved names: `__peek8/16/32/64`, `__poke8/16/32/64`,
  `__memcpy`, `__syscall`, `__fadd`, `__i2f`, and so on. On Windows,
  `__import("kernel32.dll!WriteFile")` names a system function and
  `__wincallN` calls it.
- Lists, dicts, indexing, builtins, closures, `emit`, `try` and
  `raise` do not exist and are refused at build time.

So the syntax is OMG and the meaning is close to C without types:

```omg
# Append a value; the list takes over its count.
proc rt_list_push(l, tag, v) {
    alloc n := __peek64(l + L_LEN)
    alloc cap := __peek64(l + L_CAP)
    if n == cap {
        alloc ncap := cap * 2
        if ncap < 4 { ncap := 4 }
        alloc ni := rt_alloc(ncap * 16)
        if cap > 0 {
            __memcpy(ni, __peek64(l + L_ITEMS), n * 16)
            rt_free(__peek64(l + L_ITEMS), cap * 16)
        }
        __poke64(l + L_ITEMS, ni)
        __poke64(l + L_CAP, ncap)
    }
    alloc it := __peek64(l + L_ITEMS) + n * 16
    __poke64(it, v)
    __poke64(it + 8, tag)
    __poke64(l + L_LEN, n + 1)
    return 0
}
```

The full rules are at the top of part 3 of
[`native-x64.omg`](../src/backends/native-x64.omg). These files cannot
be run by the VM or built by the other back ends.

## Values

A value of your program is two words: a tag and a payload.

| Tag | Type | Payload |
| --: | ---- | ------- |
| 0 | Int | the integer |
| 1 | Float | the bits of the double |
| 2 | Bool | 0 or 1 |
| 3 | None | 0 |
| 4 | (unset) | a variable nothing has been stored into |
| 8 | Str | pointer to the object |
| 9, 10, 11 | List, Dict, Closure | pointer to the object |
| 12 | Cell | a captured variable |

Tags of 8 and above are heap objects, and every heap object starts
with its reference count. The layouts (offsets in bytes) are at the
top of `core.omg`:

- **String**: count, length in characters, length in bytes, cached
  hash, then the UTF-8 data and a NUL.
- **List**: count, length, capacity, pointer to the items (16 bytes
  each: payload, tag).
- **Dict**: count, length, capacity, entries (key, hash, value), and
  an index of 32-bit slots for hashing. Small dicts are scanned and
  have no index. Entries keep insertion order.
- **Closure**: count, code address, name, number of parameters, and
  one cell pointer per captured variable.

Runtime procs take a value as two arguments (`t, v`) and return a
payload in RAX, leaving the tag in the global `rt_tag` when the type
of the result is not fixed.

## Memory

There is no `malloc`. `rt_alloc(size)` hands out blocks in size
classes (multiples of 16 up to 1024 bytes, powers of two above) and
`rt_free(p, size)` takes them back by size, so no block carries a
header. Small classes are carved from chunks obtained from the system
(`mmap` on Linux, `VirtualAlloc` on Windows); a block of 256 KiB or
more is mapped and unmapped on its own.

Memory is reclaimed by reference counting, strings included. A value
in a variable or on the operand stack owns one count; moving a value
moves the count, copying takes a new one, and the count reaching zero
frees the object and releases what it holds (`rt_destroy`). Literals
are static and marked immortal. As in any reference-counted system, a
cycle (a list that contains itself) is never freed.

## Errors and tracebacks

An error is a string, `"Kind: message"`. `try` pushes a handler
(address, stack pointer, frame pointer) onto a small stack;
raising pops the top handler, resets the two registers and jumps. With
no handler left, the runtime prints a traceback and exits with 1.

Nothing is recorded while the program runs to make tracebacks
possible. When an error goes uncaught, the runtime walks the chain of
saved frame pointers and looks each return address up in tables the
back end wrote into the executable: which proc, which file, which
line. The output is the same Python-style trace as everywhere else:

```
Traceback (most recent call last):
  File "/path/to/foo.omg", line 12, in <module>
  File "/path/to/foo.omg", line 7, in helper
ValueError: something went wrong
```

Recursion is bounded by the machine stack. Every proc compares the
stack pointer with a limit on entry, and running out is an ordinary
`RuntimeError: maximum recursion depth exceeded` that `try` can catch,
not a crash. The stack is 64 MiB on both systems.

## Output and files

`emit` collects its text in a buffer and writes it at the end of each
call, so a line is one write and stays in order with standard error.
File reads and writes go straight to the system; there is no buffering
layer in between.

## Floats

Arithmetic is SSE2; `sin`, `cos`, `tan`, `log` and `pow` use the x87
instructions. Printing gives the shortest decimal that reads back as
the same double, and parsing is correctly rounded. `float.omg` does
both with whole-number arithmetic of arbitrary size, which is slow
next to the published fast algorithms but short and easy to check.

## The system layer

Everything above is written against about sixty procs named `os_*`:
`os_open`, `os_read`, `os_write_all`, `os_alloc`, `os_getcwd`,
`os_subprocess`, `os_tcp_connect`, `os_read_key` and so on.
`linux.omg` implements them with system calls made directly
(`__syscall`), `windows.omg` with calls into `kernel32.dll` and
`ws2_32.dll`. Porting the runtime to another x86-64 system means
writing a third file like these, and teaching the back end that
system's executable format.

## Start-up

The entry point of an executable is a few instructions written by the
back end: save the initial stack pointer, call `rt_start`, run the
program's top-level code, flush output, exit. `rt_start` initialises
the system layer and the heap, marks every global as unset, and fills
in `args`, `module_file` and `current_dir`.

## Known limits

- TCP on Linux takes a dotted IPv4 address, `localhost`, or a name
  listed in `/etc/hosts`; there is no DNS client. (Windows uses the
  system resolver.)
- `pow` and the trigonometric functions come from x87 and can differ
  from a C library in the last digit.
- A float literal with more than 19 significant digits may be off by
  one unit in the last place.
- A traceback for very deep recursion shows the outermost 1024 frames
  and the innermost 8.
- `bytes_to_string` does not check that its input is valid UTF-8.
- The executable records the full paths of its source files, for
  tracebacks.

# Part 2: the C runtime (`omg_rt.h`)

`src/runtime/omg_rt.h` is what a program built with `--target c`
carries. About 3,700 lines of C (C99 plus a few GNU extensions that
GCC and Clang share), inlined into every generated C file. It compiles
for Linux, macOS and other Unixes, and for Windows with MinGW-w64; the
Windows side is described in [08-windows.md](08-windows.md).

## What it provides

| Layer                 | Examples                                  |
| --------------------- | ----------------------------------------- |
| Value representation  | `Value`, `omg_int`, `omg_str`, `omg_none` |
| Heap structures       | `OmgList`, `OmgDict`, `OmgClosure`        |
| Refcounting           | `omg_inc`, `omg_dec`, `omg_assign`        |
| Operators             | `omg_add`, `omg_mul`, `omg_eq`, `omg_index` |
| String operations     | `omg_str_concat`, `omg_str_index`, `omg_str_slice` |
| Closure cells         | `omg_cell_new`, `omg_cell_get`, `omg_cell_set` |
| Exception handling    | `OmgBlock`, `omg_panic`, `omg_raise`      |
| Source map + frames   | `omg_src_files`, `omg_frame_stack`, `omg_emit_traceback` |
| Builtins              | `omg_builtin_int`, `omg_builtin_file_open`, etc. |
| Process control       | `omg_builtin_subprocess`, `omg_builtin_exit`, `omg_builtin_getpid` |
| I/O                   | `omg_emit`, file table                    |
| Platform layer        | `omg_os_fopen`, `omg_os_getcwd`, `omg_os_time_ms`, `omg_start` |

## The Value type

```c
typedef enum {
    OMG_INT, OMG_STR, OMG_BOOL, OMG_NONE,
    OMG_CLOSURE, OMG_LIST, OMG_DICT, OMG_FLOAT
} omg_tag;

typedef struct {
    omg_tag tag;
    union {
        int64_t i;
        const char *s;
        int b;
        struct OmgClosure *c;
        struct OmgList *l;
        struct OmgDict *d;
        double f;
    } v;
} Value;
```

A tagged union, 16 bytes on x86-64 (8-byte tag + 8-byte payload).

Primitives (`int`, `float`, `bool`, `none`) are stored inline.
Heap types (`str`, `closure`, `list`, `dict`) are pointers — the actual
data lives on the heap.

**Strings are special**: they're a `const char *` and not refcounted.
String literals point into static storage; results from `omg_str_concat`
etc. are heap-allocated and leaked. Strings are typically small and rarely
deeply duplicated, so the leak is bounded; we may add string refcounting
in a future iteration.

## Refcounting

OMG uses a CPython-style transfer-ownership protocol. The rules:

```
  Operation              omg_inc?    Notes
  ─────────────────      ─────────   ──────────────────────────
  Push fresh value (rc=1)  no        omg_int(7), omg_list_build(...)
  Push existing (LOAD)     yes       Stack and var both own
  Pop and discard          dec       omg_dec(stack[--sp])
  Pop and transfer         no        sp--; new owner inherits
  Binop                    dec both  After computing result
  Store to slot            transfer  omg_assign(&slot, popped)
```

### Why `omg_assign`?

Naive `slot = value` would leak the old occupant. `omg_assign` does the
right thing:

```c
static void omg_assign(Value *slot, Value v) {
    Value old = *slot;
    *slot = v;
    omg_dec(old);    // ← release the previous owner
}
```

Every store goes through this.

### Why transfer instead of "always inc"?

Saves an inc/dec round-trip per push. The stack is the hot path; eliminating
half its refcount traffic is a significant win.

### What about strings?

`omg_inc`/`omg_dec` are no-ops for `OMG_STR`, `OMG_INT`, `OMG_BOOL`,
`OMG_NONE`, `OMG_FLOAT`. Only the heap-allocated tags (`LIST`, `DICT`,
`CLOSURE`) actually count.

## Heap structures

### Lists

```c
typedef struct OmgList {
    int rc;
    int len;
    int cap;
    Value *items;
} OmgList;
```

Dynamic array. Doubles capacity on push. Each `items[i]` is an owning slot —
`omg_dec`'d when the list is freed.

### Dicts

```c
typedef struct OmgDict {
    int rc;
    int len, cap;
    char **keys;
    uint32_t *hashes;   // FNV-1a 32-bit prefix; lookup gates strcmp on a match
    Value *vals;
    int frozen;         // set by freeze() — forbids further writes
} OmgDict;
```

Linear-scan lookup, but with a hash-prefix filter (`omg_dict_find` compares
the FNV-1a of the key against `d->hashes[i]` before falling through to
strcmp). Practically O(N) but with the strcmp constant cut by an order of
magnitude — enough to make per-LOAD env lookups in the OMG-in-OMG VM
tolerable for procs with ~25 locals. For huge dicts a true open-addressed
hashtable would still win; this is a step on the way without paying for
rehash-on-resize. Keys are owned heap copies; values are owning slots.

### Closures

```c
typedef struct OmgClosure {
    int rc;
    OmgFn fn;
    const char *name;    // bare source name (no __mod_N__ prefix), for tracebacks
    Value *captured;     // heap array of 1-element cells (OMG_LIST values)
    int cap_count;
} OmgClosure;
```

Captured slots are **cells** (1-element `OMG_LIST` values), not raw
`Value`s. The closure shares each cell with the enclosing frame, so a
parent's `STORE_LOCAL` and the closure's `LOAD` see the same storage
(Python/JS-style by-reference capture). See `omg_cell_new` /
`omg_cell_get` / `omg_cell_set` for the cell helpers.

`name` is the bare display name used when this closure is invoked
indirectly via `CALL_VALUE` — without it, a traceback would show the
mangled `__mod_2__tick` instead of `tick`.

`OmgFn` is the C function pointer for the OMG proc:

```c
typedef Value (*OmgFn)(Value *captured, int cap_count, int argc,
                       Value a0,  Value a1,  ...,  Value a30, Value a31);
```

Every compiled proc starts with two checks. `argc` is compared with the
proc's parameter count (`omg_arity_error`): direct calls are checked by
the compiler, but a call through a value is not, and without the check a
missing argument would be read as whatever happened to be in the slot.
Then `OMG_STACK_CHECK()` compares the frame address with a limit that
`omg_stack_init` computed at start-up from `RLIMIT_STACK` (which it first
raises to 64 MiB on Linux where the hard limit allows; on macOS the
main thread's stack size is fixed when the process starts, 8 MB, so the
limit is used as found). On Windows the program runs on a thread
created with a 64 MiB stack, and the limit comes from asking the system
where that stack ends (`VirtualQuery`). Past the limit the proc
raises `RuntimeError: maximum recursion depth exceeded` instead of
running off the end of the stack, so runaway recursion is an ordinary,
catchable error rather than a segfault.

A closure that names itself (a nested proc that recurses) finds itself
through `omg_closure_self`, which rebuilds the closure value from the
`captured` array the proc was entered with. The closure cannot simply
capture itself: its captures are collected before it exists, and a
closure that owned a reference to itself would never be freed.

Args are passed inline rather than as an array, so cc -O3 can sibling-call
optimize tail calls. `OMG_MAX_ARITY = 32`; bump it (everywhere) if you need
more parameters. The cap *only* applies to the C-AOT path
(`omg --build foo.omg --target c`); the VM, machine code and the
JavaScript path have no such limit. Most OMG procs use ≤9 — the headroom is just so the constraint
stays invisible.

## Exception handling and tracebacks

### The data structures

```c
typedef struct OmgBlock {
    int saved_sp;          // operand stack depth at SETUP_EXCEPT
    int saved_frame_top;   // call-frame depth at SETUP_EXCEPT
    jmp_buf jb;            // setjmp target
} OmgBlock;

#define OMG_MAX_BLOCKS 256
static OmgBlock *omg_block_stack[OMG_MAX_BLOCKS];
static int omg_block_top = 0;

static Value omg_pending_error;   // set by raise/panic, read by handler

// User-visible call-frame stack for traceback rendering. CALL /
// CALL_VALUE push a frame; RET pops; TCALL rewrites the top frame's
// name in place. Independent of OmgBlock — exception unwinding
// truncates it back to the saved depth.
typedef struct OmgFrame {
    const char *name;        // bare proc name (no __mod_N__ mangling)
    uint32_t call_file_idx;  // file containing the CALL/CALL_VALUE
    uint32_t call_line;      // line of the call instruction
} OmgFrame;
#define OMG_MAX_FRAMES 1024
static OmgFrame omg_frame_stack[OMG_MAX_FRAMES];
static int omg_frame_top = 0;
```

The transpiler also emits a per-program source-file table and threads
two globals — `omg_current_file_idx` (set once at function entry) and
`omg_current_line` (rewritten before each instruction whose line
differs from the previous one) — so any panic site has full
file/line context.

### How `try` becomes C

For each `try { body } except err { handler }`, the transpiler emits:

```c
{
    OmgBlock *omg_b = malloc(sizeof(OmgBlock));
    omg_b->saved_sp = sp;
    omg_b->saved_frame_top = omg_frame_top;
    omg_block_push(omg_b);
    if (setjmp(omg_b->jb) != 0) {
        // longjmp landed here; omg_panic restored omg_frame_top.
        sp = omg_b->saved_sp;
        free(omg_b);
        stack[sp++] = omg_pending_error;
        goto handler_label;
    }
}
// fall through: try body
...
omg_block_take(); free(...);   // POP_BLOCK on clean exit
```

### How `panic`/`raise` work

`omg_panic(prefix, msg)` is the universal entry point for runtime errors:

```c
static void omg_panic(const char *prefix, const char *msg) {
    const char *full = omg_format_error(prefix, msg);
    if (omg_block_top > 0) {
        omg_pending_error = omg_str(full);
        OmgBlock *b = omg_block_stack[--omg_block_top];
        // Restore the frame stack to its depth at SETUP_EXCEPT so a
        // later uncaught panic doesn't include frames unwound by this
        // catch.
        if (omg_frame_top > b->saved_frame_top) {
            omg_frame_top = b->saved_frame_top;
        }
        longjmp(b->jb, 1);
    }
    // Uncaught: print a Python-style traceback via omg_emit_traceback
    // when the program has a source map, otherwise fall back to the
    // bare "Kind: msg" line (handwritten C harnesses with no map).
    if (omg_src_files != NULL) {
        omg_emit_traceback(full);
    } else {
        fprintf(stderr, "%s\n", full);
    }
    exit(1);
}
```

If a try block is active, `longjmp` transfers control to the most recent
`SETUP_EXCEPT`. Otherwise the program emits a traceback and exits 1.

Every recoverable error path in the runtime goes through this — division
by zero, index out of range, type mismatch in arithmetic, etc. So
`try { ... } except err { ... }` catches all of them.

### The traceback format

`omg_emit_traceback(fullmsg)` walks `omg_frame_stack` and prints a
Python-style trace identical to what the VM and the machine-code
runtime produce:

```
Traceback (most recent call last):
  File "main.omg", line 5, in <top-level>
  File "main.omg", line 12, in outer
  File "main.omg", line 17, in inner
IndexError: index 5 out of range for length 0
```

The first frame's call site is labelled `<top-level>`; subsequent
frames take their *caller's* display name (i.e. `frames[i-1].name`).
The site line uses `omg_current_file_idx` / `omg_current_line`, which
are the globals the transpiler updated at the most recent instruction.

### Undefined names

A variable slot that nothing has been stored into holds `OMG_UNSET`, a
tag no OMG value ever has. `LOAD` and `STORE` of such a slot raise
`UndefinedIdentError: name`, as the VMs do, where they used to read `0`
and to create the variable silently. Globals start out as
`OMG_UNSET_INIT`, locals as `omg_unset()`, and a boxed variable (one that
a nested proc captures) goes through `omg_cell_load` / `omg_cell_store`,
which make the same check on the slot that should hold the cell. A
`CALL` to a name that is not in the function table compiles to
`omg_undefined("name")`, so it too fails only if it is reached.

Which names are parameters, locals, captured or global is decided by
the scope analysis in `backend.omg` (`analyze_all_functions`), which
follows the same rules as the compiler: a name belongs to the nearest
enclosing proc that declares it before the nested proc is defined.
`native-c.omg` and `native-js.omg` both import that one module, so the
C and JavaScript output always agree on what a name refers to.

### Caveats

- **C stack frames leak through longjmp.** If a panic happens deep inside
  several function calls, the locals in those frames don't get `omg_dec`'d.
  In practice this is rare (most try/except is shallow) and the leak is
  proportional to the panic frequency, not program duration.
- **Operand stack is restored** to `saved_sp`. Anything on top is discarded
  without dec — it leaks. Same trade-off.

## File I/O and path resolution

```c
static const char *omg_cwd_str = ".";  // initialized from getcwd() in main()

static const char *omg_resolve_path(const char *p) {
    // Absolute paths pass through; relative paths get joined with omg_cwd_str.
}
```

`main()` populates `omg_cwd_str` from `getcwd()` at startup. Whenever the
program does `current_dir := ...`, the transpiler emits an extra line to
sync `omg_cwd_str` (see `STORE` in `native-c.omg`).

File handles use a small static table:

```c
static OmgFileEntry omg_file_table[OMG_MAX_FILES];   // 64 slots
```

`file_open` returns a 1-based handle (0 is always invalid). `file_close`
frees the slot.

## Output buffering

`main()` calls `setvbuf(stdout, NULL, _IOLBF, 0)` to force line-buffering
on stdout. Without this, stdout would be block-buffered when piped, causing
stderr (unbuffered) to "jump" ahead of stdout in merged output and diverge
from every other way of running the program.

## Float formatting

OMG specifies "shortest round-trippable" formatting for floats
(e.g. `0.1` should print as `0.1`, not `0.10000000000000000555`). C's
`printf` doesn't ship this primitive, so we walk precision 1-17:

```c
for (int p = 1; p <= 17; p++) {
    snprintf(buf, bufsize, "%.*g", p, f);
    if (strchr(buf, 'e') || strchr(buf, 'E')) continue;  // prefer decimal
    double r;
    if (sscanf(buf, "%lf", &r) == 1 && r == f) { settled = 1; break; }
}
```

The first decimal form that round-trips wins. For very large/tiny values
where decimal would be unwieldy, falls back to `%.17g` scientific notation.

## How big is everything?

The same programs, built both ways on Linux:

| Program                  | `omg --build` | `--target c` |
| ------------------------ | ------------: | -----------: |
| Three lines, one `emit`  | 22 KB         | 22 KB (plus libc) |
| `examples/maze_solver.omg` (80 lines) | 37 KB | 64 KB |
| `src/compiler.omg` (3000 lines) | 268 KB | 924 KB |
| `omg.omg` and its imports (the whole toolchain) | 1.3 MB | |

A machine-code executable is static and complete: its 22 KB floor is
the part of the runtime that start-up, `emit` and error reporting
need, and nothing is loaded at run time. A C-built one is linked
against the system's C library. Beyond the floor, size scales with how
much OMG you've got: for `omg` itself almost all of it is generated
code for the compiler, the VM and the back ends.

## Read next

- [05-extending.md](05-extending.md) — practical guide to adding new opcodes
  and builtins
- [07-debugging.md](07-debugging.md) — diagnosing problems
- The source: [src/runtime/x64/core.omg](../src/runtime/x64/core.omg) and
  [src/runtime/omg_rt.h](../src/runtime/omg_rt.h) are well-commented
  and worth reading directly.
