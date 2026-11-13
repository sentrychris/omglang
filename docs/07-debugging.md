# 07 · Debugging

When OMG isn't doing what you expect. Diagnostic tools first, common
problems second.

## Tools at each stage

| Stage         | How to peek                                          |
| ------------- | ---------------------------------------------------- |
| Source        | `cat foo.omg`                                        |
| Tokens / AST  | (no dump tool yet — read the parser if needed)       |
| Bytecode      | `omg --disasm foo.omgb` (or `foo.omg`)               |
| The same program another way | `omg foo.omg` against `omg --build foo.omg`, `--target c`, `--emit-js` |
| C source      | `omg --emit-c foo.omg foo.c`                         |
| JS source     | `omg --emit-js foo.omg foo.js`                       |
| Native binary | gdb, strace, `objdump`                               |

The most useful tool is the second-last idea in that table turned into
a habit: **run the program another way**. There are four ways to run
any OMG program, they share one compiler, and they are required to
agree. If two of them disagree, the bug is in one of the back ends or
runtimes and you already know which side to look at. If they all agree
with each other and not with you, the bug is in the program or in the
compiler.

### Disassembling bytecode

```sh
omg --compile foo.omg foo.omgb
omg --disasm foo.omgb
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

### Looking at a machine-code executable

`omg --build` writes the executable directly: there is no assembly
listing and no symbol table. What you have:

```sh
strace ./foo                      # Linux: every system call the program makes
gdb ./foo                         # works; no symbols, so break on addresses
objdump -D -b binary -m i386:x86-64 foo | less    # raw disassembly
```

`strace` is the quick one. An executable built by `omg` makes its
system calls itself, so the trace is exactly what the runtime did:
which file it tried to open, what it wrote, how it exited.

The code is laid out with the entry code first, then the program's
procs, then the runtime procs it uses. In practice, when a built
program misbehaves and the interpreted one does not, it is quicker to
shrink the program (see [Crashes](#crashes-segfaults-asserts)) than to
read disassembly.

### Inspecting generated C

The C route gives you something readable for the same bytecode:

```sh
omg --emit-c foo.omg /tmp/foo.c     # a .omgb works as input too
less /tmp/foo.c
```

The runtime header is at the top (a few thousand lines) — skip past
it. Each generated proc starts with:

```c
static Value omg_pN(Value *captured, int cap_count, int argc,
                    Value omg_a0, Value omg_a1, ...) {
```

Each bytecode instruction is a small block — usually a `{ ... }` scope
that pops, computes, and pushes. There are no per-instruction comments in
the generated C, so to map a C line back to a bytecode op, run `--disasm`
in parallel and match patterns by hand. A program built with
`--target c` has symbols, so `gdb`, `valgrind` and `perf` all work on
it in the usual way.

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
- **Empty source map** (hand-built bytecode) falls back to a bare
  `Kind: message` line with no `File ...` prefix.

### `UndefinedIdentError: foo`

The name `foo` was referenced but never bound.

Common causes:
- Typo in name
- `foo := value` (assignment) instead of `alloc foo := value` (declaration)
- Missing `import` for a module function

The traceback's last `File` line tells you exactly which expression
referenced `foo` — start there.

Every way of running raises this, built executables included. A name that is only declared on a
path that did not run, such as an `alloc` inside an `if`, is undefined
too.

### `SyntaxError: 'foo' is used in a nested proc before the enclosing proc declares it`

A nested proc captures the variables that exist where it is written.
`foo` is declared by the enclosing proc, but further down, so the
closure could never see it. Move the `alloc foo` (or the sibling
`proc foo`) above the nested proc.

### `RuntimeError: maximum recursion depth exceeded`

A built executable (or a program transpiled to JavaScript) ran out of
native stack. It is catchable. A machine-code executable has a 64 MiB
stack, good for several hundred thousand nested calls; the VM keeps
its frames on the heap and has no such limit. To go deeper in a built
program, turn the recursion into a loop or a tail call (`return
f(...)` replaces the frame instead of adding one).

### `builtin 'foo' ...` errors, or `UndefinedIdentError: foo`, for a builtin you just added

One of the places a builtin has to be registered was missed: the
compiler's allowlist (`cc_builtins` in `compiler.omg`), the runtime's
`rt_bi_init` table, or one of the back ends. Which symptom you get
depends on which. [05-extending.md](05-extending.md) lists all of
them under "Adding a new builtin".

### `KeyError: "Key 'foo' not found"`

You did `dict["foo"]` (or `dict.foo`) on a dict that doesn't have that key.

If this happens during *code generation* (`omg --build` or `--emit-c`
errors out with this), it usually means the back end doesn't recognize
an instruction or builtin in the bytecode. Run `--disasm` to see what's
there.

### `TypeError: cannot order-compare these values`

`<`/`<=`/`>`/`>=` only work on numbers (any of int/float) and on string-vs-string.
Comparing a string with a number, or any other type combination, errors.

### `ZeroDivisionError`

Self-explanatory. Catchable with try/except if you'd rather handle than crash.

### `IndexError: index N out of range for length L`

List/string index past the end (or negative beyond `-len`).

### `AssertionError: assertion failed`

A `facts` statement evaluated to false.

### Errors from `omg --build` itself

A message starting `native-x64:` means the back end met bytecode it
could not translate. For your own programs that is a bug in the back
end; report it with the program. For the runtime's source
(`src/runtime/x64/`) it usually means raw-mode rules were broken: a
list, a `try`, an unknown `__name`, or a proc with 500 or more locals.

With `--target c`, if `cc` emits a warning it's almost always a false
positive, and `-w` suppresses them. If `cc` actually *errors*, that's
a bug in `native-c.omg` or `omg_rt.h` — the generated C should always
compile cleanly. To get cc's stderr:

```sh
omg --emit-c foo.omg /tmp/foo.c
cc -O2 /tmp/foo.c -o /tmp/foo -lm   # no -w; see all warnings
```

## Output mismatches

> "It runs, but the built program prints something different."

### First: confirm it's actually different

```sh
omg --build foo.omg /tmp/foo
diff <(omg foo.omg 2>&1) <(/tmp/foo 2>&1)
```

Both must produce identical output for any program. Add the third
opinion to see which side is wrong:

```sh
omg --build foo.omg /tmp/foo_c --target c && /tmp/foo_c
omg --emit-js foo.omg /tmp/foo.js && node /tmp/foo.js
```

### Likely causes

- **A real difference between runtimes.** Each runtime implements the
  builtins separately. Reduce the program to the one call that
  differs; that is the bug report, and the line to add to
  `tests/conformance/`.
- **Float formatting**: all runtimes print the shortest form that
  reads back exactly. `pow` and the trigonometric functions can differ
  in the last digit between the machine-code runtime (x87) and a C
  library.
- **Path resolution**: relative paths get joined with `current_dir`. If
  the program runs from a different cwd in one path vs the other, paths
  resolve differently. See [06-runtime.md#file-io-and-path-resolution](06-runtime.md#file-io-and-path-resolution).
- **`args[0]`**: under `omg foo.omg` it is the absolute path of
  `foo.omg`; in a built executable it is the path the executable was
  started with.

## Crashes (segfaults, asserts)

A segfault in a built program is a bug in a back end or its runtime,
never in your program: OMG code cannot touch memory directly.

### Reproduce minimally

Cut the program down until the crash disappears, then add back the
smallest piece that reintroduces it. Run each step interpreted too
(`omg foo.omg`), and with `--target c` if you have a C compiler: a
crash that happens only one way points at that back end.

### In the machine-code runtime

Most crashes there come from reference counts:

- a value used after its last count was given back (use-after-free),
- a count given back twice,
- a runtime proc that kept an argument without taking a count.

`strace` shows whether the program died in the middle of a system
call or between two. For a bug in a runtime proc, the fastest probe is
often a temporary `os_die("got here")` in the raw-mode source and a
rebuild, which takes a fraction of a second.

### In a C build

The same mistakes have C names:

- Missing `omg_inc` after pushing an existing value (use-after-free)
- Forgetting `omg_dec` on a popped-and-discarded value (slow leak, then OOM)
- Using `=` instead of `omg_assign` for STORE (leaks the previous occupant)

Run under valgrind:

```sh
omg --build foo.omg foo --target c
valgrind --error-exitcode=1 ./foo
```

If `CALL_VALUE` segfaults, it's usually because a string callee didn't
resolve to a function pointer. See `omg_lookup_fn` in the generated C.
A NULL means the OMG name isn't in the table.

## Performance

### Measuring

```sh
omg --build foo.omg foo
time ./foo
perf stat ./foo                    # wall time + instruction count
```

A machine-code executable has no symbols, so `perf report` shows
addresses, not names. For a function-level profile, build the same
program with `--target c` and profile that; the hot spots are usually
the same, though not always.

Where the time goes in a built program:

- Calls into the runtime: string building, dictionaries (attribute
  access included), slices, indexing anything but a list by a number
- Reference count updates
- `emit` (formatting and a system call per line; don't `emit` in hot loops)

Integer arithmetic, shifts, comparisons, local variables, reading and
writing the elements of a list of numbers, and calls between your own
procs are compiled in line and are cheap.

### Common perf wins

| Pattern                    | Better                                  |
| -------------------------- | --------------------------------------- |
| Lots of `emit` in a loop   | accumulate to a string, emit once       |
| Deep recursion             | rewrite as `loop` if the call isn't tail-recursive |
| Tail-recursive function    | already optimized — make sure the recursive call is the last thing in the function |
| A dict used as an array    | use a list; integer keys are turned into strings |

### The toolchain no longer rebuilds to the same bytes

```
$ tests/parity.sh
  ✗ a second rebuild is byte-identical
```

`omg` building its own source did not reproduce itself. Something in
the chain compiler → back end → runtime now behaves differently when
built by itself than when built by its predecessor. To find it:

```sh
scripts/build.sh && cp bin/omg /tmp/omg-a
scripts/build.sh && cmp bin/omg /tmp/omg-a
```

If they differ, compare the two compilers on one input, which narrows
it to the compiler or to the back end:

```sh
/tmp/omg-a --compile src/compiler.omg /tmp/a.omgb
bin/omg    --compile src/compiler.omg /tmp/b.omgb
cmp /tmp/a.omgb /tmp/b.omgb && echo "the compiler is fine: look at the back end"
```

A frequent cause is anything that depends on the order of a dict's
keys, or on an address.

### `bin/omg` is broken and cannot rebuild itself

`bin/` is checked in, so the previous toolchain is one command away:

```sh
git checkout HEAD -- bin/
scripts/build.sh
```

`build.sh` only replaces `bin/omg` after both stages of the build have
succeeded, so this is rarely needed.

## When to ask

- Output mismatch between two ways of running, with a small repro: file an issue.
- `cc` warning that looks legit: file an issue.
- Anything that says `VmInvariant`: definitely a bug, file an issue.
- "How do I do X in OMG": [03-language-tour.md](03-language-tour.md) first;
  if you can't find it there, check the [examples/](../examples/) directory.

## See also

- [05-extending.md](05-extending.md) — when the bug is "missing feature"
- [06-runtime.md](06-runtime.md) — when the bug is in a runtime
