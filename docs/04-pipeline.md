# 04 · The compilation pipeline

What actually happens between `foo.omg` and `./foo`. We'll trace one tiny
program through every stage and show you how to peek at each.

## The trace program

```omg
;;;omg
alloc x := 7
emit x * 6
```

Save as `/tmp/trace.omg`.

## The pipeline at a glance

```
                   ┌────────────┐         ┌─────────┐
   foo.omg ───────▶│  compiler  │────────▶│  .omgb  │
   (source)        └────────────┘         │bytecode │
                                          └────┬────┘
                                               │
                          ┌────────────────────┴────────────────────┐
                          │                                         │
                          ▼  (interpreted)                          ▼  (built)
                   ┌────────────┐                            ┌─────────────┐
                   │     vm     │ ──▶ output                 │ native-x64  │◀── the runtime
                   └────────────┘                            └──────┬──────┘    (OMG source,
                                                                    │           compiled the
                                                              ┌─────▼─────┐     same way)
                                                              │ executable│ ──▶ output
                                                              └───────────┘
```

The `vm` and `native-x64` paths are **alternatives**, not sequential.
From bytecode, you pick one. There are three artefacts you can
inspect: source, bytecode (`.omgb`) and the executable. Tokens and AST
exist in memory only, and so does everything inside the back end:
there is no assembly file and no object file in between.

`--target c` swaps the right-hand box for `native-c`, which writes a C
file for `cc`. That route is described [further down](#the-c-route).

## Stage 0 — Source

```omg
;;;omg
alloc x := 7
emit x * 6
```

Plain UTF-8 text. The `;;;omg` header is conventional and stripped by the
lexer if present (see [03-language-tour.md](03-language-tour.md#the-header)).
The rest is OMG syntax.

## Stage 1 — Tokens

The lexer turns the source into a stream of typed tokens.

```
[KEYWORD "alloc"] [IDENT "x"] [OP ":="] [INT 7] [NEWLINE]
[KEYWORD "emit"]  [IDENT "x"] [OP "*"]  [INT 6] [NEWLINE]
```

Tokens aren't a serialized artefact you can dump — they live in memory only.
The lexer is the first part of [src/compiler.omg](../src/compiler.omg).

## Stage 2 — AST

Tokens get parsed into an abstract syntax tree:

```
Program
├── Decl("x", Int(7))
└── Emit(BinOp("*", Var("x"), Int(6)))
```

Also memory-only. The parser is the second part of `compiler.omg`.

## Stage 3 — Bytecode (`.omgb`)

The compiler walks the AST and emits a flat instruction stream:

```
PushInt(7)
StoreLocal("x")
Load("x")
PushInt(6)
Mul
Emit
Halt
```

This is what gets written to `.omgb`. Inspect any compiled program:

```sh
omg --compile /tmp/trace.omg /tmp/trace.omgb
omg --disasm /tmp/trace.omgb
```

You'll see the function table at the top (none here), then the code:

```
0000  PushInt(7)
0001  StoreLocal("x")
0002  Load("x")
0003  PushInt(6)
0004  Mul
0005  Emit
0006  Halt
```

The `.omgb` file format is read by
[src/bytecode.omg](../src/bytecode.omg) and written by
`write_bytecode` in `compiler.omg` — header magic
`OMGB`, version `0x000200`, source-file table, function table (each
function carries a `source_file_idx`), instruction stream, then a
per-instruction source map (`(file_idx, line)` parallel to the code).
The last two sections are what give runtime errors their
`File "foo.omg", line 12, in <fn>` traceback context.

## Stage 4 (option A) — Run via VM

If you `omg /tmp/trace.omg`, this is where execution happens. The VM walks
the bytecode and updates a value stack:

```
PC  Instr           Stack after
─── ─────────────── ──────────────────
0   PushInt(7)      [Int(7)]
1   StoreLocal("x") []          # x := 7 in env
2   Load("x")       [Int(7)]
3   PushInt(6)      [Int(7), Int(6)]
4   Mul             [Int(42)]
5   Emit            []          # prints "42"
6   Halt            []          # done
```

## Stage 4 (option B) — Machine code

If you ran `omg --build /tmp/trace.omg /tmp/trace` instead,
[`native-x64.omg`](../src/backends/native-x64.omg) reads the same
bytecode and writes the executable. It does four things, all in
memory:

**1. It compiles your program ("managed mode").** Each proc becomes
one stretch of x86-64. A value is two machine words, a tag and a
payload (tag 0 is an integer, 8 a string, and so on), and heap objects
are reference counted. The bytecode is a stack machine, but the back
end does not push and pop at run time: it keeps a *virtual* stack of
where each operand currently is (a constant, a variable not read yet,
the register pair RAX:RDX), and only emits an instruction when a value
actually has to exist. For the trace program that means roughly:

```
PushInt(7)        nothing emitted: "the constant 7" goes on the virtual stack
StoreLocal("x")   mov  qword [x], 7          ; payload
                  mov  qword [x+8], 0        ; tag: Int
Load("x")         nothing emitted: "the variable x"
PushInt(6)        nothing emitted: "the constant 6"
Mul               cmp  qword [x+8], 0        ; is x an Int?
                  jne  slow                  ; no: call rt_mul, placed after the body
                  mov  rax, [x]
                  imul rax, rax, 6
                  jo   slow                  ; overflow: rt_mul raises
Emit              call rt_emit
Halt              leave the main body; the runtime exits with 0
```

Integer arithmetic, shifts and comparisons are done in line like this
whenever the tags allow, with a call into the runtime as the fallback.
So are the two things a loop over numbers does most: reading
`list[i]`, and writing `list[i] := v` when neither the new value nor
the one it replaces is a heap object. In line that is a check that the
container is a list and the index an Int inside it, then a load or a
store. A [byte list](03-language-tour.md#byte-lists) gets the same
treatment, with a one-byte load or store and a check that the value
stored is from 0 to 255; that code sits after the proc's body, a jump
away, so that the list case stays the straight path. A negative
index, a dict, a string or an error takes the call.
Anything bigger (string concatenation, dictionaries, builtins) is
always a call into the runtime.

**Each proc is compiled twice.** Those checks cost more than the work
they guard: `acc := acc + xs[i]` in a loop asks, every time round,
whether `acc` is an Int, whether `i` is, whether `xs` is a list or a
byte list and whether the element is an Int. The answers are nearly always the same
as last time. So the back end writes two versions of every proc. The
*careful version* is the code described so far. The *fast version* is
the one a call enters, and it guesses:

- that the operands of arithmetic, shifts, `//`, `%` and the ordering
  comparisons are Ints;
- that `xs[i]`, where the element goes straight into arithmetic or
  into another list, is an Int at a place inside a list or a byte
  list.

A guess is tested once and then remembered for as long as it stays
true. A local variable found to hold an Int is not tested again until
something of unknown kind is assigned to it, and the result of Int
arithmetic is an Int without looking. A loop is treated as a whole:
the variables it computes with but never assigns are tested once on
the way in, and what is known at the top of the loop is checked on the
way back round only where the body could have changed it. With the
kinds known, storing an Int over an Int is one instruction, and `//`
and `%` by a constant are a shift, a mask or a divide in line.

One thing is asked every time and not remembered: whether a container
is a list or a byte list. The same proc is often handed one and then
the other (a filter given a picture's bytes on one call and a list of
wider numbers on the next), so the answer at the top of a loop says
little about the next call, and the question is one compare of a tag
that is already in the frame.

When a guess is wrong (a Float turns up, an index is off the end, an
addition does not fit in 64 bits) the fast version jumps into the
careful version *at the same point of the same instruction*, and the
careful version does whatever the language says: computes with the
Float, raises the error. Both versions use the same frame, and the
fast version keeps every variable in memory exactly as the careful
one would, so there is nothing to convert on the way. The careful
version returns to the fast one at the top of the next loop, if what
that loop takes for granted holds by then.

This is the loop of `total` in the fast version, whole:

```
proc total(xs) {                 top:  cmp  [xs.tag], LIST     ; length(xs)
    alloc acc := 0                     je   len
    alloc i := 0                       cmp  [xs.tag], BYTES
    loop i < length(xs) {              jne  careful
        acc := acc + xs[i]       len:  mov  rcx, [xs]
        i := i + 1                     mov  rax, [rcx+8]
    }                                  cmp  [i], rax
    return acc                         jge  done
}                                      cmp  [xs.tag], LIST     ; xs[i]
                                       jne  bytes
                                       mov  rcx, [xs]
                                       mov  rsi, [i]
                                       cmp  rsi, [rcx+8]       ; i inside the list?
                                       jae  careful
                                       mov  rdi, [rcx+24]      ; the elements
                                       add  rsi, rsi           ; two words each
                                       cmp  qword [rdi+rsi*8+8], 0
                                       jne  careful            ; the element is not an Int
                                       mov  rax, [rdi+rsi*8]
                                 sum:  add  rax, [acc]
                                       jo   careful            ; does not fit
                                       mov  [acc], rax
                                       mov  rax, [i]
                                       add  rax, 1
                                       jo   careful
                                       mov  [i], rax
                                       jmp  top

                               bytes:  cmp  [xs.tag], BYTES    ; after the proc's body
                                       jne  careful
                                       mov  rcx, [xs]
                                       mov  rsi, [i]
                                       cmp  rsi, [rcx+8]
                                       jae  careful
                                       mov  rdi, [rcx+24]
                                       movzx rax, byte [rdi+rsi]   ; always an Int
                                       jmp  sum
```

That is 25 instructions a turn for a list and 27 for a byte list,
where the careful version runs 44. What is left cannot go without
knowing more than one proc's code can tell: that `i` stays inside the
list, that every element is an Int, that the sum fits.

How much time that saves depends on the program. Measured against
the careful version alone, a loop like this one runs 1.7 times as
fast, a matrix multiplication 1.6 times, and the H.264 decoder in
`tools/video/` takes a quarter less time. A big program gains less
than a small loop because checks that never fail cost a modern
processor little ([10-video.md](10-video.md#how-fast-it-is)).

Three things follow for anyone writing OMG that has to be quick:

- Put a hot loop in a proc. Code at the top level of a file gets only
  the careful version, because any proc may assign a global.
- A variable that holds an Int everywhere in a loop is cheap. One that
  is sometimes a Float, or a list of Floats, runs the loop in the
  careful version: correct, and no slower than before, but no faster.
- Programs are larger, since procs are there twice: `omg` itself went
  from 1.5 MB to 2.4 MB. Setting `mg_opt := false` in `native-x64.omg`
  leaves the fast versions out.

The two versions must agree on everything a program can observe, and
`tests/conformance/numbers.omg` exists to catch them disagreeing: its
loops are given Floats, strings, lists, nothing, zero divisors and
indexes off the end, part of the way through.

**2. It compiles the runtime ("raw mode").** The runtime is OMG source
too ([`src/runtime/x64/`](../src/runtime/x64/)), but in a restricted
dialect where every value is one untagged machine word and memory and
the operating system are reached through reserved names such as
`__peek64` and `__syscall`. `omg` compiles those files to bytecode
with the ordinary compiler on every build (about 0.1 s), and the back
end gives that bytecode its low-level meaning. Only the runtime procs
your program can reach are kept, which is why a program that never
opens a socket carries no socket code.

**3. It links.** Jumps and calls were emitted against numbered labels,
and references to data against section offsets. Once all the code
exists, the distances are filled in. Every address is relative to the
instruction that uses it, so there are no relocations.

**4. It wraps the result** as an ELF file (Linux) or a PE file
(Windows): a header, the code, the data, and on Windows a table of the
`kernel32.dll` functions the program calls. On Linux there is nothing
to import: the runtime makes system calls directly.

```sh
omg --build /tmp/trace.omg /tmp/trace
/tmp/trace
# → 42
```

The file is a static executable of about 22 KB, nearly all of it the
part of the runtime that `emit` and integer formatting need.

There is no optimiser in the classical sense: no register allocation
across statements, no inlining of procs. What makes the code quick is
the virtual stack, the in-line integer paths, the fast version of each
proc, and a pre-pass over the bytecode that recognises `and` / `or` /
`not` patterns and threads jumps. On the project's benchmarks it is several times faster than the
same program through C, largely because its calls and its integer
paths are cheaper.

## The C route

`--target c` (the default on macOS and ARM) hands the same bytecode to
[`native-c.omg`](../src/backends/native-c.omg) instead. You can see
what it writes:

```sh
bin/omg --emit-c /tmp/trace.omgb /tmp/trace.c
head -200 /tmp/trace.c
```

(`--emit-c` takes the `.omg` source directly too. `--emit-js` is the
same thing through the JavaScript back end.)

You'll see `omg_rt.h` inlined at the top, then runtime-injected
globals (`args`, `module_file`, `current_dir`), then your program — one
straight-line block of C:

```c
int main(int argc, char **argv) {
    setvbuf(stdout, NULL, _IOLBF, 0);
    Value stack[1024];
    int sp = 0;
    /* ... cwd / args / module_file setup ... */

    stack[sp++] = omg_int(7LL);                              // PushInt(7)
    omg_assign(&v_x, stack[--sp]);                           // StoreLocal("x")
    { Value v = v_x; omg_inc(v); stack[sp++] = v; }          // Load("x")
    stack[sp++] = omg_int(6LL);                              // PushInt(6)
    { Value b = stack[--sp]; Value a = stack[--sp];
      Value r = omg_mul(a, b); omg_dec(a); omg_dec(b);
      stack[sp++] = r; }                                     // Mul
    { Value v = stack[--sp]; omg_emit(v); omg_dec(v); }      // Emit
    return 0;                                                // Halt
}
```

(The `// ...` annotations are added here for clarity; the generated C
doesn't include them.) Each bytecode op becomes a few lines of C, and
`cc` does the rest:

```sh
cc -O2 -w /tmp/trace.c -o /tmp/trace -lm
```

## How long does each stage take?

Measured on one ordinary x86-64 Linux machine; the proportions matter
more than the numbers.

For an 80-line program (`examples/maze_solver.omg`):

| Stage                      | Time    | Output size |
| -------------------------- | ------- | ----------- |
| `.omg` → `.omgb`           | ~4 ms   | 4.5 KB      |
| `omg --build`              | ~0.2 s  | 42 KB       |
| `omg --build --target c`   | ~1.1 s  | 64 KB       |

For the 3000-line `compiler.omg`:

| Stage                      | Time    | Output size |
| -------------------------- | ------- | ----------- |
| `.omg` → `.omgb`           | ~0.1 s  | 149 KB      |
| `omg --build`              | ~0.35 s | 431 KB      |
| `--emit-c`, then `cc -O3`  | ~23 s   | 924 KB      |

And for the whole toolchain, `omg --build src/omg.omg`: about 1.4
seconds, against more than two minutes through C. Most of the 0.2 s
floor of a small build is compiling the runtime's source, which
happens every time.

## Why bytecode at all?

You might wonder why the back ends don't work from the AST. Two
reasons:

1. **The VM** wants bytecode for dispatch, and running a program with
   no build step is the everyday case.
2. **The back ends** are much simpler to write against bytecode (a flat
   instruction stream) than against an AST (a tree with arbitrary
   shape). One compiler feeds four consumers: the VM, x86-64, C and
   JavaScript.

Bytecode is also a useful distribution format: you can ship `.omgb`
files that `omg` runs with no source required.

## Where each stage lives

| Stage          | Where                             |
| -------------- | --------------------------------- |
| Lexer, parser, code generator, `.omgb` writer | `src/compiler.omg` |
| Bytecode VM    | `src/vm.omg`                      |
| Bytecode reader for the back ends | `src/bytecode.omg` |
| Bytecode → x86-64, ELF, PE | `src/backends/native-x64.omg` |
| Bytecode → C   | `src/backends/native-c.omg`       |
| Bytecode → JS  | `src/backends/native-js.omg`      |
| Scope analysis shared by the back ends | `src/backends/backend.omg` |
| Runtime for machine code | `src/runtime/x64/*.omg` |
| C runtime      | `src/runtime/omg_rt.h`            |
| JS runtime     | `src/runtime/omg_rt.js`           |
| The `omg` command | `src/omg.omg`                  |

A language feature usually touches several of these at once — see
[05-extending.md](05-extending.md).

## Read next

- [05-extending.md](05-extending.md) — add a builtin or opcode and trace it
  through every layer
- [06-runtime.md](06-runtime.md) — what a built program carries with it
