#!/bin/bash
# Tests for the unified `omg` driver and its 5 invocation modes:
#   omg                      → REPL (covered separately in repl.sh)
#   omg <file.omg> [args]    → compile and run
#   omg <file.omgb> [args]   → run precompiled bytecode
#   omg --compile in out     → just compile
#   omg --build in [out]     → AOT to a native executable
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


# Bad usage: missing args → exit 1.
assert_exit_code "--compile with no args → exit 1" 1 \
    "$OMG_NATIVE" --compile

# === Mode 4: omg --build ===============================================
section "Driver: omg --build"

# AOT-build the fixture and confirm the binary works.
assert_exit_code "--build succeeds" 0 \
    "$OMG_NATIVE" --build "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-aot"
is_executable "$TMPDIR_TEST/fixture-aot" && pass "--build produces an executable" || \
    fail "--build output not executable"

# The AOT binary should produce the same output as the source. Its
# first argument is its own path, which Windows spells its own way
# (see assert_program_path), so that line is checked separately.
aot_out=$("$TMPDIR_TEST/fixture-aot" p q 2>/dev/null)
aot_self=$(printf '%s\n' "$aot_out" | sed -n '1s/^args: \[\(.*\), p, q\]$/\1/p')
assert_program_path "AOT binary sees its own path and arguments" \
    "$TMPDIR_TEST/fixture-aot" "$aot_self"
assert_eq "AOT binary produces correct output" \
    "len: 3
square(7) = 49" \
    "$(printf '%s\n' "$aot_out" | sed 1d)"

# AOT binary should be a real native executable (ELF on Linux, Mach-O
# on macOS, PE on Windows, where it is named fixture-aot.exe), not a
# shell script.
aot_file="$TMPDIR_TEST/fixture-aot"
[ -f "$aot_file.exe" ] && aot_file="$aot_file.exe"
file_output=$(file "$aot_file")
case "$file_output" in
    *ELF*|*Mach-O*|*PE32*) pass "AOT output is a native executable" ;;
    *) fail "AOT output is not a native executable" "got: $file_output" ;;
esac

# Default output path: input minus .omg.
"$OMG_NATIVE" --build "$TMPDIR_TEST/fixture.omg" >/dev/null 2>&1
is_executable "$TMPDIR_TEST/fixture" && pass "--build default output path" || \
    fail "--build default output path"

# === omg --build --target ==============================================
section "Driver: omg --build --target"

default_target=$(omg_default_target)
case "$default_target" in
    linux-x64|windows-x64|c) pass "--help names the default target ($default_target)" ;;
    *) fail "--help names the default target" "got: '$default_target'" ;;
esac

# Machine code for either system, from whichever system this is.
"$OMG_NATIVE" --build "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-lin" --target linux-x64 >/dev/null 2>&1
assert_eq "--target linux-x64 writes an ELF file" "7f454c46" "$(file_magic "$TMPDIR_TEST/fixture-lin")"
"$OMG_NATIVE" --build "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-win" --target windows-x64 >/dev/null 2>&1
case "$(file_magic "$TMPDIR_TEST/fixture-win.exe")" in
    4d5a*) pass "--target windows-x64 writes a PE file and adds .exe" ;;
    *) fail "--target windows-x64 writes a PE file and adds .exe" ;;
esac

# The same input gives the same bytes, wherever --target is written.
"$OMG_NATIVE" --build --target linux-x64 "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-lin2" >/dev/null 2>&1
if [ -s "$TMPDIR_TEST/fixture-lin" ] && cmp -s "$TMPDIR_TEST/fixture-lin" "$TMPDIR_TEST/fixture-lin2"; then
    pass "--build is reproducible (byte-identical across runs)"
else
    fail "--build is NOT reproducible"
fi

# Nothing else is started: the build works with an empty PATH.
if [ "$default_target" != c ] && [ -z "${OMG_NATIVE_WRAPPED:-}" ]; then
    if (PATH=/nonexistent "$OMG_NATIVE" --build "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-nopath" >/dev/null 2>&1) \
        && is_executable "$TMPDIR_TEST/fixture-nopath"; then
        pass "--build needs no other program (empty PATH)"
    else
        fail "--build needs no other program (empty PATH)"
    fi
fi

# The route through a C compiler is still there when asked for.
if [ -n "$(omg_c_compiler)" ]; then
    "$OMG_NATIVE" --build "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-viac" --target c >/dev/null 2>&1
    viac_out=$("$TMPDIR_TEST/fixture-viac" p q 2>/dev/null)
    assert_eq "--target c builds through the C compiler" \
        "len: 3
square(7) = 49" \
        "$(printf '%s\n' "$viac_out" | sed 1d)"
else
    echo "  (--target c skipped: no C compiler installed)"
fi

bad_target=$("$OMG_NATIVE" --build "$TMPDIR_TEST/fixture.omg" "$TMPDIR_TEST/fixture-bad" --target sparc 2>&1) && bad_target_rc=0 || bad_target_rc=$?
assert_contains "unknown --target is named in the error" "unknown target 'sparc'" "$bad_target"
assert_eq "unknown --target exits 1" "1" "$bad_target_rc"
assert_exit_code "--target with no name exits 1" 1 \
    "$OMG_NATIVE" --build "$TMPDIR_TEST/fixture.omg" --target

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

# The generated C should compile cleanly with the system C compiler,
# using the same link flags as `omg --build` (on Windows: gcc or clang,
# linked statically with Winsock).
emit_cc=$(omg_c_compiler)
emit_link="-lm"
if [ -n "$OMG_WINDOWS" ]; then
    emit_link="-static -lm -lws2_32"
fi
if [ -z "$emit_cc" ]; then
    echo "  (compiling the --emit-c output skipped: no C compiler installed)"
else
# shellcheck disable=SC2086
if "$emit_cc" -O2 -w "$TMPDIR_TEST/fixture-direct.c" -o "$TMPDIR_TEST/fixture-direct" $emit_link 2>/dev/null; then
    pass "--emit-c output compiles with cc"
else
    fail "--emit-c output failed to compile"
fi

# And the resulting binary runs correctly. Its own path is checked
# apart from the rest, as for the --build fixture above.
direct_out=$("$TMPDIR_TEST/fixture-direct" z 2>/dev/null)
direct_self=$(printf '%s\n' "$direct_out" | sed -n '1s/^args: \[\(.*\), z\]$/\1/p')
assert_program_path "binary built from --emit-c output sees its own path" \
    "$TMPDIR_TEST/fixture-direct" "$direct_self"
assert_eq "binary built from --emit-c output runs" \
    "len: 2
square(7) = 49" \
    "$(printf '%s\n' "$direct_out" | sed 1d)"
fi

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

# The exact listing, including the operands that are easy to get wrong
# (false, 0, floats). This is the format the Rust runtime's
# disassembler set; the two matched line for line when it was frozen.
cat > "$TMPDIR_TEST/disasm.omg" <<'EOF'
;;;omg
proc half(x) { return x / 2.0 }
alloc off := false
alloc on := true
emit half(0)
EOF
"$OMG_NATIVE" --compile "$TMPDIR_TEST/disasm.omg" "$TMPDIR_TEST/disasm.omgb" >/dev/null
disasm_expected='# functions
FUNC half (x) @ 9
# code
0000  MakeFunc("half")
0001  PushBool(false)
0002  StoreLocal("off")
0003  PushBool(true)
0004  StoreLocal("on")
0005  PushInt(0)
0006  Call("half")
0007  Emit
0008  Halt
0009  Load("x")
0010  PushFloat(2.0)
0011  Div
0012  Ret
0013  PushNone
0014  Ret'
assert_eq "--disasm of .omg prints the expected listing" \
    "$disasm_expected" "$("$OMG_NATIVE" --disasm "$TMPDIR_TEST/disasm.omg" | tr -d '\r')"
assert_eq "--disasm of .omgb prints the expected listing" \
    "$disasm_expected" "$("$OMG_NATIVE" --disasm "$TMPDIR_TEST/disasm.omgb" | tr -d '\r')"

# === The opcode tables agree =============================================
# compiler.omg writes opcodes, vm.omg dispatches on them, and
# bytecode.omg reads them back for the back ends. Each keeps its own
# copy of the table and of the format version (the reasons are in the
# files), so nothing but this check stops one of them drifting. The
# Rust runtime's unit tests used to do this job.
section "Driver: opcode tables"
opcode_table() {
    grep -E '^alloc (OP_[A-Z_0-9]+|BC_VERSION) +:=' "$1" \
        | tr -d '\r' | sed -E 's/ +/ /g; s/ *#.*$//' | sort
}
table_compiler=$(opcode_table "$REPO_ROOT/src/compiler.omg")
case "$table_compiler" in
    *"alloc BC_VERSION := "*"alloc OP_ADD := "*) pass "compiler.omg has an opcode table and a format version" ;;
    *) fail "compiler.omg has an opcode table and a format version" ;;
esac
assert_eq "vm.omg has the same opcodes and format version as compiler.omg" \
    "$table_compiler" "$(opcode_table "$REPO_ROOT/src/vm.omg")"
assert_eq "bytecode.omg has the same opcodes and format version as compiler.omg" \
    "$table_compiler" "$(opcode_table "$REPO_ROOT/src/bytecode.omg")"
