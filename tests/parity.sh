#!/bin/bash
# Cross-implementation parity tests: the Rust runtime, the OMG-in-OMG
# compiler running on the Rust VM, the OMG-in-OMG VM, the native AOT
# path, and the native interpreted path should all produce the same
# output for the same inputs. These are the load-bearing tests for
# self-hosting correctness.

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

# === Triple-meta fixed-point check ====================================
section "Parity: triple-meta fixed-point"

# Compile bootstrap/src/compiler.omg via three paths and confirm the bytes
# match. This is the load-bearing self-hosting check.
fp_out=$("$OMG_RUST" --verify-omg-vm bootstrap/src/compiler.omg 2>&1)
if echo "$fp_out" | grep -q "OMG-on-OMG-VM output matches Rust output"; then
    pass "compiler.omg: Rust frontend == OMG-on-Rust-VM == OMG-on-OMG-VM"
else
    fail "triple-meta fixed-point" "$fp_out"
fi

# === native compiler vs Rust frontend ==================================
section "Parity: native omg vs Rust frontend bytecode"

for src in "${EXAMPLES[@]}"; do
    local_omgb="$TMPDIR_TEST/$(basename "$src" .omg)-rust.omgb"
    native_omgb="$TMPDIR_TEST/$(basename "$src" .omg)-native.omgb"
    "$OMG_RUST" --compile "$src" "$local_omgb"
    "$OMG_NATIVE" --compile "$src" "$native_omgb"
    if cmp -s "$local_omgb" "$native_omgb"; then
        pass "byte-identical: $(basename "$src")"
    else
        fail "byte-identical: $(basename "$src")" \
             "Rust frontend and native omg produced different bytecode"
    fi
done

# === AOT corpus parity ================================================
section "Parity: AOT (omg --build) vs Rust runtime"

for src in "${EXAMPLES[@]}"; do
    name=$(basename "$src" .omg)
    bin="$TMPDIR_TEST/aot-$name"
    if ! "$OMG_NATIVE" --build "$src" "$bin" >/dev/null 2>&1; then
        fail "AOT build: $name"
        continue
    fi
    # Run the example from the repo root so relative paths in the
    # source resolve the same way the Rust runtime sees them.
    rust_out=$("$OMG_RUST" "$src" 2>&1)
    aot_out=$("$bin" 2>&1)
    if [ "$rust_out" = "$aot_out" ]; then
        pass "AOT == Rust: $name"
    else
        fail "AOT == Rust: $name" "stdout differs (truncated)"
    fi
done

# === Native interpreted parity ========================================
section "Parity: native interpreted (omg <file.omg>) vs Rust runtime"

for src in "${EXAMPLES[@]}"; do
    name=$(basename "$src" .omg)
    rust_out=$("$OMG_RUST" "$src" 2>&1)
    nat_out=$("$OMG_NATIVE" "$src" 2>&1)
    if [ "$rust_out" = "$nat_out" ]; then
        pass "native == Rust: $name"
    else
        fail "native == Rust: $name" "stdout differs"
    fi
done

# === native-js (OMG → JS, run via node) parity ========================
# Skipped if `node` isn't installed — the JS backend is opt-in.

if command -v node >/dev/null 2>&1; then
    section "Parity: native-js (node) vs Rust runtime"
    for src in "${EXAMPLES[@]}"; do
        name=$(basename "$src" .omg)
        jsfile="$TMPDIR_TEST/$name.js"
        if ! "$OMG_NATIVE" --emit-js "$src" "$jsfile" >/dev/null 2>&1; then
            fail "JS == Rust: $name (--emit-js)"
            continue
        fi
        rust_out=$("$OMG_RUST" "$src" 2>&1)
        js_out=$(node "$jsfile" 2>&1)
        if [ "$rust_out" = "$js_out" ]; then
            pass "JS == Rust: $name"
        else
            fail "JS == Rust: $name" "stdout differs"
        fi
    done
fi

# === Toolchain self-rebuild ===========================================
section "Parity: native toolchain self-rebuild"

# The toolchain should be able to rebuild itself with no Rust runtime
# involved. We trigger a fresh build and check the resulting binary
# runs, and that it is at its fixed point: the C that `omg` writes for
# its own source must be the same before and after the rebuild. (The C
# is compared, not the binary, because the binary records the absolute
# paths of its sources and so differs from one checkout to the next.)
"$OMG_NATIVE" --emit-c "$REPO_ROOT/bootstrap/src/omg.omg" "$TMPDIR_TEST/self-before.c" >/dev/null 2>&1
build_out=$("$REPO_ROOT/bootstrap/build.sh" 2>&1)
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
"$OMG_NATIVE" --emit-c "$REPO_ROOT/bootstrap/src/omg.omg" "$TMPDIR_TEST/self-after.c" >/dev/null 2>&1
if [ -s "$TMPDIR_TEST/self-before.c" ] && cmp -s "$TMPDIR_TEST/self-before.c" "$TMPDIR_TEST/self-after.c"; then
    pass "self-rebuild is a fixed point (omg writes the same C for itself)"
else
    fail "self-rebuild is not a fixed point" \
         "bootstrap/bin/omg was built from older sources; run bootstrap/build.sh twice and commit"
fi
