# Compiler capacity-tracking — faster `omgc` and `bash bootstrap/build.sh`

Status: **Done — 6.5× speedup delivered via a different route.** See
"Outcome" at the bottom for the full story (two failed in-OMG
attempts, then a successful pivot to a new runtime builtin).
Owner: sentrychris + claude

## Goal

Apply the same capacity-tracking pattern we used for the OMG VM's
operand stack (see `bootstrap/src/vm.omg`'s `vm_stack` /
`vm_stack_top` design) to `bootstrap/src/compiler.omg`'s emit-time
data structures. The hot path inside `cc_emit` currently allocates a
fresh list on every instruction:

```omg
proc cc_emit(instr) {
    cc_code  := cc_code  + [instr]
    cc_lines := cc_lines + [[cc_current_file_idx, cc_current_line]]
}
```

Each program compile fires this thousands of times. Switching to a
buffer-plus-depth-index pattern makes the per-emit cost O(1) amortised
instead of O(n).

## Why

- **Build-time felt slowness.** `bash bootstrap/build.sh` recompiles
  five `.omg` sources (compiler, native-c, native-js, vm, omg). Each
  one walks `cc_emit` thousands of times during its self-rebuild.
- **`--compile` is on the user's critical path.** Every `omg <file>`
  invocation compiles the source first; the same emit pattern fires.
- **The VM-side win was real.** Capacity-tracking on `vm_stack` /
  `vm_env_stack` / `vm_ret_stack` / `vm_frames` cleared ~10% off
  `fib(25)` with zero functionality regressions. The compiler's hot
  path is the same shape — same kind of speedup expected, applied
  to a workload that runs on every build.
- **VM-side perf work (e.g. inline-cache LOAD/STORE) wouldn't help
  this scenario.** The native toolchain self-rebuilds itself by
  running AOT-compiled C, not bytecode through `vm.omg`. So the
  only place compiler-side optimisations help is on `compiler.omg`'s
  own runtime cost.

## Scope

### In

The lists that get appended to per-instruction (the hot path):

- `cc_code` — instruction list. Appended in `cc_emit`, indexed in
  `cc_patch`, length-checked in `cc_placeholder` and several
  jump-target sites.
- `cc_lines` — parallel `[file_idx, line]` per instruction. Appended
  in `cc_emit`, never indexed (just serialised at the end).

### Out

Lists touched at lower frequency aren't worth the diff:

- `cc_pending_funcs` — pushed once per `proc` definition.
- `cc_funcs` — pushed once per pending-flush.
- `cc_break_stack`, `cc_loop_try_depth`, `cc_local_scopes` — pushed
  per loop/try/function entry, not per instruction.
- `cc_loaded_modules`, `cc_loading`, `cc_top_level_declared` —
  per-import scope.
- `cc_src_files` — per source file imported.

All keep their existing `xs := xs + [v]` patterns.

## Design

Mirror the VM-side change:

```omg
alloc cc_code := []        # backing buffer; grows monotonically
alloc cc_code_top := 0     # live depth into cc_code
alloc cc_lines := []
alloc cc_lines_top := 0

proc cc_emit(instr) {
    if cc_code_top < length(cc_code) {
        cc_code[cc_code_top] := instr
    } else {
        cc_code := cc_code + [instr]
    }
    cc_code_top := cc_code_top + 1
    if cc_lines_top < length(cc_lines) {
        cc_lines[cc_lines_top] := [cc_current_file_idx, cc_current_line]
    } else {
        cc_lines := cc_lines + [[cc_current_file_idx, cc_current_line]]
    }
    cc_lines_top := cc_lines_top + 1
}
```

Read sites:

- `length(cc_code)` → `cc_code_top` (mostly inside `cc_placeholder`
  and jump-patch sites).
- `cc_code[idx]` (indexing) → unchanged; works as long as
  `idx < cc_code_top`, which is the case for all current callers.
- `cc_code[idx] := ...` (in `cc_patch`) → unchanged.

## The "function-body extraction" wrinkle

This is the load-bearing part of the design, called out as a risk:

`compile_function_body` swaps `cc_code` out, compiles the body into
a fresh empty buffer, then returns the buffer to the caller. The
caller iterates it with `cc_rebase` and concatenates into
`final_code`. With capacity-tracking, the backing list has stale
entries past `cc_code_top` — the caller would iterate those.

Fix: explicitly slice the live portion when extracting. Once-per-
function cost, not in the per-instruction hot path. The proc returns
a slice rather than the raw backing buffer:

```omg
proc compile_function_body(params, body) {
    alloc saved_code      := cc_code
    alloc saved_code_top  := cc_code_top
    alloc saved_lines     := cc_lines
    alloc saved_lines_top := cc_lines_top
    ...
    cc_code      := []
    cc_code_top  := 0
    cc_lines     := []
    cc_lines_top := 0
    ...
    compile_block_node(body)
    cc_emit_simple("PUSH_NONE")
    cc_emit_simple("RET")
    ...
    # Snapshot the live portion of each buffer before restoring.
    alloc out_code  := cc_code[0:cc_code_top]
    alloc out_lines := cc_lines[0:cc_lines_top]
    cc_code      := saved_code
    cc_code_top  := saved_code_top
    cc_lines     := saved_lines
    cc_lines_top := saved_lines_top
    ...
    return [out_code, out_lines]
}
```

The save/restore PAIRS each variable with its `_top` counter. If a
future maintainer saves only one, the body's emits could leak into
the outer scope (subtle bug, caught by the parity gate but easy to
write).

## The "compile_reset" wrinkle

`compile_reset` currently does `cc_code := []`. With capacity-
tracking, we'd want to drop the backing buffer too (otherwise the
buffer persists at high-water mark across compiles in the same
process, e.g. the REPL or the in-process `omg` driver). Resetting
the slice to `[]` and setting `_top := 0` together is right:

```omg
cc_code := []
cc_code_top := 0
cc_lines := []
cc_lines_top := 0
```

Same in `compile_program_node_seeded` (which initialises state per
program). All other state-reset sites need the same pairing.

## Risks (and how each is mitigated)

| Risk | Mitigation |
|---|---|
| `length(cc_code)` slipping through unmodified at a read site, returning the backing-buffer size instead of live depth | Audit every `cc_code` / `cc_lines` reference in `compiler.omg` before claiming done. Triple-meta parity will catch any drift between Rust and OMG output. |
| Save/restore in `compile_function_body` paired only on `cc_code` and not on `cc_code_top` (leak between scopes) | Define the save/restore as one logical block; both pairs always together. Tests with nested `proc` definitions stress this (see `examples/higher_order.omg`, `examples/maze_solver.omg`). |
| Function-body slice cost dominates the savings (per-function instead of per-instruction) | Function count is O(distinct procs), instruction count is O(emitted instructions). Slice cost is bounded by total instruction count, same order as old. Expected: still strictly faster. |
| Backing buffer persists across REPL turns or in-process driver invocations of `compile_source` | `compile_reset` and `compile_program_node_seeded` both drop the buffer (`:= []`) AND zero the counter. Belt-and-suspenders. |
| Triple-meta byte-identity drift if the optimisation accidentally changes the WHAT (not just the HOW) | Parity tests run on every test cycle. If `.omgb` bytes diverge, the byte-identical and triple-meta-fixed-point checks fail immediately and loudly. |
| OMG-VM also depends on capacity-tracking semantics? | No — the VM treats `funcs[name][1]` (the addr) as a plain integer and `code` as a list. Both come from `parse_bytecode`, which builds them fresh from `.omgb` bytes. The capacity-tracking is an internal detail of the compiler that doesn't leak through the bytecode format. |

## Validation

The cheap, automated validation is the existing test corpus. The
parity tests catch any output divergence:

- `bash tests/parity.sh` — triple-meta byte-identity across all
  18 corpus files.
- `bash tests/run.sh` — full 202-test suite.

The manual validation is timing. We have baseline numbers from the
VM-side change to reuse:

- **Self-rebuild time**: `time bash bootstrap/build.sh` from a clean
  `bootstrap/bin/` (delete the binaries, force rebuild from Rust).
  Currently ~25–30 s on this machine.
- **`--compile` on a large file**: `time bootstrap/bin/omgc
  bootstrap/src/compiler.omg /tmp/c.omgb`. Compiles ~2200 lines of
  OMG.
- **Triple-meta time**: `time bash tests/parity.sh`. Baseline
  10m40s; lots of process startup overhead, but the inner
  `--verify-omg-vm` calls feel compile-time too.

Target: at least 10% improvement on the `--compile` workload (same
order as the VM-side improvement). Anything more is a bonus.

## Where to start

1. **Audit phase.** `grep -n "cc_code\b\|cc_lines\b" bootstrap/src/compiler.omg`
   and classify every site: read (`length`, indexing) vs write
   (`:=`, append). Confirm the universe of changes.
2. **Add the counters.** Declare `cc_code_top := 0` and
   `cc_lines_top := 0` next to `cc_code` / `cc_lines`. No other
   changes yet — build should pass.
3. **Rewrite `cc_emit`.** Capacity-tracking push for both. Build
   should fail loudly on triple-meta parity because nothing reads
   the counters yet.
4. **Convert read sites.** Every `length(cc_code)` → `cc_code_top`.
   Every `length(cc_lines)` → `cc_lines_top`. Run `parity.sh` —
   should pass.
5. **Rewrite `compile_function_body`.** Save/restore pairs;
   extract slice on return. Run `parity.sh` — should still pass
   because the visible output (sliced) hasn't changed shape.
6. **Update reset paths.** `compile_reset`,
   `compile_program_node_seeded`. Look for any other state-reset I
   missed.
7. **Final audit.** Re-run `grep` to confirm every `cc_code` /
   `cc_lines` site is either internal to the helper procs we
   updated, or treats the value through the new counter.
8. **Timing pass.** Capture before-time (current bootstrap), apply
   change, measure after-time, document delta.

## Done means

- All 202 tests pass after the change.
- Triple-meta byte-identity (`--verify-omg-vm` against every example)
  passes — Rust frontend output and OMG-on-OMG output are still
  byte-identical.
- `bootstrap/bin/omgc bootstrap/src/compiler.omg /tmp/x.omgb` is
  measurably faster than before (target ≥10%, anything more is gravy).
- The compiler.omg diff is local: the `cc_emit` helpers, the
  save/restore in `compile_function_body`, the reset paths, and the
  ~5–10 `length(cc_code)` call sites. No bytecode-format change, no
  per-instruction format change, no opcode change.
- No new `_top` counter survives a `compile_reset` without being
  zeroed — verified manually by inspection.

## Out of scope

- Inline-cache LOAD/STORE (that's a VM-side change, separate plan).
- Pre-allocating dict pools for env (likely too complex for the
  payoff).
- Optimising `native-c.omg` / `native-js.omg`'s emit lists (similar
  shape but lower-frequency invocation; revisit if needed).
- Changing the bytecode format. The `.omgb` layout stays exactly as it
  is in v2 (0x000200).

## Open questions

- Worth applying the same pattern to `native-c.omg`'s emit loop?
  It builds C source via string concatenation in some places that
  may benefit from a similar pattern. Defer until after the
  compiler.omg change is in and measured.
- Should `compile_function_body`'s slice be replaced with a paired
  `(code, code_top)` return tuple to avoid the slice allocation? Adds
  one slice per function. Probably not worth the API churn — function
  count is small compared to instruction count, and parity-test
  sensitivity to API shape is non-zero.

## Outcome

Both attempts were **reverted** after measurement. Documented here so
the next person trying the same thing knows the trap.

### Attempt 1: capacity-tracking on `cc_code` / `cc_lines`

- Implemented the full plan: counters, mutation-style `cc_emit`,
  save/restore pairs in `compile_function_body`, slice-on-extract.
- All 202 tests passed. Triple-meta byte-identity held across the
  corpus. Diff was ~80 lines, well-contained.
- **Measured speedup: ~0%** on `omgc bootstrap/src/compiler.omg`
  (45–47s before, 45–47s after — within noise).

The plan's hypothesis was wrong. `cc_emit`'s `xs := xs + [v]` is
O(n) per call, but **the buffer never gets popped** at the top
level — it grows monotonically. Capacity-tracking only beats list-
concat when slots get *reused* (write-then-overwrite, like a true
operand stack). For a write-only buffer, every push falls into the
"extend" branch and costs exactly what `xs + [v]` did before — just
with extra bookkeeping per call. The same applies inside
`compile_function_body`, which resets to `[]` for each function
body, defeating any reuse.

### Attempt 2: capacity-tracking on `wb_buf` / `write_bytecode`

After phase profiling showed `write_bytecode` was the actual
dominant cost (~60% of `omgc compiler.omg`, building a ~50 KB byte
vector one byte at a time), I tried the same pattern there: global
`wb_buf` + `wb_buf_top`, mutation-style `bytes_append` / `emit_u32`
/ `emit_str` / `encode_instr`, slice on return.

- All 202 tests passed. Byte-identity held.
- **Measured: ~15% slower.** 51–54s instead of 45–47s; clean
  `bash bootstrap/build.sh` went from 3m03s to 3m39s.

Same root cause as Attempt 1: `wb_buf` only grows. Every byte hits
the "extend" branch. The bookkeeping (the `if wb_buf_top <
length(wb_buf)` branch + the increment + the global-variable
access) adds overhead per byte without unlocking any reuse benefit.

### Why this works for `vm_stack` but not for the compiler

The operand stack in `vm.omg` is the textbook case for capacity-
tracking: push-pop-push-pop, the depth oscillates within a small
range, the *same slots get rewritten thousands of times*. There the
optimisation cuts O(n) list-concats to O(1) overwrites and the
high-water mark stays low. That's where the 10% on `fib(25)` came
from.

The compiler's emit buffers (and the bytecode writer's byte buffer)
look superficially similar but are fundamentally different: they're
**write-only, monotonically growing**. There's nothing to amortise
because there's no reuse.

### What a real fix would need

OMG has no O(1)-amortised list-append primitive. Every `xs + [v]`
is O(n); there's no way to pre-allocate `n` zeros in O(n) (no
`[0] * n` either). The classic Vec<u8>-style doubling-on-grow
strategy isn't constructible in pure OMG because the "grow the
backing buffer to 2× current capacity" step itself goes through
`xs + [0]` n times = O(n²).

The real fix is a new runtime builtin. Two options:

1. **`list_extend_with_zero(xs, n)`** — extend `xs` by `n` zeros
   in O(n). Then amortised doubling in pure OMG becomes O(1)
   amortised: grow to capacity 16 → 32 → 64 → … on demand, do the
   actual element write via existing `xs[i] := v`.
2. **`list_with_capacity(n)`** — return a list of length `n` filled
   with zeros (or any sentinel). Similar effect; cleaner API.

Either needs implementation in all four backends:
- Rust runtime (`vm/builtins.rs`)
- C runtime (`omg_rt.h`)
- JS runtime (`omg_rt.js`)
- Plus telling the compiler about the builtin name.

That's a substantially bigger commitment than the original plan
called for, and it's a *runtime* change that affects the language
contract, not a pure-OMG refactor. Out of scope for this attempt.

### Status: reverted to baseline

Both attempted diffs have been reverted via `git checkout`. The
state of `bootstrap/src/compiler.omg` is what it was at the start
of this plan. The lesson is documented; the work is not in the
tree.

If someone picks this up again, the right starting point is adding
a `list_with_capacity` or `list_extend_with_zero` builtin, not
another pure-OMG attempt to work around the missing primitive.

### Attempt 3 (success): `list_repeat` runtime builtin + amortised doubling

Pivoted to the runtime-builtin approach. Added `list_repeat(item, count)`
to all four runtimes (Rust, C, JS, plus the OMG compiler's builtin
allowlist) — it returns a fresh list of `count` copies of `item`,
allocated in a single O(n) pass.

Then rewrote `compiler.omg`'s byte-buffer writer:

- `wb_buf` is a global pre-allocated buffer; `wb_buf_top` tracks
  live byte count.
- `bytes_append(b)` writes into the slot at `wb_buf_top`. When the
  buffer is full it doubles capacity via `list_repeat(0, new_cap)`
  and copies the live prefix.
- The doubling means total grow-work is O(n) amortised across all
  appends — the missing primitive made this possible.
- `emit_u32 / emit_i64 / emit_str / encode_instr` are all mutation-
  style (no buf passing); `write_bytecode` calls `wb_reset()` at
  entry and returns `wb_buf[0:wb_buf_top]` at exit.

**Measured results** (median of 3 runs, `omgc bootstrap/src/compiler.omg`):

| Workload | Before | After | Speedup |
|---|---:|---:|---:|
| `omgc bootstrap/src/compiler.omg` | ~46s | ~7s | **6.5×** |
| `bash bootstrap/build.sh` (clean rebuild) | 3m03s | 1m09s | **2.65×** |

Triple-meta byte-identity holds across the corpus. Full 202-test
suite passes. The lesson stands: **the right fix for an O(n²)
append-loop in OMG is a runtime builtin that allocates in O(n),
not a pure-OMG refactor.** Capacity-tracking only wins when slots
get *reused* (operand stack); for write-once buffers, the real
constraint is the missing pre-allocation primitive.

### Files touched (Attempt 3)

- `runtime/src/vm/builtins.rs` — added `list_repeat` handler.
- `runtime/src/compiler.rs` — added `"list_repeat"` to `builtin_names()`.
- `bootstrap/src/omg_rt.h` — added `omg_list_repeat` C helper.
- `bootstrap/src/omg_rt.js` — added `omg_list_repeat` JS helper +
  dispatch table entry.
- `bootstrap/src/native-c.omg` — wired `emit_builtin2("omg_list_repeat")`.
- `bootstrap/src/native-js.omg` — wired `emit_builtin2("omg_list_repeat")`.
- `bootstrap/src/compiler.omg` — added `"list_repeat"` to `cc_builtins`;
  rewrote the byte-buffer writer with `wb_buf` + `wb_buf_top` +
  doubling-on-grow; converted `bytes_append` / `emit_u32` /
  `emit_i64` / `emit_str` / `encode_instr` to mutation-style; replaced
  `write_bytecode`'s functional `buf := X(buf, ...)` chain with bare
  calls + final `wb_buf[0:wb_buf_top]` slice.

No bytecode format change. No opcode change. No runtime contract
change visible to existing OMG programs — just a new builtin
available to anyone who needs it.
