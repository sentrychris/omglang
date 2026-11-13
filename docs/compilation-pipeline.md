# How OMG actually runs your script

A developer-oriented walk through what happens when you type
`omg foo.omg`. The high-level pitch is simple ("OMG is written in
OMG") but the moving parts are worth knowing if you're going to hack
on the compiler, the VM or the build.

> This page covers the **interpreted path**: `omg foo.omg`, with no
> build step. The other path, `omg --build`, turns the same bytecode
> into an executable; [04-pipeline.md](04-pipeline.md) follows a
> program down both. For a one-page map of every part, see
> [`toolchain.md`](toolchain.md).

## The cast

Three pieces of code are involved. All three are OMG source, and all
three are inside `bin/omg` as machine code:

| Piece            | Lives in                                   | Job |
| ---------------- | ------------------------------------------ | --- |
| **The driver**   | [`src/omg.omg`](../src/omg.omg)            | Reads the command line, calls the other two |
| **The compiler** | [`src/compiler.omg`](../src/compiler.omg)  | Source text to bytecode |
| **The VM**       | [`src/vm.omg`](../src/vm.omg)              | Executes bytecode |

The driver imports the other two as modules (`import "compiler.omg" as
cc`, `import "vm.omg" as vm`). When `omg` was built, all of them were
compiled together into one executable.

## Run time: `omg foo.omg`

```text
   ┌─────────────────────────────┐
   │ 1. The driver reads         │
   │    foo.omg into a string.   │
   └──────────────┬──────────────┘
                  │
                  ▼
   ┌─────────────────────────────┐
   │ 2. cc.compile_source(...)   │
   │    lexes, parses and        │
   │    generates code. Imports  │
   │    are read and compiled    │
   │    into the same program.   │
   │    The result is a list of  │
   │    instructions, a function │
   │    table and a source map,  │
   │    all in memory.           │
   └──────────────┬──────────────┘
                  │
                  ▼
   ┌─────────────────────────────┐
   │ 3. The driver loads them    │
   │    into the VM and seeds    │
   │    the three globals every  │
   │    program gets: args,      │
   │    module_file, current_dir.│
   └──────────────┬──────────────┘
                  │
                  ▼
   ┌─────────────────────────────┐
   │ 4. vm.run() executes your   │
   │    program.                 │
   └─────────────────────────────┘
```

One process, no temporary files, nothing else started. The glue is
`run_omg_source` in `omg.omg`, about twenty lines.

`omg foo.omgb` skips step 2: the file is read and decoded
(`vm.parse_bytecode`) and execution starts at step 3.

### What the VM is

`vm.omg` is an ordinary interpreter: a loop that fetches an
instruction, switches on its opcode and updates an operand stack, an
environment stack and a call stack. Those stacks are OMG lists, so a
program's recursion depth is limited by memory, not by the machine
stack.

The VM does not implement the builtins. When your program calls
`length(xs)` or `read_file(p)`, the VM passes the call on with
`call_builtin(name, args)`, and the builtin that runs is the one in
the runtime `omg` itself was built with. Your interpreted program and
the toolchain share one set of builtins.

### Errors

A compile error is reported as one line and exit code 1. The driver
runs the compiler inside a `try`, so you see
`SyntaxError: Unexpected token ... on line 12 in /path/foo.omg` and
not a traceback through the compiler's own source.

A runtime error that nothing catches leaves the VM with a pending
error and a traceback built from the source map of *your* program;
the driver prints it and exits with 1.

## The other commands

| Command | What happens |
| ------- | ------------ |
| `omg <script.omg>`             | Compile in memory, run on the VM. |
| `omg <script.omgb>`            | Decode, run on the VM. |
| `omg --compile <in> <out>`     | Compile, write the bytecode to a file. |
| `omg --disasm <file>`          | Compile if needed, print the instructions. |
| `omg --build <in> [<out>]`     | Compile, then hand the bytecode to a back end, which writes an executable. The VM is not involved. |
| `omg --emit-c`, `--emit-js`    | Compile, then write the back end's output as source. |
| `omg` (no args)                | The REPL: each complete chunk you type is compiled and appended to a VM that stays alive between chunks. |

## How fast is it?

`omg` is the compiler and the VM as machine code, so starting it and
compiling a small program takes a couple of milliseconds:

| What | Time |
| ---- | ---- |
| `omg examples/hello_world.omg` (compile and run) | ~2 ms |
| Compile `tools/test-all.omg` and the 14 tools it imports | ~0.15 s |
| Compile `src/compiler.omg` (3000 lines) | ~0.1 s |
| Compile `src/omg.omg` and everything it imports (the toolchain) | ~0.9 s |

Running a program on the VM is slower than running it built, by a
factor that depends on the program; tight integer loops show the
biggest gap. That is what `--build` is for.

## The compiler compiling itself

The compiler is an OMG program, so it can be given its own source:

```sh
omg --compile src/compiler.omg /tmp/a.omgb
```

Here the compiler runs as machine code. It can also be run
*interpreted*, by asking `omg` to run the driver from source and
passing it the same request:

```sh
omg src/omg.omg --compile src/compiler.omg /tmp/b.omgb
cmp /tmp/a.omgb /tmp/b.omgb && echo byte-identical
```

Now the VM is interpreting the driver and the compiler (about thirty
seconds instead of a tenth of one), and the two outputs must be the
same file. One compiler, executed two different ways, by a back end
and by the VM, has to reach the same answer for a 3000-line input.
`tests/parity.sh` checks it on every run.

The bigger version of the same check is the build itself.
`scripts/build.sh` has `omg` build `src/omg.omg` into a new `omg`, and
that one build it again; a third round must produce the same bytes as
the second. See [02-architecture.md](02-architecture.md#the-fixed-point).

### Evolving the compiler

Because `omg` is built by the `omg` before it, the compiler's own
source may only use what the checked-in binary already understands.
New syntax or a new builtin goes in in two steps: first make the
compiler, the VM and the back ends accept it, rebuild, and commit the
new `bin/`; then start using it inside `src/`.
[05-extending.md](05-extending.md#the-bootstrap-rule) has the details.

## Where this came from

Until the machine-code back end existed, this page described a
different arrangement. A runtime written in Rust held a Rust VM and a
second, Rust-written compiler; the OMG compiler ran on that VM as
embedded bytecode, and the two compilers were required to produce
identical bytes for every input. That runtime is still in
[`rust/`](../rust/), frozen, and its README describes how it works and
how to rebuild OMG from it. Nothing on this page depends on it any
more.

## Things this design lets us do

**Test the compiler by running it.** Every `omg <script>` exercises
the compiler, and every rebuild of the toolchain has it compile
itself. The programs in `examples/`, `tools/` and `tests/conformance/`
form a corpus it is asked to produce bytecode for, and each of them
is then run every way there is.

**Check implementations against each other.** The VM, the
machine-code back end, the C back end and the JavaScript back end all
consume the same bytecode. A bug in one of them shows up as a
disagreement with the other three, not as silent mis-execution.

## History of the implicit-return bug

Worth recording because it took the SSG to surface and would have
silently corrupted any program that called a void proc inside an
expression.

Before the fix, the compiler emitted a single `Ret` opcode at the end
of a `proc` body. The VM's `Ret` handler takes whatever happens to be
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

Fix: emit `PushNone + Ret` for the implicit-return path, so the
implicit `Ret` always pops a real value.

The bug had been latent for the entire life of the language — it just
didn't trigger because no example or tool happened to use a void proc
inside an expression. The SSG was the first program that did.

## Files to read if you're hacking on this

- [`src/omg.omg`](../src/omg.omg) — the driver: argument handling,
  `run_omg_source`, the REPL, `--build`.
- [`src/compiler.omg`](../src/compiler.omg) — the compiler. Walk it
  top to bottom: lexer → parser → code generator → bytecode writer.
- [`src/vm.omg`](../src/vm.omg) — the VM: decoder, `step_inner`
  (the dispatch loop), error handling.
- [`src/bytecode.omg`](../src/bytecode.omg) — the `.omgb` format as
  the back ends read it.

If you change anything in `src/`, run `scripts/build.sh` and
`tests/run.sh` before committing.
