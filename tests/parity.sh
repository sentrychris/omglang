#!/bin/bash
# Cross-implementation parity tests: the interpreter (bytecode on the
# OMG-written VM), a built executable, the same through C, and
# JavaScript should all produce the same output for the same inputs,
# and the compiler must be the same compiler whichever of them runs
# it. These are the load-bearing tests for self-hosting correctness.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

# Examples expected to match across all paths.
EXAMPLES=(
    "examples/assignment.omg"
    "examples/bitwise.omg"
    "examples/dictionaries.omg"
    "examples/file_ops.omg"
    "examples/floats.omg"
    "examples/hello_world.omg"
    "examples/hex_to_rgb.omg"
    "examples/higher_order.omg"
    "examples/import_modules.omg"
    "examples/matrix_ops.omg"
    "examples/maze_solver.omg"
    "examples/merge_sort.omg"
    "examples/permissions.omg"
    "examples/prime_sieve.omg"
    "examples/rot_13.omg"
    "examples/stack_vm.omg"
    "examples/stack_vm_and_asm.omg"
    "examples/tabula_recta.omg"
)

cd "$REPO_ROOT"

# === The compiler on two engines ======================================
section "Parity: the compiler as machine code and on the OMG VM"

# Compile src/compiler.omg twice: by `omg` itself, and by omg.omg run
# from source, which puts the same compiler on the OMG-written VM
# (vm.omg) instead. The bytes must match. This is the load-bearing
# self-hosting check: two different ways of executing the compiler
# agree on what the compiler is.
"$OMG_NATIVE" --compile src/compiler.omg "$TMPDIR_TEST/compiler-direct.omgb" >/dev/null 2>&1
"$OMG_NATIVE" src/omg.omg --compile src/compiler.omg "$TMPDIR_TEST/compiler-on-vm.omgb" >/dev/null 2>&1
if [ -s "$TMPDIR_TEST/compiler-direct.omgb" ] && cmp -s "$TMPDIR_TEST/compiler-direct.omgb" "$TMPDIR_TEST/compiler-on-vm.omgb"; then
    pass "compiler.omg: built omg == omg.omg on the OMG VM"
else
    fail "compiler.omg: built omg == omg.omg on the OMG VM" \
         "the two produced different bytecode"
fi

# === Every way of running a program agrees ============================
# The examples carry no expected output of their own, so the back ends
# referee each other: the interpreter (bytecode on the OMG VM), a
# built executable, the same through C where a C compiler is
# installed, and JavaScript under node.
section "Parity: omg --build vs omg <file.omg>"

for src in "${EXAMPLES[@]}"; do
    name=$(basename "$src" .omg)
    bin="$TMPDIR_TEST/aot-$name"
    if ! "$OMG_NATIVE" --build "$src" "$bin" >/dev/null 2>&1; then
        fail "AOT build: $name"
        continue
    fi
    # Run the example from the repo root so relative paths in the
    # source resolve the same way for both.
    nat_out=$("$OMG_NATIVE" "$src" 2>&1)
    aot_out=$("$bin" 2>&1)
    if [ "$nat_out" = "$aot_out" ]; then
        pass "built == interpreted: $name"
    else
        fail "built == interpreted: $name" "stdout differs (truncated)"
    fi
done

if [ "$(omg_default_target)" != c ] && [ -n "$(omg_c_compiler)" ]; then
    section "Parity: omg --build --target c vs omg <file.omg>"
    for src in "${EXAMPLES[@]}"; do
        name=$(basename "$src" .omg)
        bin="$TMPDIR_TEST/aotc-$name"
        if ! "$OMG_NATIVE" --build "$src" "$bin" --target c >/dev/null 2>&1; then
            fail "build through C: $name"
            continue
        fi
        nat_out=$("$OMG_NATIVE" "$src" 2>&1)
        aot_out=$("$bin" 2>&1)
        if [ "$nat_out" = "$aot_out" ]; then
            pass "built through C == interpreted: $name"
        else
            fail "built through C == interpreted: $name" "stdout differs (truncated)"
        fi
    done
fi

# === native-js (OMG → JS, run via node) parity ========================
# Skipped if `node` isn't installed — the JS backend is opt-in.

if command -v node >/dev/null 2>&1; then
    section "Parity: native-js (node) vs omg <file.omg>"
    for src in "${EXAMPLES[@]}"; do
        name=$(basename "$src" .omg)
        jsfile="$TMPDIR_TEST/$name.js"
        if ! "$OMG_NATIVE" --emit-js "$src" "$jsfile" >/dev/null 2>&1; then
            fail "JS == interpreted: $name (--emit-js)"
            continue
        fi
        nat_out=$("$OMG_NATIVE" "$src" 2>&1)
        js_out=$(node "$jsfile" 2>&1)
        if [ "$nat_out" = "$js_out" ]; then
            pass "JS == interpreted: $name"
        else
            fail "JS == interpreted: $name" "stdout differs"
        fi
    done
fi

# === Toolchain self-rebuild ===========================================
section "Parity: native toolchain self-rebuild"

# The toolchain should be able to rebuild itself with nothing else
# involved. We trigger a fresh build and check the resulting binary
# runs, and that it is at its fixed point: the C that `omg` writes for
# its own source must be the same before and after the rebuild. (The C
# is compared, not the binary, because the binary records the absolute
# paths of its sources and so differs from one checkout to the next.)
"$OMG_NATIVE" --emit-c "$REPO_ROOT/src/omg.omg" "$TMPDIR_TEST/self-before.c" >/dev/null 2>&1
build_out=$("$REPO_ROOT/scripts/build.sh" 2>&1)
if echo "$build_out" | grep -q "Self-rebuild"; then
    pass "build script chose self-rebuild path"
else
    fail "build script didn't self-rebuild" "$build_out"
fi

if "$OMG_NATIVE" --help >/dev/null 2>&1; then
    pass "rebuilt omg works"
else
    fail "rebuilt omg is broken"
fi
"$OMG_NATIVE" --emit-c "$REPO_ROOT/src/omg.omg" "$TMPDIR_TEST/self-after.c" >/dev/null 2>&1
if [ -s "$TMPDIR_TEST/self-before.c" ] && cmp -s "$TMPDIR_TEST/self-before.c" "$TMPDIR_TEST/self-after.c"; then
    pass "self-rebuild is a fixed point (omg writes the same C for itself)"
else
    fail "self-rebuild is not a fixed point" \
         "bin/omg was built from older sources; run scripts/build.sh and commit"
fi

# Where omg writes the executable itself there is a stronger statement:
# rebuilding again gives the same file, byte for byte.
case "$(omg_default_target)" in
    linux-x64|windows-x64)
        cp "$OMG_NATIVE" "$TMPDIR_TEST/omg-rebuilt-once"
        "$REPO_ROOT/scripts/build.sh" >/dev/null 2>&1
        if cmp -s "$OMG_NATIVE" "$TMPDIR_TEST/omg-rebuilt-once"; then
            pass "a second rebuild is byte-identical"
        else
            fail "a second rebuild is byte-identical"
        fi ;;
esac
