# How OMG actually runs your script

A developer-oriented walk through what happens when you type `omg foo.omg`.
The high-level pitch is simple ("OMG is self-hosted") but the moving
parts are worth knowing if you're going to hack on the runtime, the
compiler, or the bootstrap.

> This doc covers the **bytecode-VM path** — how the Rust runtime
> compiles and executes OMG. There's also a **native-compilation path**
> ([`docs/native/`](native/)) that turns OMG into standalone ELF
> binaries with no Rust runtime. Both paths share the same compiler
> and bytecode format; they're alternative backends.

## The cast

There are three pieces of code involved. They live in different places
and run at different times:

| Piece                       | Lives in                          | Written in | Runs on  |
| --------------------------- | --------------------------------- | ---------- | -------- |
| **Rust frontend** (stage-0) | [`runtime/src/{lexer,parser,compiler}.rs`](../runtime/src/) | Rust       | Host CPU |
| **OMG compiler**  (stage-1) | [`bootstrap/src/compiler.omg`](../bootstrap/src/compiler.omg)   | OMG        | OMG VM   |
| **OMG VM**                  | [`runtime/src/vm.rs`](../runtime/src/vm.rs) and friends | Rust       | Host CPU |

The VM only knows how to execute `.omgb` bytecode. Both compilers exist
to *produce* `.omgb` bytecode — one in Rust, one in OMG. They produce
identical output for the same input (this is the
[fixed-point property](#the-fixed-point-check)), so the choice between
them is purely a matter of cost.

## Build time: the bootstrap

When you `cargo build` the runtime, this happens in [build.rs](../runtime/build.rs):

```text
bootstrap/src/compiler.omg ──► [Rust frontend] ──► bootstrap/src/compiler.omgb
                                              (committed? no — built fresh
                                               on every cargo build)
```

The output file is then **embedded into the runtime binary**:

```rust
// runtime/src/main.rs
const SELF_HOSTED_COMPILER: &[u8] =
    include_bytes!("../../bootstrap/src/compiler.omgb");
```

So the binary you ship contains, statically baked in, the compiled
bytecode of an OMG compiler written in OMG. That's the artifact every
non-`--rust` invocation will reach for at runtime.

The Rust frontend is the "stage-0" compiler in the textbook
self-hosting sense: it exists to bootstrap the real compiler, and isn't
the one a self-hosted run actually uses.

## Run time: `omg foo.omg`

Here's the default path, step by step. No `--rust` flag.

```text
   ┌─────────────────────────────┐
   │ 1. Rust binary starts.      │
   │    Reads SELF_HOSTED_COMPILER (the embedded .omgb)
   │    and parses it into (code, funcs).
   └──────────────┬──────────────┘
                  │
                  ▼
   ┌─────────────────────────────┐
   │ 2. The OMG VM runs the      │
   │    OMG-written compiler.    │
   │                             │
   │    args = ["<embedded>",    │
   │            <your-script>,   │
   │            <tmp-output>]    │
   │                             │
   │    The compiler reads the   │
   │    user source, lexes /     │
   │    parses / lowers it, and  │
   │    writes the bytecode to   │
   │    a temp `.omgb` via OMG's │
   │    own `file_write` builtin.│
   └──────────────┬──────────────┘
                  │
                  ▼
   ┌─────────────────────────────┐
   │ 3. Host reads the temp      │
   │    file back, parses it     │
   │    into (code, funcs),      │
   │    deletes the temp file.   │
   └──────────────┬──────────────┘
                  │
                  ▼
   ┌─────────────────────────────┐
   │ 4. The OMG VM runs *your*   │
   │    program with             │
   │    args = [foo.omg, ...].   │
   └─────────────────────────────┘
```

Two VM invocations in a single process. The first one runs the
OMG-written compiler; the second runs your program. Same VM
implementation, different `(code, funcs)` loaded.

The relevant glue lives in
[`self_hosted_compile`](../runtime/src/main.rs) — about 30 lines of
plumbing. The bulk of the work — lexing, parsing, name mangling, import
resolution, bytecode emission — all happens in OMG, on the VM.

### Why the temp file?

Step 2 → step 3 communicates via `/tmp/omg-stage1-<pid>-<rand>.omgb`
rather than threading bytes through memory directly. The reason is
boring: the OMG compiler is a regular OMG program. It writes its output
with `file_write`, the same way `tools/wc.omg` writes its results. The
Rust host invokes it the same way you invoke any OMG program — by
passing `args` and waiting for it to finish. There's no special
"return the bytecode buffer" channel.

This is one syscall round-trip per `omg <script>` invocation. It's not
a hot path; the compile time itself dominates.

## `--rust`: skip the self-hosted layer

Pass `--rust` and the runtime takes the obvious shortcut:

```text
   your .omg source
        │
        ▼
   [Rust frontend in compiler.rs]
        │
        ▼
   your .omgb (in memory)
        │
        ▼
   [OMG VM] ──► your program
```

One VM invocation, no temp file, no compiler-on-VM step. Roughly
**1000–2000× faster** to compile, because all the lexing/parsing/
lowering work happens in compiled Rust code instead of interpreted
bytecode.

The output is byte-identical to what the self-hosted path would have
produced — so `--rust` is purely a performance/dogfooding choice, not a
behavior choice.

## The other commands

| Command | Frontend | Notes |
| ------- | -------- | ----- |
| `omg <script>`                              | self-hosted | Default for `.omg`. `.omgb` skips compilation entirely. |
| `omg --rust <script>`                       | Rust        | Same result, much faster compile. |
| `omg <script.omgb>`                         | none        | Already bytecode; just runs it. |
| `omg --compile <in> [<out>]`                | Rust        | Stays Rust by default. Compiling is interactive iteration; the slow path would hurt. |
| `omg --self-hosted-compile <in> [<out>]`    | self-hosted | Explicit opt-in to the self-hosted compiler for AOT compile. |
| `omg --verify-self-hosted <file>`           | both        | Compiles `<file>` with both compilers and asserts byte-identical output. The self-hosting fixed-point check. |
| `omg --verify-omg-vm <file>`                | both, plus the OMG-written VM | Triple-meta fixed-point check: compares the Rust-frontend output against running the OMG compiler **on the OMG VM** (`bootstrap/src/vm.omg`, also embedded). Proves both stage-1 components behave like their Rust counterparts on the input. |
| `omg --disasm <file>`                       | Rust (only for `.omg` input) | Backend-agnostic for `.omgb` input. |
| `omg` (no args)                             | Rust        | REPL. Stays Rust because per-turn compile latency matters interactively. |

`--self-hosted` (no compile) used to be the opt-in for the OMG-written
path. It's now a deprecated no-op alias for the (default) self-hosted
behavior, kept around so existing scripts don't break.

## The fixed-point check

```sh
omg --verify-self-hosted bootstrap/src/compiler.omg
```

This is the single most load-bearing test in the repo. It does:

1. Compile `bootstrap/src/compiler.omg` with the **Rust** frontend → bytes A.
2. Compile `bootstrap/src/compiler.omg` with the **OMG-written** compiler
   running on the VM → bytes B.
3. Assert `A == B` byte-for-byte.

If they ever drift apart, one of the compilers is wrong. The CI
workflow at [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)
runs this on every push / PR. Both compilers must agree on:

- the order in which `MakeFunc` instructions are emitted at top level;
- the per-module name mangling for imported names;
- the function table sort key (alphabetical);
- the encoding of every literal — including, recently, `f64` literals
  (the OMG compiler computes their bit pattern via the `float_bits`
  builtin and writes the i64 bits the same way it writes any other
  8-byte payload);
- the implicit-return convention (`PushNone + Ret` for procs that fall
  off the end — see the
  [history below](#history-of-the-implicit-return-bug)).

Anything you change in either compiler that affects emitted bytecode
needs the matching change in the other. The fixed-point check will
catch you immediately if you forget.

## The compiler compiling itself

`--self-hosted-compile` runs the embedded OMG-written compiler on
whatever input you give it. Point it at the compiler's own source and
the result is exactly what it sounds like:

```sh
omg --self-hosted-compile bootstrap/src/compiler.omg /tmp/recompiled.omgb
cmp bootstrap/src/compiler.omgb /tmp/recompiled.omgb && echo byte-identical
```

The two files are byte-for-byte equal (around 130 KB at time of writing).
The first one was produced at `cargo build` time by the Rust frontend;
the second was produced just now by the OMG-in-OMG compiler running on
the VM. Both took the same source as input. They agree.

This is the same demonstration as `--verify-self-hosted`, just with the
two outputs surfaced as files you can inspect. `--verify-self-hosted`
is essentially:

```sh
omg --compile bootstrap/src/compiler.omg /tmp/rust.omgb       # stage-0 output
omg --self-hosted-compile bootstrap/src/compiler.omg /tmp/omg.omgb   # stage-1 output
cmp /tmp/rust.omgb /tmp/omg.omgb
```

with the temp files held in memory and the comparison built in.

### Why this is the load-bearing claim

The Rust frontend exists to **bootstrap** — to break the chicken-and-egg
of needing a working OMG implementation to compile the OMG-in-OMG
compiler the first time. Once `compiler.omgb` exists and the fixed
point holds, the Rust frontend is no longer load-bearing for
*correctness*; the OMG compiler can sustain itself.

That sustains-itself property is what makes a compiler self-hosted in
the textbook sense. It's not just "we have a compiler written in OMG"
— it's "the OMG compiler can compile its own source, byte-identically,
to the same artifact the bootstrap produced."

### Evolving the compiler without the Rust frontend

The fixed-point property means you can update the OMG compiler using
*itself* as the build tool:

1. Edit [`bootstrap/src/compiler.omg`](../bootstrap/src/compiler.omg).
2. Use the *current* `compiler.omgb` (running on the VM) to compile
   the new source:
   ```sh
   omg --self-hosted-compile bootstrap/src/compiler.omg /tmp/new.omgb
   ```
3. Replace the embedded copy:
   ```sh
   cp /tmp/new.omgb bootstrap/src/compiler.omgb
   cargo build --release --manifest-path runtime/Cargo.toml
   ```
   *(`build.rs` will rebuild `compiler.omgb` from source; if you want
   to skip that and embed a hand-built copy, you'd point `build.rs` at
   the existing file or run with the build script bypassed.)*
4. Run the fixed-point check to confirm the Rust frontend still agrees:
   ```sh
   omg --verify-self-hosted bootstrap/src/compiler.omg
   ```

Most of the time, step 4 just works — because most compiler changes
preserve the existing bytecode contract for existing inputs. But if a
change *does* affect emitted bytecode, the Rust frontend has to be
updated in lockstep (see the float-literal and implicit-return changes
for examples). Otherwise the bootstrap will succeed but the fixed-point
check will fail — your two compilers have drifted.

### The triple-meta fixed point

There's a third leg now: an OMG-written *VM* at
[`bootstrap/src/vm.omg`](../bootstrap/src/vm.omg). It executes `.omgb` bytecode
the same way the Rust VM does — bytecode loader, dispatch loop,
operand stack, env stacks, the whole thing — only it's all written in
OMG. `cargo build` compiles it to `bootstrap/src/vm.omgb` and embeds it in
the runtime alongside `compiler.omgb`.

Running the OMG compiler *on top of* the OMG VM gives you an entire
language-level pipeline that lives inside OMG: the Rust runtime is
just the substrate at the bottom. `--verify-omg-vm` compares this
triple-meta path against the Rust frontend's output and asserts
byte-identical equality:

```sh
omg --verify-omg-vm bootstrap/src/compiler.omg
```

That command, when it passes (which it does, in ~60 s), is the
strongest claim the project makes about itself: the OMG compiler and
the OMG VM, both running on a Rust *substrate* but otherwise expressed
entirely in OMG, produce the same artifact for the same input as the
reference Rust toolchain does.

For day-to-day use the triple-meta path is too slow (the OMG VM
running the OMG compiler running the OMG-source-language is slow³).
For verification it's perfect: any drift in either stage-1 component
shows up immediately as a byte mismatch.

### What the Rust frontend still has to do

Even though the OMG compiler can compile itself, the Rust frontend
still has one job: it has to be able to compile `bootstrap/src/compiler.omg`
*at all*, because `cargo build` runs it during the build to produce
`compiler.omgb` from source. So:

- Any feature you add to the OMG language that you want
  `compiler.omg` to **use** has to land in the Rust frontend first
  (else the build can't bootstrap).
- Features you add that `compiler.omg` doesn't use can stay
  OMG-only — but in practice, dogfooding in `compiler.omg` is the
  goal, so this is rarely the path you take.

This is why the float-literal work touched both compilers: the Rust
frontend needed `f64` lex/parse/emit support so a future
`compiler.omg` could use float literals if it wanted to. Today it
doesn't — but the door is open.

## Things this design lets us do

**Test the compiler by running it.** Every `omg <script>` (default mode)
exercises the OMG-written compiler. The hundreds of programs in
`examples/` and `tools/` collectively form a corpus the compiler is
asked to produce bytecode for, and the runtime exercises that bytecode
end-to-end.

**Self-host new language features.** Float literals were added by
extending *both* compilers in lockstep, then proving fixed-point
preservation on the existing compiler source. Once the fixed-point
check passes, we know the OMG compiler can compile programs using the
new feature.

**Mutual sanity check.** A bug that affects only one compiler will
surface as a fixed-point divergence rather than as user-visible
mis-execution.

## Things this design *doesn't* do

**It doesn't make `omg <script>` fast.** Compiling 200 lines of OMG via
the OMG-on-VM compiler takes hundreds of milliseconds; compiling all of
`tools/test-all.omg` (with its 14 imported tools) takes ~1 second;
the 2.4k-line `compiler.omg` itself takes ~9 seconds. The Rust frontend
does the same work in single-digit milliseconds. That's the cost you
pay for dogfooding by default, and the reason `--rust` exists.

> **Note**: the *bytecode-VM* path described above is one of two ways
> OMG can run. There's also a **native-compilation path** —
> [`bootstrap/src/native-c.omg`](../bootstrap/src/native-c.omg) transpiles
> bytecode to C, which `cc -O3` turns into a standalone ELF binary.
> Programs compiled that way have no Rust dependency at runtime: not
> the VM, not the built-ins, nothing. See [`docs/native/`](native/)
> for the full story. The two paths share the same bytecode format;
> they're alternative *backends*.

## History of the implicit-return bug

Worth recording because it took the SSG to surface and would have
silently corrupted any program that called a void proc inside an
expression.

Before the fix, both compilers emitted a single `Ret` opcode at the end
of a `proc` body. The VM's `Ret` handler does
`stack.pop().unwrap_or(Value::None)` — it takes whatever happens to be
on top of the operand stack and treats it as the return value. For a
proc that called `return expr`, that's correct: the explicit `return`
already pushed the result.

For a proc with no explicit return, the implicit `Ret` instead popped
*whatever the caller left on the operand stack before the call*. Most
of the time this didn't matter, because most call sites push the
function's args then immediately consume the return value, so the stack
height is back to where it was. But this pattern broke it:

```omg
xs := xs + [void_proc()]
```

Bytecode for `xs + [void_proc()]`:

```
Load("xs")           ; stack: [xs]
Call("void_proc")    ; stack: [xs] (callee enters)
                     ;   inside the callee, with no explicit return,
                     ;   the implicit Ret pops xs and returns IT
                     ;   — leaving the operand stack empty
                     ; back in caller, expecting [xs, return_value]
BuildList(1)         ; pops one — there's nothing — VmInvariant
```

Fix: emit `PushNone + Ret` for the implicit-return path in both
compilers, in lockstep, so the implicit `Ret` always pops a real value.

The bug had been latent for the entire life of the language — it just
didn't trigger because no example or tool happened to use a void proc
inside an expression. The SSG was the first program that did.

## Files to read if you're hacking on this

- [`runtime/src/main.rs`](../runtime/src/main.rs) — dispatch,
  `self_hosted_compile`, `--rust` flag handling.
- [`runtime/build.rs`](../runtime/build.rs) — the stage-0 bootstrap.
- [`bootstrap/src/compiler.omg`](../bootstrap/src/compiler.omg) — the
  OMG-written compiler. Walk it top to bottom: lexer → parser →
  bytecode emitter → bytecode writer.
- [`runtime/src/compiler.rs`](../runtime/src/compiler.rs) — the Rust
  frontend. The structure mirrors `bootstrap/src/compiler.omg` closely
  because they're meant to produce identical output.
- [`runtime/src/bytecode.rs`](../runtime/src/bytecode.rs) — the
  ground-truth definition of the `.omgb` format.

If you change anything in `bootstrap/src/compiler.omg`, run the
fixed-point check before committing. Same for any change to
`runtime/src/compiler.rs` or `runtime/src/bytecode.rs` that affects
emitted bytes.
