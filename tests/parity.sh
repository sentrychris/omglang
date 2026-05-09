#!/bin/bash
# Cross-implementation parity tests: the Rust runtime, the OMG-in-OMG
# compiler running on the Rust VM, the OMG-in-OMG VM, the native AOT
# path, and the native interpreted path should all produce the same
# output for the same inputs. These are the load-bearing tests for
# self-hosting correctness.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

# Examples expected to match across all paths. self_hosted has a known
# divergence on the native interpreted path (meta-interpreter mutates
# `current_dir` from inside its hosted scope, which the native runtime
# can't observe — orthogonal to the test surface).
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

# Examples we deliberately skip on certain paths.
INTERPRETED_SKIP=("examples/self_hosted.omg")

cd "$REPO_ROOT"

# === Triple-meta fixed-point check ====================================
section "Parity: triple-meta fixed-point"

# Compile bootstrap/compiler.omg via three paths and confirm the bytes
# match. This is the load-bearing self-hosting check.
fp_out=$("$OMG_RUST" --verify-omg-vm bootstrap/compiler.omg 2>&1)
if echo "$fp_out" | grep -q "OMG-on-OMG-VM output matches Rust output"; then
    pass "compiler.omg: Rust frontend == OMG-on-Rust-VM == OMG-on-OMG-VM"
else
    fail "triple-meta fixed-point" "$fp_out"
fi

# === omgc vs Rust frontend ============================================
section "Parity: omgc vs Rust frontend bytecode"

for src in "${EXAMPLES[@]}"; do
    local_omgb="$TMPDIR_TEST/$(basename "$src" .omg)-rust.omgb"
    native_omgb="$TMPDIR_TEST/$(basename "$src" .omg)-native.omgb"
    "$OMG_RUST" --compile "$src" "$local_omgb"
    "$OMGC_NATIVE" "$src" "$native_omgb"
    if cmp -s "$local_omgb" "$native_omgb"; then
        pass "byte-identical: $(basename "$src")"
    else
        fail "byte-identical: $(basename "$src")" \
             "Rust frontend and omgc produced different bytecode"
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
    skip=0
    for s in "${INTERPRETED_SKIP[@]}"; do
        [ "$s" = "$src" ] && skip=1
    done
    if [ "$skip" = 1 ]; then
        continue
    fi
    rust_out=$("$OMG_RUST" "$src" 2>&1)
    nat_out=$("$OMG_NATIVE" "$src" 2>&1)
    if [ "$rust_out" = "$nat_out" ]; then
        pass "native == Rust: $name"
    else
        fail "native == Rust: $name" "stdout differs"
    fi
done

# === Toolchain self-rebuild ===========================================
section "Parity: native toolchain self-rebuild"

# The toolchain should be able to rebuild itself with no Rust runtime
# involved. We trigger a fresh build and check the resulting binaries
# all run.
build_out=$("$REPO_ROOT/bootstrap/build-native-toolchain.sh" 2>&1)
if echo "$build_out" | grep -q "Self-rebuild from existing native toolchain"; then
    pass "build script chose self-rebuild path"
else
    fail "build script didn't self-rebuild" "$build_out"
fi

# After self-rebuild every binary is still callable.
for bin in "$OMG_NATIVE" "$OMGC_NATIVE" "$OMGCC_NATIVE" "$OMGVM_NATIVE"; do
    if "$bin" --help >/dev/null 2>&1 || \
       "$bin" -h >/dev/null 2>&1 || \
       echo 'quit' | "$bin" >/dev/null 2>&1; then
        pass "rebuilt binary works: $(basename "$bin")"
    else
        fail "rebuilt binary broken: $(basename "$bin")"
    fi
done
