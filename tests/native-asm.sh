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

section "native-asm (omgna): phases 1-5d"

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

# === Phase 4: functions ===
assert_omgna "fn_simple"     $';;;omg\nproc add(a, b) { return a + b }\nemit add(3, 4)'
assert_omgna "fn_neg_args"   $';;;omg\nproc add(a, b) { return a + b }\nemit add(100, -50)'
assert_omgna "fn_nested"     $';;;omg\nproc inc(x) { return x + 1 }\nproc dbl(x) { return x * 2 }\nemit inc(dbl(5))'
assert_omgna "fn_locals"     $';;;omg\nproc compute(x) {\n    alloc r := x * 2\n    r := r + 5\n    return r\n}\nemit compute(10)'
assert_omgna "fn_factorial"  $';;;omg\nproc fact(n) {\n    if n <= 1 { return 1 }\n    return n * fact(n - 1)\n}\nemit fact(10)'
assert_omgna "fn_fibonacci"  $';;;omg\nproc fib(n) {\n    if n < 2 { return n }\n    return fib(n - 1) + fib(n - 2)\n}\nemit fib(15)'
assert_omgna "fn_mutual"     $';;;omg\nproc is_even(n) {\n    if n == 0 { return true }\n    return is_odd(n - 1)\n}\nproc is_odd(n) {\n    if n == 0 { return false }\n    return is_even(n - 1)\n}\nemit is_even(10)\nemit is_odd(7)\nemit is_even(0)'
assert_omgna "fn_5_args"     $';;;omg\nproc sum5(a, b, c, d, e) { return a + b + c + d + e }\nemit sum5(1, 2, 3, 4, 5)'
assert_omgna "fn_global"     $';;;omg\nalloc base := 1000\nproc add_base(x) { return x + base }\nemit add_base(42)'
assert_omgna "fn_tail_recur" $';;;omg\nproc fact_tail(n, acc) {\n    if n <= 1 { return acc }\n    return fact_tail(n - 1, n * acc)\n}\nemit fact_tail(10, 1)'

# === Phase 5a: lists, indexing, length ===
assert_omgna "list_basic"    $';;;omg\nalloc xs := [10, 20, 30]\nemit xs[0]\nemit xs[1]\nemit xs[2]'
assert_omgna "list_empty"    $';;;omg\nalloc xs := []\nemit length(xs)'
assert_omgna "list_single"   $';;;omg\nalloc xs := [42]\nemit xs[0]\nemit length(xs)'
assert_omgna "list_strings"  $';;;omg\nalloc fs := ["apple", "banana", "cherry"]\nemit fs[0]\nemit fs[1]\nemit fs[2]'
assert_omgna "list_in_loop"  $';;;omg\nalloc xs := [100, 200, 300, 400, 500]\nalloc i := 0\nloop i < length(xs) {\n    emit xs[i]\n    i := i + 1\n}'
assert_omgna "len_string"    $';;;omg\nemit length("hello")\nemit length("a")\nemit length("")'
assert_omgna "fn_list_arg"   $';;;omg\nproc get(xs, i) { return xs[i] }\nalloc xs := [10, 20, 30]\nemit get(xs, 0)\nemit get(xs, 2)'
assert_omgna "list_mixed"    $';;;omg\nalloc m := [1, "two", 3, "four"]\nemit m[0]\nemit m[1]\nemit m[2]\nemit m[3]'

# === Phase 5b: string concat ===
assert_omgna "concat_simple" $';;;omg\nemit "hello" + " " + "world"'
assert_omgna "concat_empty"  $';;;omg\nemit "" + "abc"\nemit "xyz" + ""\nemit "" + ""'
assert_omgna "concat_chain"  $';;;omg\nemit "a" + "b" + "c" + "d" + "e"'
assert_omgna "concat_in_fn"  $';;;omg\nproc greet(name) { return "Hello, " + name + "!" }\nemit greet("world")\nemit greet("OMG")'
assert_omgna "concat_loop"   $';;;omg\nalloc s := ""\nalloc i := 0\nloop i < 5 {\n    s := s + "x"\n    i := i + 1\n}\nemit s'
assert_omgna "concat_arg"    $';;;omg\nproc show(s) { emit s }\nshow("abc" + "def")\nshow("a" + "b" + "c")'
assert_omgna "ints_still_ok" $';;;omg\nemit 1 + 2\nemit 5 + 10 + 20'

# === Phase 5c: emit-on-list formatting ===
assert_omgna "emit_list_basic"  $';;;omg\nemit [1, 2, 3]'
assert_omgna "emit_list_empty"  $';;;omg\nemit []'
assert_omgna "emit_list_one"    $';;;omg\nemit [42]'
assert_omgna "emit_list_strs"   $';;;omg\nemit ["a", "b", "c"]'
assert_omgna "emit_list_mixed"  $';;;omg\nemit [1, "two", 3, "four"]'
assert_omgna "emit_list_nested" $';;;omg\nemit [[1, 2], [3, 4]]'
assert_omgna "emit_list_deep"   $';;;omg\nemit [[[1]], [[2, 3], [4]]]'
assert_omgna "emit_list_bools"  $';;;omg\nemit [true, false, true]'
assert_omgna "emit_list_mixed2" $';;;omg\nemit [1, [2, 3], "four", [5, [6, 7]]]'
assert_omgna "emit_list_ret"    $';;;omg\nproc make() { return [10, 20, 30] }\nemit make()'
assert_omgna "emit_list_var"    $';;;omg\nalloc xs := [1, 2, 3]\nemit xs'
assert_omgna "emit_list_many"   $';;;omg\nemit [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]'

# === Phase 5d: store_index, slice, list concat ===
assert_omgna "store_index_basic" $';;;omg\nalloc xs := [1, 2, 3]\nxs[1] := 99\nemit xs[0]\nemit xs[1]\nemit xs[2]'
assert_omgna "store_index_loop"  $';;;omg\nalloc xs := [0, 0, 0, 0, 0]\nalloc i := 0\nloop i < 5 {\n    xs[i] := i * 10\n    i := i + 1\n}\nemit xs'
assert_omgna "list_slice"        $';;;omg\nalloc xs := [10, 20, 30, 40, 50]\nemit xs[1:4]\nemit xs[0:0]\nemit xs[0:5]'
assert_omgna "string_slice"      $';;;omg\nemit "hello world"[0:5]\nemit "hello world"[6:11]\nemit "abc"[1:2]'
assert_omgna "slice_in_loop"     $';;;omg\nalloc s := "abcdefghij"\nalloc i := 0\nloop i < 5 {\n    emit s[i:i+2]\n    i := i + 1\n}'
assert_omgna "list_concat"       $';;;omg\nemit [1, 2] + [3, 4]\nemit [] + [1]\nemit [1] + []'
assert_omgna "list_concat_chain" $';;;omg\nalloc xs := [1, 2] + [3, 4] + [5, 6]\nemit xs\nemit length(xs)'
assert_omgna "merge_sort"        $';;;omg\nproc merge(xs, lo, mid, hi) {\n    alloc tmp := []\n    alloc i := lo\n    alloc j := mid\n    loop i < mid {\n        if j >= hi {\n            tmp := tmp + [xs[i]]\n            i := i + 1\n        } else {\n            if xs[i] <= xs[j] {\n                tmp := tmp + [xs[i]]\n                i := i + 1\n            } else {\n                tmp := tmp + [xs[j]]\n                j := j + 1\n            }\n        }\n    }\n    loop j < hi {\n        tmp := tmp + [xs[j]]\n        j := j + 1\n    }\n    alloc k := 0\n    loop k < length(tmp) {\n        xs[lo + k] := tmp[k]\n        k := k + 1\n    }\n}\nproc msort(xs, lo, hi) {\n    if hi - lo <= 1 { return false }\n    alloc mid := (lo + hi) / 2\n    msort(xs, lo, mid)\n    msort(xs, mid, hi)\n    merge(xs, lo, mid, hi)\n}\nalloc data := [5, 2, 8, 1, 9, 3, 7, 4, 6]\nmsort(data, 0, length(data))\nemit data'

# Binary should be a real statically-linked ELF, no libc dependency.
elf="$TMPDIR_TEST/na-hello_world"
if file "$elf" 2>/dev/null | grep -q "ELF 64-bit LSB executable, x86-64.*statically linked"; then
    pass "produces a statically-linked x86_64 ELF"
else
    fail "produces a statically-linked x86_64 ELF" "file output: $(file "$elf" 2>&1)"
fi

# Hello-world ELF size — the runtime blob grows as we add helpers
# (alloc, list build, concat, slice, list-aware repr dispatcher, etc).
# Bumped to 2 KB at phase 5d when list_concat + slice landed.
size=$(wc -c < "$elf")
if [ "$size" -lt 2048 ]; then
    pass "hello-world ELF is <2 KB ($size bytes)"
else
    fail "hello-world ELF is <2 KB" "size: $size bytes"
fi
