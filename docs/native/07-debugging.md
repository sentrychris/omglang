# 07 · Debugging

When OMG isn't doing what you expect. Diagnostic tools first, common
problems second.

## Tools at each stage

| Stage         | How to peek                                          |
| ------------- | ---------------------------------------------------- |
| Source        | `cat foo.omg`                                        |
| Tokens / AST  | (no dump tool yet — read the parser if needed)       |
| Bytecode      | `omg --disasm foo.omgb` (or `foo.omg`)               |
| C source      | run `omgcc` manually to produce a `.c` you can keep  |
| Native binary | gdb, strace, valgrind                                |

`omg --build` cleans up its tempdir, so to keep the intermediate `.c`,
run the transpiler yourself.

### Disassembling bytecode

```sh
omg --compile foo.omg foo.omgb
omg --disasm foo.omgb            # native driver; matches the Rust frontend byte-for-byte
# Or feed a .omg directly — disasm runs the compiler in-process:
omg --disasm foo.omg
```

You'll get the function table at the top, then the linear instruction stream:

```
# functions
FUNC __mod_1__add (a, b) @ 89
FUNC __mod_1__multiply (a, b) @ 101
# code
0000  PushInt(7)
0001  StoreLocal("x")
0002  Load("x")
...
```

Each line is `<pc>  <opname>(<args>)`. If your program is misbehaving,
disasm is usually the first place to look.

### Inspecting generated C

```sh
omg --compile foo.omg foo.omgb
bootstrap/bin/omgcc foo.omgb /tmp/foo.c     # or runtime/target/release/omg bootstrap/src/native-c.omg foo.omgb /tmp/foo.c
less /tmp/foo.c
```

The runtime header is at the top (~1700 lines) — skip past it. Each
generated proc starts with:

```c
static Value omg_pN(Value *captured, int cap_count, int argc,
                    Value omg_a0, Value omg_a1, ...) {
```

Each bytecode instruction is a small block — usually a `{ ... }` scope
that pops, computes, and pushes. There are no per-instruction comments in
the generated C, so to map a C line back to a bytecode op, run `--disasm`
in parallel and match patterns by hand.

### Disassembling the binary

```sh
objdump -d foo | less
```

Useful for confirming `cc -O2` did the optimizations you expected (e.g.
sibling-call TCO showing as `jmp omg_pN` rather than `call omg_pN`).

## Common errors and what they mean

### Reading a traceback

Every uncaught runtime error prints a Python-style traceback to stderr:

```
Traceback (most recent call last):
  File "/abs/path/main.omg", line 17, in <top-level>
  File "/abs/path/main.omg", line 13, in outer
  File "/abs/path/main.omg", line 8, in middle
  File "/abs/path/main.omg", line 4, in inner
IndexError: index 5 out of range for length 0
```

Frames are listed most-recent-call-*last* (Python convention). The
final `File ... in <fn>` line is the call site where the error fired;
the lines above it are the chain that led there. Names show the
**bare** source name — `__mod_N__` module-mangling is stripped for
display.

A few quirks:
- **Tail calls** (`return foo(...)`) reuse the caller's frame slot, so
  TCO chains collapse to a single frame with the innermost callee's
  name. Match this against Python's missing trampoline frames if you
  expected to see all of them.
- **Caught exceptions** print no traceback — only the final, uncaught
  one does. The trace itself goes to stderr, so `... 2>/dev/null`
  hides it while preserving any `emit` output.
- **REPL chunks** show `File "<repl>", line N` — `<repl>` is a
  placeholder because the chunk wasn't read from disk.
- **Empty source map** (synthetic bytecode, e.g. hand-built unit
  tests in `vm/tests.rs`) falls back to a bare `Kind: message` line
  with no `File ...` prefix.

### `UndefinedIdentError: foo`

The name `foo` was referenced but never bound.

Common causes:
- Typo in name
- `foo := value` (assignment) instead of `alloc foo := value` (declaration)
- Missing `import` for a module function

The traceback's last `File` line tells you exactly which expression
referenced `foo` — start there.

### `UndefinedIdentError: foo` *only on native, not on `omg`*

You added a builtin but forgot to add it to the OMG compiler's allowlist
(`cc_builtins` in `compiler.omg`). The Rust frontend has it; the OMG
compiler treats `foo` as a regular function call and fails.

Fix: see [05-extending.md](05-extending.md), Step C of "Adding a builtin."

### `KeyError: "Key 'foo' not found"`

You did `dict["foo"]` (or `dict.foo`) on a dict that doesn't have that key.

If this happens during *transpilation* (`omgcc` errors out with this), it
usually means `native-c.omg` doesn't recognize an instruction or builtin
in the bytecode. Run `--disasm` to see what's there.

### `TypeError: cannot order-compare these values`

`<`/`<=`/`>`/`>=` only work on numbers (any of int/float) and on string-vs-string.
Comparing a string with a number, or any other type combination, errors.

### `ZeroDivisionError`

Self-explanatory. Catchable with try/except if you'd rather handle than crash.

### `IndexError: index N out of range for length L`

List/string index past the end (or negative beyond `-len`).

### `AssertionError: assertion failed`

A `facts` statement evaluated to false.

### Compile-time errors during AOT

If `cc -O2 ...` emits a warning, it's almost always a false positive. The
`-w` flag suppresses them. If `cc` actually *errors*, that's a bug in
`native-c.omg` or `omg_rt.h` — the generated C should always compile cleanly.

To get cc's stderr:

```sh
omg --compile foo.omg foo.omgb
bootstrap/bin/omgcc foo.omgb /tmp/foo.c
cc -O2 /tmp/foo.c -o /tmp/foo -lm   # no -w; see all warnings
```

Save the message and search for it in this repo's commit history; we may
have hit it before.

## Output mismatches

> "It runs, but produces different output than the Rust VM."

### First: confirm it's actually different

```sh
diff <(omg foo.omg 2>&1) \
     <(runtime/target/release/omg foo.omg 2>&1)
```

Both should produce identical output for any program in the corpus.

### Likely causes

- **Float formatting**: very rare — we walk %g precision to match Rust.
  If you see `1e+03` vs `1000.0`, that's the bug; report it.
- **Buffer interleaving**: stdout vs stderr appearing in different orders.
  Check that `omg_rt.h`'s `setvbuf(stdout, NULL, _IOLBF, 0)` is firing
  (look at `main()` in the generated C).
- **Path resolution**: relative paths get joined with `current_dir`. If
  the program runs from a different cwd in one path vs the other, paths
  resolve differently. See [06-runtime.md#file-io-and-path-resolution](06-runtime.md#file-io-and-path-resolution).
- **Args[0]**: when running via `omg foo.omg`, args[0] points to a tempfile,
  not `foo.omg`. AOT binaries see args[0] as their own path. Both differ
  from the Rust VM's "args[0] is the user-typed path" semantics.

## Crashes (segfaults, asserts)

A real segfault in a native binary is a `omg_rt.h` bug or a `native-c.omg`
codegen bug.

### Reproduce minimally

Cut the program down until the crash disappears, then add back the
smallest piece that reintroduces it.

### Check refcount discipline

Most crashes I've debugged in this codebase came from:

- Missing `omg_inc` after pushing an existing value (use-after-free)
- Forgetting `omg_dec` on a popped-and-discarded value (slow leak, then OOM)
- Using `=` instead of `omg_assign` for STORE (leaks the previous occupant)

Run under valgrind:

```sh
valgrind --error-exitcode=1 ./foo
```

Errors point at the C line; map back to the bytecode op via the comments.

### Check the function table

If `CALL_VALUE` segfaults, it's usually because a string callee didn't
resolve to a function pointer. See `omg_lookup_fn` in the generated C.
A NULL means the OMG name isn't in the table.

## Performance

### Profiling

```sh
omg --build foo.omg foo
perf stat ./foo                    # quick wall-time + insn count
perf record ./foo && perf report   # function-level breakdown
```

cc -O2 is doing most of the work; the runtime overhead is mostly:

- Refcount inc/dec
- `omg_emit` formatting (very slow path; don't `emit` in hot loops)
- malloc/free for list/dict/closure ops

### Common perf wins

| Pattern                    | Better                                  |
| -------------------------- | --------------------------------------- |
| `xs := xs + [item]` in a loop | use `omg_list_push` directly (n/a yet — we always re-allocate) |
| Lots of `emit` in a loop   | accumulate to a string, emit once       |
| Deep recursion             | rewrite as `loop` if the call isn't tail-recursive |
| Tail-recursive function    | already optimized — make sure the recursive call is the last thing in the function |

### Sibling-call TCO

cc -O2 turns `return foo(...)` (tail call) into a real `jmp foo`. To verify:

```sh
objdump -d foo | grep -A2 'omg_p1>:'
# Look for "jmp <omg_p2>" rather than "call <omg_p2>"
```

If it's a `call` not a `jmp`, the args may not match `OMG_MAX_ARITY`.
Check that all your tail-call args are passed inline (a0..a31 slots).

### Fixed-point check broke

```
$ omg --verify-omg-vm bootstrap/src/compiler.omg
DIFF: Rust output ≠ OMG-on-OMG-VM output
```

Means you changed the Rust toolchain without updating the OMG side
(or vice versa). Use `diff` on the two intermediate `.omgb` files:

```sh
runtime/target/release/omg --compile bootstrap/src/compiler.omg /tmp/rust.omgb
runtime/target/release/omg --self-hosted-compile bootstrap/src/compiler.omg /tmp/omg.omgb
cmp /tmp/rust.omgb /tmp/omg.omgb && echo "OK" || diff <(xxd /tmp/rust.omgb) <(xxd /tmp/omg.omgb) | head
```

The first diverging byte tells you which instruction's encoding doesn't match.

## When to ask

- Output mismatch between native and Rust VM, with a small repro: file an issue.
- `cc` warning that looks legit: file an issue.
- Anything that says `VmInvariant`: definitely a bug, file an issue.
- "How do I do X in OMG": [03-language-tour.md](03-language-tour.md) first;
  if you can't find it there, check the [examples/](../../examples/) directory.

## See also

- [05-extending.md](05-extending.md) — when the bug is "missing feature"
- [06-runtime.md](06-runtime.md) — when the bug is in the C runtime
