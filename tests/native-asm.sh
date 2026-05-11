#!/bin/bash
# Tests for the OMG bytecode → x86_64 ELF native-asm backend.
#
# Phase 1 covers only PUSH_STR + EMIT + HALT, so each test is a
# hello-world-shaped program. We compile the .omg to .omgb with omgc,
# then to ELF with omgna, then run it and assert stdout matches what
# the Rust runtime produces for the same source — proving the
# generated machine code behaves identically to the reference VM.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

if [ ! -x "$OMGNA_NATIVE" ]; then
    echo -e "${RED}omgna missing.${NC} Run bootstrap/build.sh."
    exit 2
fi

section "native-asm (omgna): phases 1, 2, 3"

# Round-trip a .omg through omgc + omgna and compare ./<bin> stdout
# against the Rust runtime's output for the same source.
assert_omgna() {
    local name="$1" src_text="$2"
    local src="$TMPDIR_TEST/na-$name.omg"
    local omgb="$TMPDIR_TEST/na-$name.omgb"
    local elf="$TMPDIR_TEST/na-$name"
    printf '%s\n' "$src_text" > "$src"
    "$OMGC_NATIVE" "$src" "$omgb" >/dev/null 2>&1
    "$OMGNA_NATIVE" "$omgb" "$elf" >/dev/null 2>&1
    local elf_out rust_out
    elf_out=$("$elf" 2>&1) || true
    rust_out=$("$OMG_RUST" "$src" 2>&1) || true
    assert_eq "$name" "$rust_out" "$elf_out"
}

# === Phase 1: strings ===
assert_omgna "hello_world"   $';;;omg\nemit "hello, world"'
assert_omgna "two_emits"     $';;;omg\nemit "first line"\nemit "second line"'
assert_omgna "empty_string"  $';;;omg\nemit ""'
assert_omgna "long_string"   $';;;omg\nemit "abcdefghijklmnopqrstuvwxyz 0123456789 ABCDEFGHIJKLMNOPQRSTUVWXYZ"'

# === Phase 2: integers + arithmetic ===
assert_omgna "int_42"        $';;;omg\nemit 42'
assert_omgna "int_zero"      $';;;omg\nemit 0'
assert_omgna "int_one"       $';;;omg\nemit 1'
assert_omgna "int_negative"  $';;;omg\nemit -42'
assert_omgna "int_large"     $';;;omg\nemit 9999999'
assert_omgna "add"           $';;;omg\nemit 1+2'
assert_omgna "sub"           $';;;omg\nemit 10-3'
assert_omgna "mul"           $';;;omg\nemit 6*7'
assert_omgna "div"           $';;;omg\nemit 100/7'
assert_omgna "mod"           $';;;omg\nemit 100 % 7'
assert_omgna "neg_expr"      $';;;omg\nemit -1*1000000'
assert_omgna "precedence"    $';;;omg\nemit 1+2*3'
assert_omgna "mixed_int_str" $';;;omg\nemit "answer:"\nemit 42'

# === Phase 3: bool/none, comparisons, control flow, globals ===
assert_omgna "bool_true"     $';;;omg\nemit true'
assert_omgna "bool_false"    $';;;omg\nemit false'
assert_omgna "cmp_lt"        $';;;omg\nemit 1 < 2'
assert_omgna "cmp_eq"        $';;;omg\nemit 5 == 5'
assert_omgna "cmp_gt"        $';;;omg\nemit 10 > 100'
assert_omgna "cmp_chain"     $';;;omg\nemit 1 < 2\nemit 2 <= 2\nemit 3 >= 3\nemit 4 != 5'
assert_omgna "if_yes"        $';;;omg\nif 1 < 2 {\n    emit "yes"\n} else {\n    emit "no"\n}'
assert_omgna "if_no"         $';;;omg\nif 100 < 2 {\n    emit "yes"\n} else {\n    emit "no"\n}'
assert_omgna "truthy_zero"   $';;;omg\nif 0 { emit "truthy" } else { emit "falsy" }'
assert_omgna "global_assign" $';;;omg\nalloc x := 42\nemit x'
assert_omgna "global_mutate" $';;;omg\nalloc x := 10\nx := x + 5\nemit x'
assert_omgna "loop_count"    $';;;omg\nalloc i := 0\nloop i < 3 {\n    emit i\n    i := i + 1\n}\nemit "done"'
assert_omgna "loop_countdown" $';;;omg\nalloc n := 5\nloop n > 0 {\n    emit n\n    n := n - 1\n}'
assert_omgna "fibonacci"     $';;;omg\nalloc a := 0\nalloc b := 1\nalloc i := 0\nloop i < 10 {\n    alloc t := a + b\n    a := b\n    b := t\n    i := i + 1\n}\nemit b'
assert_omgna "nested_if"     $';;;omg\nalloc x := 7\nif x > 5 {\n    if x < 10 {\n        emit "in range"\n    } else {\n        emit "too big"\n    }\n} else {\n    emit "too small"\n}'

# Binary should be a real statically-linked ELF, no libc dependency.
elf="$TMPDIR_TEST/na-hello_world"
if file "$elf" 2>/dev/null | grep -q "ELF 64-bit LSB executable, x86-64.*statically linked"; then
    pass "produces a statically-linked x86_64 ELF"
else
    fail "produces a statically-linked x86_64 ELF" "file output: $(file "$elf" 2>&1)"
fi

# Phase-1 binaries should be tiny — under 1 KB for hello-world.
size=$(wc -c < "$elf")
if [ "$size" -lt 1024 ]; then
    pass "hello-world ELF is <1 KB ($size bytes)"
else
    fail "hello-world ELF is <1 KB" "size: $size bytes"
fi
