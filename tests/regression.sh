#!/bin/bash
# Regression tests for specific bugs we've fixed. Each test exists
# because the bug was real, was caught (often painfully), and needs
# to stay fixed. Run via every applicable path.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

# === control_flow_in_try ==============================================
# Bug: `return`, `break`, and tail calls inside a `try` block didn't
# emit POP_BLOCK before exiting, leaving a stale SETUP_EXCEPT on the
# block stack. A *later* panic would then unwind to the dead handler
# and produce VmInvariant errors elsewhere in the program.
#
# The regression test has been part of the repo for a while; we just
# wire it through every path here.

section "Regression: control flow inside try"

CFT="$REPO_ROOT/tools/tests/control_flow_in_try.omg"
expected_cft=$("$OMG_RUST" "$CFT" 2>&1)

assert_stdout "control_flow_in_try via Rust VM" "$expected_cft" \
    "$OMG_RUST" "$CFT"

assert_stdout "control_flow_in_try via native interpreted" "$expected_cft" \
    "$OMG_NATIVE" "$CFT"

# AOT path
"$OMG_NATIVE" --build "$CFT" "$TMPDIR_TEST/cft-aot" >/dev/null 2>&1
assert_stdout "control_flow_in_try via native AOT" "$expected_cft" \
    "$TMPDIR_TEST/cft-aot"

# === PUSH_FLOAT decoding in compile_source ============================
# Bug: the OMG compiler emits `PUSH_FLOAT` with the f64's i64 bit
# pattern as the payload; bytecode decoders convert via bits_to_float.
# When `compile_source` returns code straight to the VM (no serialise/
# parse round-trip), that decode never happens, and PUSH_FLOAT pushes
# an int instead of a float. The unified `omg` driver hit this on
# every program with float literals.

section "Regression: float-bits decoding"

cat > "$TMPDIR_TEST/floats_smoke.omg" <<'EOF'
;;;omg
emit 1.5 + 2.5
emit sqrt(2)
emit 10 / 3.0
EOF

# Run via every path; must agree with Rust.
expected=$("$OMG_RUST" "$TMPDIR_TEST/floats_smoke.omg" 2>&1)

assert_stdout "floats: native interpreted" "$expected" \
    "$OMG_NATIVE" "$TMPDIR_TEST/floats_smoke.omg"

"$OMG_NATIVE" --build "$TMPDIR_TEST/floats_smoke.omg" "$TMPDIR_TEST/floats_aot" >/dev/null
assert_stdout "floats: AOT binary" "$expected" \
    "$TMPDIR_TEST/floats_aot"

# === args[0] semantics across all paths ===============================
# Documented behaviour: args[0] is the user-typed path on Rust runtime
# and the unified `omg` driver, and the binary's own path for AOT.
# Earlier the native subprocess driver leaked tempfile paths through
# args[0], breaking programs that printed "usage: " + args[0].

section "Regression: args[0] semantics"

cat > "$TMPDIR_TEST/argv0.omg" <<'EOF'
;;;omg
emit args[0]
EOF

# Rust runtime: args[0] = the script path the user typed.
out=$("$OMG_RUST" "$TMPDIR_TEST/argv0.omg")
assert_eq "args[0] via Rust = source path" \
    "$TMPDIR_TEST/argv0.omg" "$out"

# Native unified omg: same — source path the user typed (NOT a tempfile).
out=$("$OMG_NATIVE" "$TMPDIR_TEST/argv0.omg")
assert_eq "args[0] via native = source path (not a tempfile)" \
    "$TMPDIR_TEST/argv0.omg" "$out"

# AOT binary: args[0] is the binary's own path.
"$OMG_NATIVE" --build "$TMPDIR_TEST/argv0.omg" "$TMPDIR_TEST/argv0-aot" >/dev/null
out=$("$TMPDIR_TEST/argv0-aot")
assert_eq "args[0] via AOT = binary path" \
    "$TMPDIR_TEST/argv0-aot" "$out"

# === Error format: no doubled "RuntimeError:" prefix ==================
# Bug: vm.omg's panic-display path produced "RuntimeError:
# UndefinedIdentError: x" (doubled prefix) because vm_lookup called
# panic() and the host's panic-display added another "RuntimeError:"
# on top. Now stripped in step()'s except handler.

section "Regression: no doubled RuntimeError prefix"

cat > "$TMPDIR_TEST/undef_caught.omg" <<'EOF'
;;;omg
try {
    emit undefined_xyz
} except err {
    emit "caught: " + err
}
EOF

# Should emit a SINGLE prefix: "UndefinedIdentError: undefined_xyz".
# The fix lives in vm.omg's step() handler; only the bytecode-VM paths
# (Rust runtime, native interpreted) exercise it. The AOT path uses
# omg_rt.h's setjmp/longjmp directly without that wrapper, so it has
# no doubled-prefix issue to begin with — and currently doesn't error
# at all on undefined globals (a separate, pre-existing limitation
# tracked elsewhere). Skip AOT here.
expected="caught: UndefinedIdentError: undefined_xyz"
assert_stdout "no doubled prefix: native interpreted" "$expected" \
    "$OMG_NATIVE" "$TMPDIR_TEST/undef_caught.omg"

assert_stdout "no doubled prefix: Rust runtime" "$expected" \
    "$OMG_RUST" "$TMPDIR_TEST/undef_caught.omg"

# === ;;;omg header is optional ========================================
# Bug: the README and several docs claimed the `;;;omg` header was
# required and the compiler would refuse a file without it. Actually
# the lexer just strips it if present.

section "Regression: ;;;omg header is optional"

cat > "$TMPDIR_TEST/no_header.omg" <<'EOF'
emit "no header here"
EOF

assert_stdout "no header via Rust" "no header here" \
    "$OMG_RUST" "$TMPDIR_TEST/no_header.omg"

assert_stdout "no header via native" "no header here" \
    "$OMG_NATIVE" "$TMPDIR_TEST/no_header.omg"

# === Output buffering: stderr after stdout ============================
# Bug: native binaries used C's default block-buffered stdout when
# piped, causing stderr (unbuffered) to "jump" ahead of stdout in the
# merged output. A test program that printed N emits then errored
# would show the error BEFORE the emits. Now `setvbuf(stdout, _IOLBF)`
# is called at startup so stdout line-buffers like Rust's println!.

section "Regression: stdout/stderr ordering"

cat > "$TMPDIR_TEST/buffer_order.omg" <<'EOF'
;;;omg
emit "first"
emit "second"
emit "third"
panic("boom")
EOF

# When merged with 2>&1, stderr line should come AFTER all the stdout.
"$OMG_NATIVE" --build "$TMPDIR_TEST/buffer_order.omg" "$TMPDIR_TEST/buffer-aot" >/dev/null
actual=$("$TMPDIR_TEST/buffer-aot" 2>&1)
expected="first
second
third
RuntimeError: boom"
assert_eq "AOT stdout flushes before stderr" "$expected" "$actual"

# === Self-host: compiler.omg compiles itself byte-identically =========
# A core invariant: every iteration of compiler.omgb produces the same
# bytes when handed compiler.omg again. Drift here means we've broken
# self-hosting.

section "Regression: compiler.omg fixed point"

"$OMG_RUST"   --compile bootstrap/src/compiler.omg "$TMPDIR_TEST/cmp-rust.omgb"
"$OMGC_NATIVE" bootstrap/src/compiler.omg          "$TMPDIR_TEST/cmp-native.omgb"
if cmp -s "$TMPDIR_TEST/cmp-rust.omgb" "$TMPDIR_TEST/cmp-native.omgb"; then
    pass "compiler.omg via Rust == via omgc"
else
    fail "compiler.omg byte-identical" \
         "Rust frontend and omgc produced different bytecode"
fi

# === Compile-source uses no ambient state =============================
# Bug we *almost* hit: compiler.omg has module-level state
# (cc_code, cc_funcs, etc.). Without explicit reset between
# compile_source calls, leftover state from a previous compile would
# corrupt the next. compile_reset() exists for this.

section "Regression: compile_source isolates state"

# Place the test program in bootstrap/src/ so the relative `import
# "compiler.omg"` resolves against bootstrap/src/compiler.omg.
cat > "$REPO_ROOT/bootstrap/src/_two_progs_test.omg" <<'EOF'
;;;omg
import "compiler.omg" as cc

# Compile two structurally-identical programs through compile_source.
# If module state leaks (e.g. cc_code accumulates between calls), the
# second compile produces longer bytecode than the first. With proper
# isolation via compile_reset(), the two should match in length.
cc.compile_reset()
alloc a := cc.compile_source(";;;omg\nemit \"first\"\n", "<a>")
cc.compile_reset()
alloc b := cc.compile_source(";;;omg\nemit \"second\"\n", "<b>")

emit "len_a: " + length(a[0])
emit "len_b: " + length(b[0])
EOF

actual=$("$OMG_RUST" "$REPO_ROOT/bootstrap/src/_two_progs_test.omg" 2>&1)
rm -f "$REPO_ROOT/bootstrap/src/_two_progs_test.omg"
# The two programs are structurally identical, so their bytecode
# lengths must match. Specific lengths can change with compiler
# tweaks; we only assert equality.
len_a=$(echo "$actual" | sed -n 's/^len_a: //p')
len_b=$(echo "$actual" | sed -n 's/^len_b: //p')
if [ -n "$len_a" ] && [ "$len_a" = "$len_b" ]; then
    pass "compile_source: identical programs produce same-length bytecode (len=$len_a)"
else
    fail "compile_source: state isolation" \
         "len_a=$len_a len_b=$len_b ($actual)"
fi

# === Closure-as-arg across module boundary ================================
# Two related bugs hit while building tools/db (omgdb), where exec.omg
# needed to pass an alloc-page closure to btree.btree_insert across
# the module boundary:
#
# Bug 1 (VM half — vm.rs / vm.omg): the compiler mangles names with
# `__mod_N__` for module isolation, so the nested-proc body lives in
# the funcs table under e.g. `__mod_1__capt`. MakeFunc bound the env
# entry under that mangled key, but `Load("capt")` (the source-level
# reference) used the bare name. UndefinedIdentError.
#
# Bug 2 (AOT half — omg_rt.h's omg_dict_keys + native-c.omg's
# MAKE_FUNC emitter):
#   (a) omg_dict_keys returned aliases into the dict's owned key
#       storage. When the dict was freed, the list of returned keys
#       dangled; the captured_list inside native-c.omg's analyze
#       step ended up containing freed memory, so the wrong variable
#       got marked as captured.
#   (b) The MAKE_FUNC C-emission only assigned to the mangled C
#       variable; cross-module Load(bare) read a different,
#       never-assigned variable and saw omg_none() — surfacing as
#       "cannot call non-function value".
#
# Both halves are fixed; this regression locks them down across
# Rust runtime, native interpreter, AND AOT.

section "Regression: nested-closure-as-arg across module boundary"

cat > "$REPO_ROOT/bootstrap/src/_closure_mod_lib.omg" <<'OMG_LIB'
;;;omg
proc invoke_zero_arg(f) {
    return f()
}
proc identity(x) {
    return x
}
proc apply_with_closure(seed) {
    proc capt() {
        return identity(seed) + 1
    }
    return invoke_zero_arg(capt)
}
OMG_LIB

cat > "$TMPDIR_TEST/closure_mod_main.omg" <<MAIN_EOF
;;;omg
import "$REPO_ROOT/bootstrap/src/_closure_mod_lib.omg" as lib
emit lib.apply_with_closure(41)
MAIN_EOF

# All three paths must produce 42 (41 + 1 with `seed` correctly captured).
actual=$("$OMG_RUST" "$TMPDIR_TEST/closure_mod_main.omg" 2>&1)
assert_eq "Rust runtime: closure-as-arg across module" "42" "$actual"

actual=$("$OMG_NATIVE" "$TMPDIR_TEST/closure_mod_main.omg" 2>&1)
assert_eq "Native interp: closure-as-arg across module" "42" "$actual"

"$OMG_NATIVE" --build "$TMPDIR_TEST/closure_mod_main.omg" "$TMPDIR_TEST/closure_aot" >/dev/null
actual=$("$TMPDIR_TEST/closure_aot" 2>&1)
assert_eq "AOT binary: closure-as-arg across module" "42" "$actual"

rm -f "$REPO_ROOT/bootstrap/src/_closure_mod_lib.omg"

# === Regression: closures capture by reference ============================
#
# Closures must share the *cell* of an outer binding, not a snapshot
# of its value at MAKE_FUNC time. Without this, the classic
# `make_counter` pattern (mutate `n` from inside an inner proc) would
# return 1, 1, 1 instead of 1, 2, 3 — and each call to make_counter
# would also share state with previous calls.
#
# Three scenarios are covered:
#   1. Counter: inner proc mutates an outer local. Three calls to the
#      same counter must observe 1, 2, 3.
#   2. Independence: two counters built from the same factory must
#      have independent state (each call to the factory creates a
#      fresh cell).
#   3. Two-closure shared state: two closures returned together share
#      the same cell — a get/set pair.
#
# Verified across all four backends: Rust runtime, OMG VM, native
# interp, AOT, and (if node is available) native-js.

section "Regression: closures capture by reference"

cat > "$TMPDIR_TEST/closure_byref.omg" <<'OMG_EOF'
;;;omg
proc make_counter() {
    alloc n := 0
    proc tick() {
        n := n + 1
        return n
    }
    return tick
}

# Independent counters from the same factory must not share state.
alloc a := make_counter()
alloc b := make_counter()
emit a()
emit a()
emit a()
emit b()
emit b()
emit a()

# Two closures sharing the same cell — one reads, one writes.
proc make_pair() {
    alloc v := 100
    proc get() { return v }
    proc set(x) { v := x }
    return [get, set]
}
alloc pair := make_pair()
alloc g := pair[0]
alloc s := pair[1]
emit g()
s(7)
emit g()
s(g() + 1)
emit g()
OMG_EOF

EXPECTED=$'1\n2\n3\n1\n2\n4\n100\n7\n8'

actual=$("$OMG_RUST" "$TMPDIR_TEST/closure_byref.omg" 2>&1)
assert_eq "Rust runtime: closures by reference" "$EXPECTED" "$actual"

actual=$("$OMG_NATIVE" "$TMPDIR_TEST/closure_byref.omg" 2>&1)
assert_eq "Native interp: closures by reference" "$EXPECTED" "$actual"

# OMG VM (omgvm executes a .omgb produced by omgc).
"$OMGC_NATIVE" "$TMPDIR_TEST/closure_byref.omg" "$TMPDIR_TEST/closure_byref.omgb" >/dev/null
actual=$("$OMGVM_NATIVE" "$TMPDIR_TEST/closure_byref.omgb" 2>&1)
assert_eq "OMG VM: closures by reference" "$EXPECTED" "$actual"

"$OMG_NATIVE" --build "$TMPDIR_TEST/closure_byref.omg" "$TMPDIR_TEST/closure_byref_aot" >/dev/null
actual=$("$TMPDIR_TEST/closure_byref_aot" 2>&1)
assert_eq "AOT binary: closures by reference" "$EXPECTED" "$actual"

if command -v node >/dev/null 2>&1; then
    "$OMG_NATIVE" "$REPO_ROOT/bootstrap/src/native-js.omg" \
        "$TMPDIR_TEST/closure_byref.omgb" "$TMPDIR_TEST/closure_byref.js" >/dev/null
    actual=$(node "$TMPDIR_TEST/closure_byref.js" 2>&1)
    assert_eq "native-js: closures by reference" "$EXPECTED" "$actual"
fi
