# 06 · The C runtime (`omg_rt.h`)

`bootstrap/src/omg_rt.h` is the only piece of non-OMG, non-Rust code that ships
with native binaries. ~1700 lines of C99, inlined into every AOT output.

This doc covers what's in it and why.

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

Args are passed inline rather than as an array, so cc -O3 can sibling-call
optimize tail calls. `OMG_MAX_ARITY = 32`; bump it (everywhere) if you need
more parameters. The cap *only* applies to the C-AOT path
(`omg --build foo.omg`); the Rust runtime, the OMG-on-OMG VM, and the
transpiled-JS path all take args via variadic structures and have no
limit. Most OMG procs use ≤9 — the headroom is just so the constraint
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
Python-style trace identical to what the Rust VM produces:

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
from the Rust VM's behavior (`println!` line-buffers natively).

## Float formatting

Both Rust and OMG specify "shortest round-trippable" formatting for floats
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

```
$ wc -l bootstrap/src/omg_rt.h
1697 bootstrap/src/omg_rt.h
```

| Program                  | Source size | AOT binary |
| ------------------------ | ----------- | ---------- |
| Hello world (1 emit)     | 22 B        | 16 KB      |
| Calculator (~25 lines)   | 1 KB        | 35 KB      |
| `compiler.omg` (omgc)    | 65 KB       | 432 KB     |
| `native-c.omg` (omgcc)   | 54 KB       | 290 KB     |
| `native-js.omg` (omgjs)  | 53 KB       | 330 KB     |

The constant overhead is small (~16 KB for trivial programs, mostly libc);
the rest scales with how much OMG you've got. For `omgc` (the largest of the
toolchain binaries), almost all the size is generated code for `compiler.omg`
itself — the runtime is < 5%.

## Read next

- [05-extending.md](05-extending.md) — practical guide to adding new opcodes
  and builtins
- [07-debugging.md](07-debugging.md) — diagnosing problems
- The source: [bootstrap/src/omg_rt.h](../../bootstrap/src/omg_rt.h) is well-commented
  and worth reading directly.
