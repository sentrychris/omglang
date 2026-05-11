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
                          ▼  (interpreted)                          ▼  (transpiled)
                   ┌────────────┐                            ┌─────────────┐
                   │     vm     │ ──▶ output                 │  native-c   │
                   └────────────┘                            └──────┬──────┘
                                                                    │
                                                              ┌─────▼─────┐
                                                              │   foo.c   │
                                                              └─────┬─────┘
                                                                    │ cc -O2
                                                              ┌─────▼─────┐
                                                              │    ELF    │ ──▶ output
                                                              └───────────┘
```

The `vm` and `native-c` paths are **alternatives**, not sequential. From
bytecode, you pick one. There are five distinct artefacts you can inspect:
source, bytecode (`.omgb`), C source (`.c`), ELF, and the runtime output.
Tokens and AST exist in memory only.

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
The Rust frontend's lexer is at [runtime/src/lexer.rs](../../runtime/src/lexer.rs);
the OMG-in-OMG version is the first half of [bootstrap/src/compiler.omg](../../bootstrap/src/compiler.omg).

## Stage 2 — AST

Tokens get parsed into an abstract syntax tree:

```
Program
├── Decl("x", Int(7))
└── Emit(BinOp("*", Var("x"), Int(6)))
```

Also memory-only. Parser code: [runtime/src/parser.rs](../../runtime/src/parser.rs)
and the second half of `compiler.omg`.

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
runtime/target/release/omg --compile /tmp/trace.omg /tmp/trace.omgb
runtime/target/release/omg --disasm /tmp/trace.omgb
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

The `.omgb` file format is documented in
[runtime/src/bytecode.rs](../../runtime/src/bytecode.rs) — header magic
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

## Stage 4 (option B) — C source (`.c`)

If you took the AOT path instead, `native-c.omg` reads the same bytecode
and emits C. You can see the output:

```sh
bootstrap/bin/omgcc /tmp/trace.omgb /tmp/trace.c
head -200 /tmp/trace.c
```

You'll see `omg_rt.h` inlined at the top (~1700 lines), then runtime-injected
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
doesn't include them.) Each bytecode op becomes a few lines of C. There's
no opcode dispatch loop at runtime — it's been "unrolled" at transpile time.

## Stage 5 — ELF (`./foo`)  *(continues option B only)*

```sh
cc -O2 -w /tmp/trace.c -o /tmp/trace -lm
/tmp/trace
# → 42
```

`cc -O2` does all the heavy lifting: register allocation, sibling-call
optimization (so OMG tail calls become real `jmp` instructions),
dead-code elimination, constant propagation. The `-w` flag suppresses cc
warnings (mostly false positives around setjmp/longjmp). The resulting ELF
is a normal native binary.

## How long does each stage take?

For a 50-line program on a modern laptop:

| Stage             | Time   | Output size       |
| ----------------- | ------ | ----------------- |
| `.omg` → `.omgb`  | ~5 ms  | ~1 KB             |
| `.omgb` → `.c`    | ~20 ms | ~70 KB            |
| `.c` → ELF        | ~500 ms| ~30 KB            |
| **Total AOT**     | ~525 ms| 30 KB binary      |

For larger programs (the ~2100-line `compiler.omg`):

| Stage             | Time   | Output size       |
| ----------------- | ------ | ----------------- |
| `.omg` → `.omgb`  | ~80 ms | 53 KB             |
| `.omgb` → `.c`    | ~250 ms| 740 KB            |
| `.c` → ELF        | ~7 s   | 432 KB            |

The cc step dominates. That's an unavoidable cost of going through C, but
you only pay it for AOT — `omg foo.omg` skips it entirely.

## Why bytecode at all?

You might wonder why we don't go straight from AST to C. Two reasons:

1. **The Rust VM** wants bytecode for fast dispatch. It's the production
   runtime; bytecode is its native input.
2. **`native-c.omg`** is much simpler to write against bytecode (a flat
   instruction stream) than against an AST (a tree with arbitrary shape).
   The bytecode normalizes everything into push/pop/op operations.

Bytecode is also a useful distribution format: you can ship `.omgb` files
that any OMG runtime can execute, no source required.

## Where each stage lives

| Stage          | Rust impl                       | OMG impl                          |
| -------------- | ------------------------------- | --------------------------------- |
| Lexer          | `runtime/src/lexer.rs`          | first half of `bootstrap/src/compiler.omg` |
| Parser         | `runtime/src/parser.rs`         | second half of `bootstrap/src/compiler.omg` |
| Compiler       | `runtime/src/compiler.rs`       | rest of `bootstrap/src/compiler.omg`  |
| Bytecode VM    | `runtime/src/vm.rs` + `vm/ops_*.rs` | `bootstrap/src/vm.omg`            |
| Bytecode → C   | (none)                          | `bootstrap/src/native-c.omg`          |
| C runtime      | (none)                          | `bootstrap/src/omg_rt.h`              |

The Rust and OMG implementations of stages 1-4 are kept in lockstep — see
[05-extending.md](05-extending.md).

## Read next

- [05-extending.md](05-extending.md) — add a builtin or opcode and trace it
  through every layer
- [06-runtime.md](06-runtime.md) — what `omg_rt.h` actually does at runtime
