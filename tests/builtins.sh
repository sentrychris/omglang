#!/bin/bash
# Tests for the new process-control & I/O builtins added alongside the
# native toolchain: subprocess, exit, getpid, stdin_readline, print.
# Each builtin is exercised through both the Rust runtime and a native
# AOT binary, so a divergence between the two surfaces immediately.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

# Build a tiny .omg file, AOT-compile it, return the binary path. The
# binary is placed in TMPDIR_TEST so the runner can clean it up.
build_aot() {
    local src="$1" out="$2"
    "$OMG_NATIVE" --build "$src" "$out" >/dev/null 2>&1
    echo "$out"
}

# Run an OMG program two ways (Rust VM and native AOT) and assert the
# stdout matches. Useful for tests that don't need fancy stdin/stderr.
assert_both_paths() {
    local name="$1" src="$2" expected="$3"
    shift 3
    local rust_out native_out aot_bin
    rust_out=$("$OMG_RUST" "$src" "$@" 2>&1) && rust_rc=0 || rust_rc=$?

    aot_bin="$TMPDIR_TEST/$(basename "$src" .omg)-aot"
    "$OMG_NATIVE" --build "$src" "$aot_bin" >/dev/null 2>&1
    native_out=$("$aot_bin" "$@" 2>&1) && native_rc=0 || native_rc=$?

    if [ "$rust_out" = "$expected" ] && [ "$native_out" = "$expected" ]; then
        pass "$name"
    else
        fail "$name" "rust output: $(printf '%q' "$rust_out")
      native output: $(printf '%q' "$native_out")
      expected:      $(printf '%q' "$expected")"
    fi
}

section "Builtins: subprocess"

# subprocess returns the child's exit code. We test this by spawning a
# shell that exits with a chosen status — predictable and uses no OMG.
cat > "$TMPDIR_TEST/subprocess_basic.omg" <<'EOF'
;;;omg
emit subprocess(["sh", "-c", "exit 0"])
emit subprocess(["sh", "-c", "exit 7"])
emit subprocess(["sh", "-c", "exit 42"])
EOF
assert_both_paths "subprocess returns child exit code" \
    "$TMPDIR_TEST/subprocess_basic.omg" \
    "0
7
42"

# Child stdout is inherited by the parent — we capture it normally.
cat > "$TMPDIR_TEST/subprocess_stdout.omg" <<'EOF'
;;;omg
emit "before"
alloc rc := subprocess(["sh", "-c", "echo from-child"])
emit "after"
emit rc
EOF
assert_both_paths "subprocess inherits stdout" \
    "$TMPDIR_TEST/subprocess_stdout.omg" \
    "before
from-child
after
0"

# Argv-list with multiple args: each becomes a separate shell argument.
cat > "$TMPDIR_TEST/subprocess_argv.omg" <<'EOF'
;;;omg
subprocess(["printf", "%s|%s|%s", "a", "b c", "d"])
emit ""
EOF
assert_both_paths "subprocess argv items stay separate" \
    "$TMPDIR_TEST/subprocess_argv.omg" \
    "a|b c|d"

# Failed exec (binary not found) is catchable on the Rust runtime
# because std::process::Command::status() returns Err. The native
# C runtime's fork+execvp path forks unconditionally, so the child
# exits 127 and the parent gets that as the subprocess() return value
# (no exception). This is a documented divergence; we test the two
# behaviours separately rather than asserting parity.
cat > "$TMPDIR_TEST/subprocess_missing.omg" <<'EOF'
;;;omg
try {
    alloc rc := subprocess(["/no/such/binary/anywhere"])
    emit "rc=" + rc
} except err {
    emit "caught"
}
EOF
rust_out=$("$OMG_RUST" "$TMPDIR_TEST/subprocess_missing.omg" 2>/dev/null)
assert_eq "subprocess missing-binary: Rust raises catchable error" \
    "caught" "$rust_out"

# Native: child exits 127 from the failed exec (its stderr leaks the
# error message — we ignore it for this assertion).
build_aot "$TMPDIR_TEST/subprocess_missing.omg" "$TMPDIR_TEST/sp_missing-aot" >/dev/null
nat_out=$("$TMPDIR_TEST/sp_missing-aot" 2>/dev/null)
assert_eq "subprocess missing-binary: native returns 127" \
    "rc=127" "$nat_out"

section "Builtins: exit"

# `exit(code)` propagates. We can't compare exit codes via stdout, so
# we run the binary and check $?.
cat > "$TMPDIR_TEST/exit_zero.omg" <<'EOF'
;;;omg
exit(0)
EOF
build_aot "$TMPDIR_TEST/exit_zero.omg" "$TMPDIR_TEST/exit_zero" >/dev/null
assert_exit_code "exit(0) → status 0" 0  "$TMPDIR_TEST/exit_zero"
assert_exit_code "exit(0) via Rust → 0" 0  "$OMG_RUST" "$TMPDIR_TEST/exit_zero.omg"

cat > "$TMPDIR_TEST/exit_seven.omg" <<'EOF'
;;;omg
exit(7)
EOF
build_aot "$TMPDIR_TEST/exit_seven.omg" "$TMPDIR_TEST/exit_seven" >/dev/null
assert_exit_code "exit(7) → status 7" 7  "$TMPDIR_TEST/exit_seven"
assert_exit_code "exit(7) via Rust → 7" 7  "$OMG_RUST" "$TMPDIR_TEST/exit_seven.omg"

# exit(code) doesn't return — emits after it never run.
cat > "$TMPDIR_TEST/exit_terminates.omg" <<'EOF'
;;;omg
emit "before"
exit(0)
emit "after"
EOF
assert_both_paths "exit terminates immediately" \
    "$TMPDIR_TEST/exit_terminates.omg" \
    "before"

section "Builtins: getpid"

# getpid returns a positive integer. We can't predict the exact value,
# so we just check it parses and is > 0.
cat > "$TMPDIR_TEST/getpid_positive.omg" <<'EOF'
;;;omg
alloc p := getpid()
if p > 0 {
    emit "ok"
} else {
    emit "fail: " + p
}
EOF
assert_both_paths "getpid returns positive int" \
    "$TMPDIR_TEST/getpid_positive.omg" \
    "ok"

# getpid is stable within one process (calling twice returns the same
# value), but differs between separate process invocations.
cat > "$TMPDIR_TEST/getpid_stable.omg" <<'EOF'
;;;omg
alloc a := getpid()
alloc b := getpid()
if a == b {
    emit "stable"
} else {
    emit "unstable"
}
EOF
assert_both_paths "getpid stable within one process" \
    "$TMPDIR_TEST/getpid_stable.omg" \
    "stable"

section "Builtins: stdin_readline"

# Read three lines, echo them. Final stdin_readline returns false on
# EOF; we flip a flag to break out of the loop. (Top-level `return` is
# not valid OMG; only `proc` bodies have it.)
cat > "$TMPDIR_TEST/readline_echo.omg" <<'EOF'
;;;omg
alloc done := false
loop done == false {
    alloc line := stdin_readline()
    if line == false {
        done := true
    } else {
        emit "got: " + line
    }
}
EOF
build_aot "$TMPDIR_TEST/readline_echo.omg" "$TMPDIR_TEST/readline_echo" >/dev/null

# Pipe three lines in; expect three "got: ..." lines plus clean exit.
expected="got: hello
got: world
got: !"
actual=$(printf 'hello\nworld\n!\n' | "$TMPDIR_TEST/readline_echo")
assert_eq "stdin_readline: native binary" "$expected" "$actual"

actual=$(printf 'hello\nworld\n!\n' | "$OMG_RUST" "$TMPDIR_TEST/readline_echo.omg")
assert_eq "stdin_readline: Rust runtime"  "$expected" "$actual"

# Empty stdin (immediate EOF) — should produce no output, clean exit 0.
assert_exit_code "stdin_readline: EOF clean exit (native)" 0 \
    bash -c "echo -n '' | '$TMPDIR_TEST/readline_echo'"
assert_exit_code "stdin_readline: EOF clean exit (rust)" 0 \
    bash -c "echo -n '' | '$OMG_RUST' '$TMPDIR_TEST/readline_echo.omg'"

# A line containing CR shouldn't include the CR in the result (Windows
# line ending stripping).
cat > "$TMPDIR_TEST/readline_crlf.omg" <<'EOF'
;;;omg
alloc line := stdin_readline()
emit "len=" + length(line)
emit "line=[" + line + "]"
EOF
build_aot "$TMPDIR_TEST/readline_crlf.omg" "$TMPDIR_TEST/readline_crlf" >/dev/null
expected="len=3
line=[abc]"
actual=$(printf 'abc\r\n' | "$TMPDIR_TEST/readline_crlf")
assert_eq "stdin_readline: strips trailing CRLF" "$expected" "$actual"

section "Builtins: print"

# print(s) writes to stdout WITHOUT a trailing newline. Three prints
# in a row should produce one line.
cat > "$TMPDIR_TEST/print_no_newline.omg" <<'EOF'
;;;omg
print("a")
print("b")
print("c")
emit ""
EOF
assert_both_paths "print: no trailing newline" \
    "$TMPDIR_TEST/print_no_newline.omg" \
    "abc"

# print rejects non-strings with TypeError.
cat > "$TMPDIR_TEST/print_typeerror.omg" <<'EOF'
;;;omg
try {
    print(42)
} except err {
    emit err
}
EOF
assert_both_paths "print: rejects non-string" \
    "$TMPDIR_TEST/print_typeerror.omg" \
    "TypeError: print() expects a string"
