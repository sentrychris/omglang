# native-asm — OMG bytecode → x86_64 ELF, no C compiler

Status: **Phase 6 complete (a + b + c).** Closures with arbitrarily deep capture: each function's env carries a flattened ancestor chain via `func_env_layouts`. `gen_make_func` copies parent's env entries (via r12) followed by parent's scope entries (via r14), so a 4-level nested function sees its grandparent's, grandparent's-grandparent's variables etc. with O(1) lookup. Phases 7-12 pending.

Owner: sentrychris + claude

## Goal

A third backend at the same level as [`native-c.omg`](../bootstrap/src/native-c.omg)
and [`native-js.omg`](../bootstrap/src/native-js.omg): take `.omgb`
bytecode and write a statically-linked x86_64 Linux ELF directly —
machine code emitted by OMG, no `cc`, no libc, no C in the chain
anywhere.

End-state demo:

```sh
apt remove gcc clang        # no C compiler on the system
bootstrap/build-asm.sh      # rebuild the OMG toolchain from itself
./bootstrap/bin/omg foo.omg # works
```

The seed shrinks from "Rust + cc" to just "Rust." Once the new backend
is self-hosting, even the Rust seed is optional for further iteration
— the existing self-rebuild path (`bootstrap/build.sh`'s native branch)
keeps working without it.

## Why this is a showcase

Three pitches at once:

- **Deepest language-flex possible.** OMG goes from "compiles to C, uses
  `cc`" to "owns its toolchain top to bottom." That's the line between
  "transpiler" and "real compiler." See [Wikipedia stage-2 vs stage-3](https://en.wikipedia.org/wiki/Bootstrapping_(compilers)#Process) —
  the new backend is what makes our stage-2 genuinely independent.
- **Bootstrappable Builds territory.** With C removed from the chain,
  OMG joins the conversation alongside Mes, TinyCC, and the Stage0
  project.
- **Reusable infrastructure for a JIT.** The same machine-code emitter
  is most of what a baseline JIT for `vm.omg` would need. Pick this and
  the JIT plan halves in size.

## Architecture

Five new pieces, all parallel to existing backends:

```
bootstrap/src/native-asm.omg    ← parallel to native-c.omg; bytecode → ELF
bootstrap/src/elf.omg           ← ELF64 writer (header, program headers, sections)
bootstrap/src/x64.omg           ← x86_64 instruction encoder (REX, ModR/M, SIB)
bootstrap/src/omg_rt.asm        ← tiny hand-written runtime helpers (alloc,
                                  dict-lookup, list-grow, syscalls). Assembled
                                  once, checked in as a byte blob.
bootstrap/src/omg_rt_blob.omg   ← the assembled helpers as `alloc rt_blob := [...]`
                                  embedded into every emitted ELF's .text.
```

Tool name: `omgna` (OMG Native Assembler) — bytecode → ELF. Mirrors
`omgcc` (bytecode → C → ELF via `cc`) in role.

### Pipeline

```
foo.omg
   │  omgc
   ▼
foo.omgb
   │  omgna (← native-asm.omg compiled with itself)
   ▼
foo (statically linked ELF; no libc, no cc)
```

### Machine-code generation strategy

**Macro-expansion / template codegen.** Each bytecode opcode maps to a
fixed sequence of x86_64 instructions that manipulate a software-managed
operand stack in memory (mirroring the VM's design). No register
allocator in phase 1 — every value lives on the operand stack between
ops, and ops materialize values into registers only for the duration of
the op itself.

Concretely: register `r15` holds the operand-stack pointer (similar to
`vm_stack_top` in `vm.omg`); `r14` holds the locals/globals frame
pointer. `rsp` is the native call stack, used for native CALLs only.
This split keeps OMG's semantics straightforward (the operand stack
behaves exactly like the VM's) and avoids tangling our value
representation with the SysV calling convention.

This is the same shape as a "threaded code" VM compiler or a baseline
JIT (think LuaJIT's interpreter tier, or V8 Sparkplug). It's not
optimal — register allocation would shave ~3-5× — but it's tractable in
~2000 lines of OMG and gives us a working pipeline to optimize *into*.

### Value representation

Pointer-tagging in the low 3 bits of a 64-bit word:

```
tag 000  ptr to heap object (list, dict, string, closure)
tag 001  small int (61 bits, sign-extended; matches OMG semantics for
         the common case, falls back to boxed bigints for overflow)
tag 010  float bits (NaN-boxed; non-NaN doubles fit; NaNs box a small
         payload)
tag 011  bool / none / sentinel
tag 100-111  reserved
```

This is exactly the shape OMG's existing C runtime uses (see
[`omg_rt.h`](../bootstrap/src/omg_rt.h)), so we can port the same
helper logic.

### What the runtime blob provides

Hand-written x86_64 assembly, ~500-1000 lines:

- `omg_alloc` — bump allocator on a `mmap`'d arena (no GC yet).
- `omg_emit` — write a tagged value's string form to stdout via `write`
  syscall.
- `omg_dict_get` / `omg_dict_set` — open-addressed hash, same shape as
  the C runtime's.
- `omg_list_grow` — geometric capacity growth.
- `omg_panic` — write message + `exit(1)`.
- `_start` — set up the operand stack arena, call `omg_main`, exit.

These are stable, small, and rarely change. They're assembled once
(during a one-off bootstrap from a system assembler like `nasm` or
`as`) and checked in as `omg_rt_blob.omg` containing a literal byte
array. The OMG-only build path then never touches a system assembler.

### ELF writer

[`bootstrap/src/elf.omg`](../bootstrap/src/elf.omg) emits a minimal
ELF64 executable:

- ELF header
- Two program headers: `PT_LOAD` for `.text` (executable, embedded
  runtime + emitted code), `PT_LOAD` for `.data` (constant strings,
  literal floats, function table).
- No section headers needed for execution; we may emit a minimal
  section table for `objdump` friendliness.
- No dynamic linking, no `.interp`, no relocations needed since
  internal jumps are patched at emit time and the runtime blob is
  position-independent.

Entry point: `_start` in the runtime blob. Total binary size for
hello-world: target ≤ 8 KB.

## Phases

Phasing is critical — this is a multi-week project. Each phase ships
a working artifact and unlocks the next.

| # | Phase | Bytecode covered | Test |
|---|---|---|---|
| 1 | Skeleton + hello-world | `PUSH_STR`, `EMIT`, `HALT` | `hello.omg` → ELF prints "hello, world" |
| 2 | Integer arithmetic | `PUSH_INT`, `ADD`/`SUB`/`MUL`/`DIV`/`MOD`, comparisons | `tests/regression/arith.omg` matches Rust |
| 3 | Control flow | `JUMP`, `JUMP_IF_FALSE`, label patching | fib, loops, conditionals |
| 4 | Functions | `CALL`, `RET`, locals, parameter passing | recursive fib, mutual recursion |
| 5 | Heap values | `BUILD_LIST`, `BUILD_DICT`, indexing, `PUSH_STR` strings on heap | maze_solver, merge_sort |
| 6 | Closures + globals | `MAKE_FUNC`, `LOAD`/`STORE` of mangled globals, `CALL_VALUE` | higher_order test |
| 7 | Try / except + raise | `SETUP_EXCEPT`, `POP_EXCEPT`, `RAISE` | regression suite traceback tests |
| 8 | Builtins parity | every builtin in `cc_builtins` | full regression suite |
| 9 | **Self-hosting checkpoint** | compile `compiler.omg` with `omgna`; resulting binary recompiles itself byte-identically | new triple-meta verify command |
| 10 | GC | mark-sweep over the bump arena, root from operand+local stacks | long-running programs don't OOM |
| 11 | Peephole optimizer | constant folding, dead push/pop, jump threading | benchmark vs current C-backend output |
| 12 | (Stretch) ARM64 backend | mirror `x64.omg` → `arm64.omg` | run on Raspberry Pi |

Phase 9 is the headline. Phases 10-12 are quality-of-life and reach.

## Open questions

- **How much of `omg_rt.asm` can be written in OMG instead?** The pure
  syscall wrappers (write, mmap, exit) are tiny but inherently
  hand-coded — there's no way to express "syscall instruction" in
  OMG without a builtin. The dict / list / GC logic could plausibly
  be OMG that we compile with `omgna` itself, removing more hand-
  written assembly. Decide after phase 5.
- **Position-independent or position-dependent?** PIC is more
  future-proof (matters if we ever want to load OMG ELFs as shared
  libraries or do JIT) but adds complexity. Start position-dependent,
  load at a fixed base; revisit at phase 10.
- **Stack frame layout for `CALL`.** OMG's calling convention is
  variadic-shaped (everything is a tagged value on the operand
  stack). Either (a) translate every OMG call to a native
  `call`/`ret` with a custom convention, or (b) keep CALL frames
  on the OMG operand stack as the VM does, and use `rsp` only at
  the syscall boundary. (b) is simpler and matches what the VM
  does already. Recommend (b) for phase 4, revisit at phase 11
  if native `call` would unlock peephole wins.
- **Debug info.** ELF supports DWARF; do we emit any? Probably
  punt for phase 1 — the C backend emits no DWARF either, so
  parity is fine. Add a `.note.omg_lines` custom section in phase
  4 if traceback frames need source-mapping back to OMG line
  numbers.

## Non-goals

- **Windows, macOS, BSD.** Linux ELF only. The architecture is the
  same, but each platform's loader has its own quirks (Mach-O for
  macOS, PE for Windows). Cross-platform support is a fork of the
  ELF writer; not in scope here.
- **Beating optimized C output.** The C backend uses `cc -O2`. Our
  macro-expansion emits ~3-5× slower code than that. Acceptable; the
  point is self-containment, not speed. Phase 11's peephole pass
  closes part of the gap but full register allocation is a separate
  project.
- **Replacing the C backend.** `omgcc` stays. Users who want
  optimized binaries or cross-compile to non-Linux targets keep
  using it. `omgna` is the *bootstrapping* path; `omgcc` is the
  *production* path.

## Definition of done

- `omgna` exists in `bootstrap/bin/` and produces working ELFs for
  the full regression corpus (`tests/regression/`).
- `bootstrap/build.sh` gains a `--no-cc` mode that uses `omgna`
  instead of `omgcc`+cc for every compile step. The whole toolchain
  rebuilds itself from sources with no C compiler invoked.
- A new `omg --verify-native-asm <file.omg>` command compiles the
  source via both `omgcc` and `omgna`, runs both binaries, and
  asserts equal stdout (functional parity, not byte-identity — the
  generated machine code obviously won't match `cc`'s output).
- Updated [`docs/native/02-architecture.md`](../docs/native/02-architecture.md)
  documents the new path; updated [`docs/native/01-quickstart.md`](../docs/native/01-quickstart.md)
  shows `--no-cc` in the cheat sheet.
- Hello-world ELF emitted by `omgna` is ≤ 8 KB; full self-hosted
  `omgna` binary is ≤ 200 KB. (Compared with ~80 KB for the
  `cc -O2 -s`-stripped equivalent — we'll be 2-3× bigger because of
  the unoptimized templates, which is fine.)

## Approximate scope

- `bootstrap/src/x64.omg`: ~600 lines (instruction encoder for the
  ~40 x86_64 instructions we actually need: MOV, ADD/SUB/IMUL/IDIV,
  CMP, Jcc, JMP, CALL, RET, PUSH, POP, syscall variants).
- `bootstrap/src/elf.omg`: ~250 lines (ELF header + 2 program headers
  + section pack).
- `bootstrap/src/native-asm.omg`: ~1800 lines (template per opcode,
  plus the toplevel program/function emit drivers).
- `bootstrap/src/omg_rt.asm`: ~700 lines of x86_64 assembly,
  one-time hand-written.

Total: ~3300 lines of new code, of which ~700 is one-time assembly
and the rest is OMG. Comparable in size to `native-c.omg` + `omg_rt.h`
(1672 + 2217 = ~3900 lines). Estimated calendar time: 3-4 weeks of
focused work to phase 9 (self-hosting), another 2-3 weeks for the
remaining phases.

## Suggested first commit

Just phase 1: a `hello.omg` that compiles to an ELF via `omgna` and
prints "hello, world" on stdout. ~600 lines (skeleton + minimal x64
encoder + minimal ELF writer + 4 opcode templates + a 50-line runtime
blob with just `write` + `exit`). Everything after that builds on this
spine.
