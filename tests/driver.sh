#!/bin/bash
# Tests for the unified `omg` driver and its 5 invocation modes:
#   omg                      → REPL (covered separately in repl.sh)
#   omg <file.omg> [args]    → compile and run
#   omg <file.omgb> [args]   → run precompiled bytecode
#   omg --compile in out     → just compile
#   omg --build in [out]     → AOT to native ELF
# Plus error/help paths.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

# A small fixture that exercises emit, args, arithmetic, and proc
# definition all at once. Used across several modes below so we know
# the same source produces the same output through each entry point.
cat > "$TMPDIR_TEST/fixture.omg" <<'EOF'
;;;omg
proc square(x) { return x * x }
emit "args: " + args
emit "len: " + length(args)
emit "square(7) = " + square(7)
EOF

# === Mode 1: omg <file.omg> ============================================
section "Driver: omg <file.omg>"

expected="args: [$TMPDIR_TEST/fixture.omg, foo, bar]
len: 3
square(7) = 49"
assert_stdout "compile and run with forwarded args" "$expected" \
    "$OMG_NATIVE" "$TMPDIR_TEST/fixture.omg" foo bar

# args[0] should be the user-typed path, NOT a tempfile (that was the
# old subprocess-driver bug).
cat > "$TMPDIR_TEST/argv0.omg" <<'EOF'
;;;omg
emit args[0]
EOF
assert_stdout "args[0] = user-typed source path" \
    "$TMPDIR_TEST/argv0.omg" \
    "$OMG_NATIVE" "$TMPDIR_TEST/argv0.omg"

# Missing file → ModuleImportError, exit 1.
assert_exit_code "missing source file → exit 1" 1 \
    "$OMG_NATIVE" "/no/such/file.omg"

# === Mode 2: omg <file.omgb> ===========================================
section "Driver: omg <file.omgb>"

# Compile fixture once, then run via the bytecode path.
"$OMG_NATIVE" --compile "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture.omgb" >/dev/null
expected="args: [$TMPDIR_TEST/fixture.omgb, x, y]
len: 3
square(7) = 49"
assert_stdout "run precompiled bytecode" "$expected" \
    "$OMG_NATIVE" "$TMPDIR_TEST/fixture.omgb" x y

# === Mode 3: omg --compile =============================================
section "Driver: omg --compile"

assert_exit_code "--compile produces a .omgb file" 0 \
    "$OMG_NATIVE" --compile "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-c.omgb"
[ -f "$TMPDIR_TEST/fixture-c.omgb" ] && pass "--compile output exists" || \
    fail "--compile output missing"

# Compile twice → byte-identical (deterministic compiler).
"$OMG_NATIVE" --compile "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-c1.omgb"
"$OMG_NATIVE" --compile "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-c2.omgb"
if cmp -s "$TMPDIR_TEST/fixture-c1.omgb" "$TMPDIR_TEST/fixture-c2.omgb"; then
    pass "--compile is reproducible (byte-identical across runs)"
else
    fail "--compile is NOT reproducible"
fi

# native --compile output matches Rust frontend's output.
"$OMG_RUST" --compile "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-rust.omgb" >/dev/null
if cmp -s "$TMPDIR_TEST/fixture-c1.omgb" "$TMPDIR_TEST/fixture-rust.omgb"; then
    pass "--compile byte-identical to Rust frontend"
else
    fail "--compile differs from Rust frontend"
fi

# Bad usage: missing args → exit 1.
assert_exit_code "--compile with no args → exit 1" 1 \
    "$OMG_NATIVE" --compile

# === Mode 4: omg --build ===============================================
section "Driver: omg --build"

# AOT-build the fixture and confirm the binary works.
assert_exit_code "--build succeeds" 0 \
    "$OMG_NATIVE" --build "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-aot"
[ -x "$TMPDIR_TEST/fixture-aot" ] && pass "--build produces an executable" || \
    fail "--build output not executable"

# The AOT binary should produce the same output as the source.
expected="args: [$TMPDIR_TEST/fixture-aot, p, q]
len: 3
square(7) = 49"
assert_stdout "AOT binary produces correct output" "$expected" \
    "$TMPDIR_TEST/fixture-aot" p q

# AOT binary should be a real native executable (ELF on Linux, Mach-O
# on macOS), not a shell script.
file_output=$(file "$TMPDIR_TEST/fixture-aot")
case "$file_output" in
    *ELF*|*Mach-O*) pass "AOT output is a native executable" ;;
    *) fail "AOT output is not a native executable" "got: $file_output" ;;
esac

# Default output path: input minus .omg.
"$OMG_NATIVE" --build "$TMPDIR_TEST/fixture.omg" >/dev/null 2>&1
[ -x "$TMPDIR_TEST/fixture" ] && pass "--build default output path" || \
    fail "--build default output path"

# === Mode 5: --help / no args ==========================================
section "Driver: --help / unknown args"

# --help exits 0 and mentions all five modes.
help_out=$("$OMG_NATIVE" --help)
assert_contains "--help mentions REPL"      "REPL"      "$help_out"
assert_contains "--help mentions --compile" "--compile" "$help_out"
assert_contains "--help mentions --build"   "--build"   "$help_out"

assert_exit_code "--help exits 0" 0 "$OMG_NATIVE" --help
assert_exit_code "-h exits 0"     0 "$OMG_NATIVE" -h

# Unknown file extension is rejected with a clear message.
unknown_out=$("$OMG_NATIVE" foo.txt 2>&1) && unknown_rc=0 || unknown_rc=$?
assert_contains "unknown extension reports type"  "unknown file type" "$unknown_out"
[ "$unknown_rc" != 0 ] && pass "unknown extension exits non-zero" || \
    fail "unknown extension should exit non-zero"

# === One stage at a time (--compile, --emit-c, --emit-js) ======
section "Driver: one stage at a time"

# --compile takes .omg → .omgb.
"$OMG_NATIVE" --compile "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-direct.omgb" >/dev/null
[ -f "$TMPDIR_TEST/fixture-direct.omgb" ] && pass "--compile produces .omgb" || \
    fail "--compile didn't produce output"

# `omg foo.omgb` runs the bytecode directly (bin/omg imports vm.omg in-process).
expected_vm="args: [$TMPDIR_TEST/fixture-direct.omgb, hi]
len: 2
square(7) = 49"
assert_stdout "omg runs .omgb in-process" "$expected_vm" \
    "$OMG_NATIVE" "$TMPDIR_TEST/fixture-direct.omgb" hi

# --emit-c turns .omgb → .c.
"$OMG_NATIVE" --emit-c "$TMPDIR_TEST/fixture-direct.omgb" "$TMPDIR_TEST/fixture-direct.c" >/dev/null
[ -f "$TMPDIR_TEST/fixture-direct.c" ] && pass "--emit-c produces .c" || \
    fail "--emit-c didn't produce output"

# The generated C should compile cleanly.
if cc -O2 -w "$TMPDIR_TEST/fixture-direct.c" -o "$TMPDIR_TEST/fixture-direct" -lm 2>/dev/null; then
    pass "--emit-c output compiles with cc"
else
    fail "--emit-c output failed to compile"
fi

# And the resulting binary runs correctly.
expected_dir="args: [$TMPDIR_TEST/fixture-direct, z]
len: 2
square(7) = 49"
assert_stdout "binary built from --emit-c output runs" "$expected_dir" \
    "$TMPDIR_TEST/fixture-direct" z

# --emit-c and --emit-js also take source directly, and give the same
# output as going through a .omgb first (only the first line, which
# names the input, differs).
"$OMG_NATIVE" --emit-c "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-src.c" >/dev/null
if cmp -s <(tail -n +2 "$TMPDIR_TEST/fixture-direct.c") <(tail -n +2 "$TMPDIR_TEST/fixture-src.c"); then
    pass "--emit-c from .omg == --emit-c from .omgb"
else
    fail "--emit-c from .omg differs from --emit-c from .omgb"
fi

if command -v node >/dev/null 2>&1; then
    "$OMG_NATIVE" --emit-js "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture.js" >/dev/null
    # Under node the program's args[0] is the script path, like the
    # other backends; the fixture prints it, so compare the last line.
    js_last=$(node "$TMPDIR_TEST/fixture.js" q 2>&1 | tail -n 1)
    assert_eq "--emit-js output runs under node" "square(7) = 49" "$js_last"
    "$OMG_NATIVE" --emit-js "$TMPDIR_TEST/fixture-direct.omgb" "$TMPDIR_TEST/fixture-bc.js" >/dev/null
    if cmp -s <(tail -n +2 "$TMPDIR_TEST/fixture.js") <(tail -n +2 "$TMPDIR_TEST/fixture-bc.js"); then
        pass "--emit-js from .omg == --emit-js from .omgb"
    else
        fail "--emit-js from .omg differs from --emit-js from .omgb"
    fi
fi

# A compile error stops --emit-c / --emit-js with the diagnostic alone.
printf ';;;omg\nemit [1 2]\n' > "$TMPDIR_TEST/bad-emit.omg"
bad_out=$("$OMG_NATIVE" --emit-c "$TMPDIR_TEST/bad-emit.omg" "$TMPDIR_TEST/bad-emit.c" 2>&1) && bad_rc=0 || bad_rc=$?
assert_contains "--emit-c reports a syntax error" "SyntaxError" "$bad_out"
[ "$bad_rc" != 0 ] && [ ! -f "$TMPDIR_TEST/bad-emit.c" ] && pass "--emit-c writes nothing on error" || \
    fail "--emit-c should fail and write nothing on a syntax error"
emit_usage=$("$OMG_NATIVE" --emit-js 2>&1) && eu_rc=0 || eu_rc=$?
assert_contains "--emit-js without arguments prints usage" "usage: omg --emit-js" "$emit_usage"

# --disasm reads a .omgb back and prints what --disasm of the source prints.
dis_src=$("$OMG_NATIVE" --disasm "$TMPDIR_TEST/fixture.omg")
dis_bc=$("$OMG_NATIVE" --disasm "$TMPDIR_TEST/fixture-direct.omgb")
assert_eq "--disasm of .omgb == --disasm of .omg" "$dis_src" "$dis_bc"

# And both match the Rust runtime's disassembler line for line, including
# the operands that are easy to get wrong (false, 0, floats).
cat > "$TMPDIR_TEST/disasm.omg" <<'EOF'
;;;omg
proc half(x) { return x / 2.0 }
alloc off := false
alloc on := true
emit half(0)
EOF
"$OMG_NATIVE" --compile "$TMPDIR_TEST/disasm.omg" "$TMPDIR_TEST/disasm.omgb" >/dev/null
assert_eq "--disasm of .omg == Rust runtime's" \
    "$("$OMG_RUST" --disasm "$TMPDIR_TEST/disasm.omg")" "$("$OMG_NATIVE" --disasm "$TMPDIR_TEST/disasm.omg")"
assert_eq "--disasm of .omgb == Rust runtime's" \
    "$("$OMG_RUST" --disasm "$TMPDIR_TEST/disasm.omgb")" "$("$OMG_NATIVE" --disasm "$TMPDIR_TEST/disasm.omgb")"
