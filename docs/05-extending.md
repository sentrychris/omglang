# 05 · Extending the compiler

Three kinds of changes, in increasing difficulty:

1. **New builtin** — easiest. Pure addition.
2. **New opcode** — medium. Touches every layer but the surface is small.
3. **New syntax form** — biggest. Parser + compiler, and often an opcode.

The golden rule: **a feature exists once per way of running a
program**. There is one compiler, but four things execute what it
produces (the VM, machine code, C, JavaScript), and each has to learn
the feature. The conformance corpus runs every program every way, so
a missed one fails the tests.

## The lockstep rule

| What                     | Where it has to go |
| ------------------------ | ------------------ |
| Syntax (lexer, parser, code generation) | `compiler.omg` only |
| Opcode numbers, format version | the tables in `compiler.omg`, `vm.omg` and `bytecode.omg` |
| What an opcode does      | `vm.omg` (`step_inner`), `native-x64.omg` (`mg_body`, and `pp_depths` for its effect on the stack), `native-c.omg` (`emit_c_for_instr`), `native-js.omg` (`emit_js_for_instr`) |
| A builtin's name         | the `cc_builtins` list in `compiler.omg` |
| A builtin's behaviour    | `runtime/x64/builtins.omg` (a `bi_` proc and a line in `rt_bi_init`); `omg_rt.h` (the function, `omg_call_builtin`) with `emit_builtin` in `native-c.omg`; `omg_rt.js` with `emit_builtin` in `native-js.omg` |
| What the editor says about a builtin | `vscode/server/src/builtins.ts` (its hover text and argument counts) and the builtin name list in `vscode/syntaxes/omg.tmLanguage.json`; `npm test` in `vscode/` fails until both match `cc_builtins` |

Two things are *not* duplicated and need changing in one place only:
how names resolve in generated code (`backend.omg`, shared by the back
ends) and how a `.omgb` is read back for them (`bytecode.omg`).

The VM has no builtins of its own. `vm.omg` passes a builtin call on
with `call_builtin(name, args)` to the runtime of whatever is running
it, so `omg foo.omg` gets a new builtin as soon as `omg` has been
rebuilt on a runtime that has it.

After any change, run:

```sh
scripts/build.sh
tests/run.sh
```

`build.sh` builds the toolchain twice, the second time with the result
of the first, so a change that breaks the compiler or a back end
usually stops right there. `tests/run.sh` then runs the conformance
corpus every way.

### The bootstrap rule

`omg` is built by the `omg` that is already in `bin/`. So the
toolchain's own source (`src/*.omg`, `src/backends/`, `src/runtime/x64/`)
may only use what that existing binary understands. Adding a feature
and using it inside the compiler are therefore two steps:

1. Add the feature, rebuild, run the tests, commit `src/` and the
   rebuilt `bin/`.
2. Now the checked-in `omg` knows it, and the toolchain's source may
   use it.

The frozen Rust runtime in `rust/` is not part of the lockstep. It
does not learn new features.

---

## 1. Adding a new builtin

We'll add `square(x)` — returns `x * x`.

### Step A — Compiler allowlist

Add to the `cc_builtins` list in [src/compiler.omg](../src/compiler.omg)
(don't forget the comma if it's not the last element):

```omg
alloc cc_builtins := [
    "chr", "ascii", "hex", "binary", "length", "read_file",
    ...
    "exit_with_error", "square"
]
```

This tells the compiler that calls to `square(...)` should compile to
`BUILTIN("square", argc)` rather than a regular function call.

### Step B — The machine-code runtime

Add the implementation to
[src/runtime/x64/builtins.omg](../src/runtime/x64/builtins.omg). This
file is raw-mode OMG ([06-runtime.md](06-runtime.md)): each argument
arrives as two words, its tag and its payload, and the proc returns a
payload after putting the result's tag in `rt_tag`:

```omg
proc bi_square(t, v) {
    if t == T_INT {
        rt_tag := T_INT
        return v * v
    }
    if t == T_FLOAT {
        rt_tag := T_FLOAT
        return __fmul(v, v)
    }
    return rt_type_error("square() expects one number")
}
```

The name is the contract: for `BUILTIN("square", 1)` the back end
looks for a proc called `bi_square` (or `bi_square_1`, when a builtin
takes a varying number of arguments). Nothing needs adding to
`native-x64.omg`.

Then register it in `rt_bi_init`, in the same file, so that
`call_builtin("square", ...)` finds it. That is how the VM reaches it:

```omg
rt_bi_add("square", bi_square, 1)
```

(Raw-mode `*` wraps on overflow. A real `square` would check for it
the way `rt_mul` does; this one keeps the example short.)

### Step C — The C runtime

Add the implementation to [src/runtime/omg_rt.h](../src/runtime/omg_rt.h):

```c
static Value omg_builtin_square(Value v) {
    if (v.tag == OMG_INT)   return omg_int(v.v.i * v.v.i);
    if (v.tag == OMG_FLOAT) return omg_float(v.v.f * v.v.f);
    omg_panic("TypeError", "square() expects one number");
    return omg_none(); /* unreachable */
}
```

Also add it to `omg_call_builtin`'s switch (used by reflective dispatch):

```c
if (strcmp(n, "square") == 0)         return omg_builtin_square(a[0]);
```

And a case in `emit_builtin` in [src/backends/native-c.omg](../src/backends/native-c.omg):

```omg
if name == "square" and argc == 1 { return emit_builtin1("omg_builtin_square") }
```

### Step D — The JS runtime

The same two things, or the builtin will be missing in the playground
and from `omg --emit-js`: a function in
[src/runtime/omg_rt.js](../src/runtime/omg_rt.js) and a case in
`emit_builtin` in [src/backends/native-js.omg](../src/backends/native-js.omg).

### Step E — Rebuild and verify

```sh
scripts/build.sh

echo ';;;omg
emit square(7)' > /tmp/sq.omg
bin/omg /tmp/sq.omg                               # 49, interpreted
bin/omg --build /tmp/sq.omg /tmp/sq && /tmp/sq    # 49, built
bin/omg --emit-js /tmp/sq.omg /tmp/sq.js && node /tmp/sq.js
```

Then give it a place in the conformance corpus: a line in one of the
programs under `tests/conformance/` (or a new `NAME.omg` with its
`NAME.expected`). From then on every way of running OMG is held to it.

Last, the editor: add an entry to `vscode/server/src/builtins.ts` (the
text shown on hover, with an example) and the name to the builtin list
in `vscode/syntaxes/omg.tmLanguage.json`. The *VS Code extension* job in
CI fails until the two match `cc_builtins`.

### Common mistakes

- Forgetting the allowlist (Step A): `square` compiles as a regular
  function call and you'll see `UndefinedIdentError: square`.
- Forgetting `rt_bi_add` (Step B): a built executable works, and
  `omg foo.omg` says the builtin does not exist, because the VM goes
  through `call_builtin`.
- Forgetting `omg_call_builtin` (Step C): the mirror image on the C
  route. Direct calls work; `call_builtin("square", ...)` fails.
- Using `square` inside the toolchain's own source in the same change:
  see [the bootstrap rule](#the-bootstrap-rule).

---

## 2. Adding a new opcode

Opcodes live in every layer. Adding one is more work but each spot is small.

We'll add `OP_DUP` — pushes a copy of the stack top.

### Step A — Opcode constants

Pick a free number (bump the existing high-water mark) and add
`alloc OP_DUP := 56` to each of:

- [src/compiler.omg](../src/compiler.omg) (it writes the opcode)
- [src/vm.omg](../src/vm.omg) (it dispatches on it)
- [src/bytecode.omg](../src/bytecode.omg) (it reads it back for the back ends)

The three tables must agree, and `tests/driver.sh` fails when they do
not.

### Step B — Compiler emission

Emit the instruction where the code generator in `compiler.omg` needs
it, and give it a case in `encode_instr`, which turns instructions
into bytes.

### Step C — VM dispatch

In [vm.omg](../src/vm.omg)'s `step_inner` switch. `step_inner`
pre-advances `vm_pc` before dispatching, so a normal-flow opcode skips
its own pc-increment; only `JUMP` / `JUMP_IF_FALSE` / `CALL` / `RET`
override the pre-advanced pc:

```omg
if op == OP_DUP {
    alloc top := vm_stack[vm_stack_top - 1]
    vm_push(top)
    return false
}
```

Opcodes are dispatched on integer tags (the `OP_*` constants at the top
of vm.omg) — the compiler emits string-tagged tuples and
`vm_extend_code` normalises them to ints on ingest, so step_inner only
ever sees ints. Register the new name in vm.omg's `OPNAME_TO_INT`
table.

### Step D — Decoders

In [vm.omg](../src/vm.omg)'s `decode_one`:

```omg
if op == OP_DUP { return tagged0(OP_DUP, cursor) }
```

The back ends read bytecode through a second decoder, `decode_one` in
[bytecode.omg](../src/bytecode.omg), which keeps opcode
names as strings. Add the opcode there as well:

```omg
if op == OP_DUP { return tagged0("DUP", cursor) }
```

and its name in `disasm_op_name` in [omg.omg](../src/omg.omg), so
`--disasm` can print it.

### Step E — The x86-64 back end

Two places in [native-x64.omg](../src/backends/native-x64.omg):

- `pp_depths`, the pre-pass that works out how many values are on the
  operand stack at every instruction. Add `DUP` to the group with the
  same effect (one more value), or the back end will refuse the
  program.
- `mg_body`, the loop that generates code for a proc of your program.
  Add a branch that updates the virtual stack: for `DUP`, push a
  second entry describing the same value, taking a reference count if
  it is (or may be) a heap object. The helpers around it (`mg_push`,
  `mg_flush`, `mg_generic` for "call this runtime proc with the top n
  values") cover most new opcodes; read the neighbouring branches
  first.

If the runtime's own source will use the opcode too, `rw_body` (raw
mode) needs it as well. Most new opcodes do not get that far.

### Step F — The C and JS back ends

In [native-c.omg](../src/backends/native-c.omg)'s `emit_c_for_instr`:

```omg
if op == "DUP" {
    return "    { Value v = stack[sp - 1]; omg_inc(v); stack[sp++] = v; }\n"
}
```

Note the `omg_inc` — duplicating a refcounted value bumps the count so both
slots own a reference.

And the same opcode in [native-js.omg](../src/backends/native-js.omg)'s
`emit_js_for_instr`, where there is no reference counting to do.

### Step G — Rebuild and verify

```sh
scripts/build.sh
tests/run.sh
```

Remember [the bootstrap rule](#the-bootstrap-rule): the compiler may
not *emit* the new opcode for constructs the toolchain's own source
already uses until a binary whose VM and back ends understand it is in
`bin/`. If it must (a change to how `if` compiles, say), do it in two
commits: first teach the VM and the back ends the opcode, rebuild and
commit; then switch the compiler over.

---

## 3. Adding a new syntax form

Hardest case, but mechanical once you've got the muscle memory.

Suppose we want to add `unless cond { ... }` — the inverse of `if`.

### Step A — Lexer (if needed)

If `unless` is a new keyword, register it in `keyword_kind` in
[src/compiler.omg](../src/compiler.omg).

If it's a soft keyword (just an identifier with special meaning to the
parser), you can skip this — but soft keywords are usually a mistake.

### Step B — Parser

Add an `Unless(cond, body)` AST node, and have the statement parser in
`compiler.omg` recognise the keyword and produce that node.

### Step C — Compiler

Lower `Unless` to existing bytecode. `unless cond { body }` is just
`if !cond { body }`, so emit:

```
<compile cond>
Not
JumpIfFalse end_label
<compile body>
end_label:
```

Because this uses only opcodes that already exist, nothing downstream
changes: the VM and all three back ends run it as it is. That is the
payoff of the bytecode layer, and the reason to prefer lowering to
existing opcodes over inventing one.

### Step D — The editor and the web

Syntax has one more home outside the toolchain: the VS Code extension
in `vscode/`. And the playground on the site runs a copy of the
compiler, so it goes on compiling the old syntax until
`scripts/update-site.sh` has been run and the site deployed
([toolchain.md](toolchain.md#the-browser-builds)). The site's own
pages teach the language too, and are edited there.

The extension's language server carries a port of the lexer and the
parser (`vscode/server/src/lexer.ts`, `parser.ts`), the scope rules
(`analysis.ts`), the builtin reference (`builtins.ts`) and the keyword
reference (`language.ts`); the grammar in `vscode/syntaxes/` has its own
list of builtin names. `npm test` in `vscode/` says what is out of
step: it compares the builtin list with `cc_builtins`, runs every
documented example, and checks that broken programs get the same first
syntax error from the language server as from `omg --compile`.

### Step E — Verify

```sh
scripts/build.sh
tests/run.sh
```

Add a program that uses `unless` to `tests/conformance/`. Do not use
it in the toolchain's own source until the rebuilt `bin/` is committed
([the bootstrap rule](#the-bootstrap-rule)).

---

## Common pitfalls across all three

### Bytecode versioning

`src/compiler.omg` has `BC_VERSION := 512` (`0x000200` —
`(MAJOR<<16)|(MINOR<<8)|PATCH`), and the same constant is in
`src/vm.omg` and `src/bytecode.omg`. If you change the bytecode format
incompatibly, bump this in **all three** places (`tests/driver.sh`
compares them). Old `.omgb` files are
rejected at load time with a clear "recompile your .omgb" SyntaxError
— there's no silent breakage, but also no fallback to v1.

The current format carries a source-file table and a per-instruction
source map (parallel to the code blob) that the VM and the back ends
use to render Python-style tracebacks. Both sections were added at the
v1 → v2 bump.

### Refcount discipline

The machine-code and C runtimes both count references, and the rules
are the same in spirit:

- A value on the operand stack or in a variable owns one count.
- Moving a value transfers the count; copying it takes a new one.
- Whoever discards a value gives its count back.
- A runtime proc never keeps what it is passed without taking a count
  of its own, and returns a value its caller owns.

In C that is `omg_inc` / `omg_dec` around `stack[sp]`; in
`native-x64.omg` the virtual-stack entries record whether they are
owned, and `mg_release` / the `mg_stub_drop` helpers give counts back.
See [06-runtime.md](06-runtime.md) for the full protocol.

### Raw mode is not OMG as you know it

`src/runtime/x64/*.omg` looks like OMG but is compiled with different
rules: one untagged word per value, wrapping arithmetic, no lists, no
dicts, no `try`. A proc there with 500 or more locals is refused. The
rules are at the top of part 3 of `native-x64.omg`, and
[06-runtime.md](06-runtime.md) summarises them.

### Checked-in artefacts

`bin/` is generated and checked in. After a change under `src/`,
rebuild it (`scripts/build.sh`) and commit it, in a commit of its own.
CI fails when it has fallen behind its sources.

The browser bundles are generated too, and are not checked in here:
`scripts/update-site.sh` builds them and copies them into the site,
which is where the playground lives
([toolchain.md](toolchain.md#the-browser-builds)).

---

## A worked example

There's a regression test for an actual compiler bug:

- The bug: `return` / `break` / tail-call inside `try` didn't pop the
  exception-handler block, leaving a stale handler that caught later panics.
- The fix was in how `compiler.omg` emits `PopBlock` instructions.
- The test: [tests/regression/control_flow_in_try.omg](../tests/regression/control_flow_in_try.omg),
  with the output it must produce next to it.

That test exercises four leak paths and must pass interpreted and
built. Use it as a template for adding regression tests when you fix
similar bugs.

## Read next

- [06-runtime.md](06-runtime.md) — the runtimes in detail
- [07-debugging.md](07-debugging.md) — diagnosing what went wrong
