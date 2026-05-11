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

section "native-asm (omgna): phase 1 + phase 2"

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
