#!/bin/bash
# Tests for the new process-control & I/O builtins added alongside the
# native toolchain: subprocess, exit, getpid, stdin_readline, print.
# Each builtin is exercised two ways: interpreted (`omg prog.omg`,
# bytecode on the OMG-written VM) and as a built executable, so a
# divergence between the two surfaces immediately.

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

# Run an OMG program two ways (interpreted and built) and assert the
# stdout matches. Useful for tests that don't need fancy stdin/stderr.
assert_both_paths() {
    local name="$1" src="$2" expected="$3"
    shift 3
    local interp_out native_out aot_bin
    interp_out=$("$OMG_NATIVE" "$src" "$@" 2>&1) && interp_rc=0 || interp_rc=$?

    aot_bin="$TMPDIR_TEST/$(basename "$src" .omg)-aot"
    "$OMG_NATIVE" --build "$src" "$aot_bin" >/dev/null 2>&1
    native_out=$("$aot_bin" "$@" 2>&1) && native_rc=0 || native_rc=$?

    if [ "$interp_out" = "$expected" ] && [ "$native_out" = "$expected" ]; then
        pass "$name"
    else
        fail "$name" "interpreted:   $(printf '%q' "$interp_out")
      built:         $(printf '%q' "$native_out")
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

# Failed exec (binary not found) is an error the program can catch, the
# same on every backend. (The native runtime used to fork first and
# hand back the child's exit code 127 instead.)
cat > "$TMPDIR_TEST/subprocess_missing.omg" <<'EOF'
;;;omg
try {
    alloc rc := subprocess(["/no/such/binary/anywhere"])
    emit "rc=" + rc
} except err {
    emit "caught"
}
EOF
assert_both_paths "subprocess missing-binary is catchable" \
    "$TMPDIR_TEST/subprocess_missing.omg" \
    "caught"

section "Builtins: exit"

# `exit(code)` propagates. We can't compare exit codes via stdout, so
# we run the binary and check $?.
cat > "$TMPDIR_TEST/exit_zero.omg" <<'EOF'
;;;omg
exit(0)
EOF
build_aot "$TMPDIR_TEST/exit_zero.omg" "$TMPDIR_TEST/exit_zero" >/dev/null
assert_exit_code "exit(0) → status 0" 0  "$TMPDIR_TEST/exit_zero"
assert_exit_code "exit(0) interpreted → 0" 0  "$OMG_NATIVE" "$TMPDIR_TEST/exit_zero.omg"

cat > "$TMPDIR_TEST/exit_seven.omg" <<'EOF'
;;;omg
exit(7)
EOF
build_aot "$TMPDIR_TEST/exit_seven.omg" "$TMPDIR_TEST/exit_seven" >/dev/null
assert_exit_code "exit(7) → status 7" 7  "$TMPDIR_TEST/exit_seven"
assert_exit_code "exit(7) interpreted → 7" 7  "$OMG_NATIVE" "$TMPDIR_TEST/exit_seven.omg"

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

actual=$(printf 'hello\nworld\n!\n' | "$OMG_NATIVE" "$TMPDIR_TEST/readline_echo.omg")
assert_eq "stdin_readline: interpreted"   "$expected" "$actual"

# Empty stdin (immediate EOF) — should produce no output, clean exit 0.
assert_exit_code "stdin_readline: EOF clean exit (native)" 0 \
    bash -c "echo -n '' | '$TMPDIR_TEST/readline_echo'"
assert_exit_code "stdin_readline: EOF clean exit (interpreted)" 0 \
    bash -c "echo -n '' | '$OMG_NATIVE' '$TMPDIR_TEST/readline_echo.omg'"

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

section "Builtins: stdin_read / stdin_read_bytes"

# stdin_read() slurps all of stdin to EOF as a UTF-8 string. The
# pipe-friendly counterpart to read_file().
cat > "$TMPDIR_TEST/stdin_read_text.omg" <<'EOF'
;;;omg
alloc text := stdin_read()
emit "len=" + length(text)
emit "first=" + text[0:5]
emit "last=" + text[length(text) - 4:]
EOF
build_aot "$TMPDIR_TEST/stdin_read_text.omg" "$TMPDIR_TEST/stdin_read_text" >/dev/null
expected="len=11
first=hello
last=orld"
actual=$(printf 'hello world' | "$TMPDIR_TEST/stdin_read_text")
assert_eq "stdin_read: AOT binary"      "$expected" "$actual"
actual=$(printf 'hello world' | "$OMG_NATIVE" "$TMPDIR_TEST/stdin_read_text.omg")
assert_eq "stdin_read: native interpreted" "$expected" "$actual"

# Empty stdin → empty string (NOT false, unlike stdin_readline). This
# matches read_file() semantics for an empty file.
cat > "$TMPDIR_TEST/stdin_read_empty.omg" <<'EOF'
;;;omg
alloc text := stdin_read()
emit "len=" + length(text)
EOF
build_aot "$TMPDIR_TEST/stdin_read_empty.omg" "$TMPDIR_TEST/stdin_read_empty" >/dev/null
actual=$(echo -n '' | "$TMPDIR_TEST/stdin_read_empty")
assert_eq "stdin_read: empty stdin → empty string (native)" "len=0" "$actual"
actual=$(echo -n '' | "$OMG_NATIVE" "$TMPDIR_TEST/stdin_read_empty.omg")
assert_eq "stdin_read: empty stdin → empty string (interpreted)" "len=0" "$actual"

# Multi-line input is preserved verbatim, including the trailing
# newline (read_file behaves the same way).
cat > "$TMPDIR_TEST/stdin_read_multiline.omg" <<'EOF'
;;;omg
alloc text := stdin_read()
emit "len=" + length(text)
emit "ends_with_newline=" + (text[length(text) - 1] == "\n")
EOF
build_aot "$TMPDIR_TEST/stdin_read_multiline.omg" "$TMPDIR_TEST/stdin_read_multiline" >/dev/null
expected="len=14
ends_with_newline=true"
actual=$(printf 'one\ntwo\nthree\n' | "$TMPDIR_TEST/stdin_read_multiline")
assert_eq "stdin_read: multi-line preserves newlines" "$expected" "$actual"

# stdin_read_bytes() is the binary-pipe form. Bytes 0-255 round-trip
# losslessly, including bytes that wouldn't be valid UTF-8.
cat > "$TMPDIR_TEST/stdin_read_bytes.omg" <<'EOF'
;;;omg
alloc bs := stdin_read_bytes()
emit "len=" + length(bs)
alloc i := 0
loop i < length(bs) {
    emit "b" + i + "=" + bs[i]
    i := i + 1
}
# What comes back is a byte list: it refuses a number that is no byte.
try {
    append(bs, 256)
    emit "took 256"
} except e {
    emit e
}
EOF
build_aot "$TMPDIR_TEST/stdin_read_bytes.omg" "$TMPDIR_TEST/stdin_read_bytes" >/dev/null
expected="len=4
b0=65
b1=66
b2=255
b3=0
ValueError: a byte list holds whole numbers from 0 to 255, not 256"
actual=$(printf '\x41\x42\xff\x00' | "$TMPDIR_TEST/stdin_read_bytes")
assert_eq "stdin_read_bytes: AOT preserves 0x00 / 0xff" "$expected" "$actual"
actual=$(printf '\x41\x42\xff\x00' | "$OMG_NATIVE" "$TMPDIR_TEST/stdin_read_bytes.omg")
assert_eq "stdin_read_bytes: interpreted preserves 0x00 / 0xff" "$expected" "$actual"

# Empty stdin → empty list.
cat > "$TMPDIR_TEST/stdin_read_bytes_empty.omg" <<'EOF'
;;;omg
alloc bs := stdin_read_bytes()
emit length(bs)
EOF
build_aot "$TMPDIR_TEST/stdin_read_bytes_empty.omg" "$TMPDIR_TEST/stdin_read_bytes_empty" >/dev/null
actual=$(echo -n '' | "$TMPDIR_TEST/stdin_read_bytes_empty")
assert_eq "stdin_read_bytes: empty stdin → empty list (native)" "0" "$actual"
actual=$(echo -n '' | "$OMG_NATIVE" "$TMPDIR_TEST/stdin_read_bytes_empty.omg")
assert_eq "stdin_read_bytes: empty stdin → empty list (interpreted)" "0" "$actual"

# Round-trip: read text via stdin_read_bytes, reassemble with
# bytes_to_string. Confirms the byte-list shape is identical to what
# string_bytes / file_open("rb") + file_read would produce.
cat > "$TMPDIR_TEST/stdin_round_trip.omg" <<'EOF'
;;;omg
alloc bs := stdin_read_bytes()
alloc s := bytes_to_string(bs)
emit s
EOF
build_aot "$TMPDIR_TEST/stdin_round_trip.omg" "$TMPDIR_TEST/stdin_round_trip" >/dev/null
actual=$(printf 'round trip' | "$TMPDIR_TEST/stdin_round_trip")
assert_eq "stdin_read_bytes round-trip via bytes_to_string" "round trip" "$actual"

section "Builtins: file_seek / file_tell / rb+ / wb+"

# Random-access read-after-write: write known bytes, seek mid-file,
# overwrite three of them, read everything back. Exercises wb+ (create
# for read+write+truncate) and the seek/tell pair.
cat > "$TMPDIR_TEST/seek_overwrite.omg" <<'EOF'
;;;omg
alloc h := file_open(args[1], "wb+")
file_write(h, [65, 66, 67, 68, 69, 70, 71, 72, 73, 74])
emit "wrote_at=" + file_tell(h)
file_seek(h, 4)
emit "seek_to=" + file_tell(h)
file_write(h, [120, 121, 122])
file_seek(h, 0)
alloc bs := file_read(h)
file_close(h)
emit "len=" + length(bs)
alloc i := 0
loop i < length(bs) {
    emit "b" + i + "=" + bs[i]
    i := i + 1
}
EOF
build_aot "$TMPDIR_TEST/seek_overwrite.omg" "$TMPDIR_TEST/seek_overwrite" >/dev/null

expected="wrote_at=10
seek_to=4
len=10
b0=65
b1=66
b2=67
b3=68
b4=120
b5=121
b6=122
b7=72
b8=73
b9=74"
actual=$("$OMG_NATIVE" "$TMPDIR_TEST/seek_overwrite.omg" "$TMPDIR_TEST/seek_a.bin")
assert_eq "wb+/seek/tell: interpreted" "$expected" "$actual"
actual=$("$TMPDIR_TEST/seek_overwrite" "$TMPDIR_TEST/seek_b.bin")
assert_eq "wb+/seek/tell: AOT binary"   "$expected" "$actual"
actual=$("$OMG_NATIVE" "$TMPDIR_TEST/seek_overwrite.omg" "$TMPDIR_TEST/seek_c.bin")
assert_eq "wb+/seek/tell: native interpreted" "$expected" "$actual"

# rb+ preserves existing contents (does NOT truncate). Create a file,
# reopen with rb+, overwrite the middle, and confirm the unwritten
# bytes survived.
cat > "$TMPDIR_TEST/rb_plus.omg" <<'EOF'
;;;omg
# Phase 1: create a 10-byte file (contents 0..9) via wb.
alloc h := file_open(args[1], "wb")
file_write(h, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9])
file_close(h)

# Phase 2: open existing for read+write WITHOUT truncating.
alloc h2 := file_open(args[1], "rb+")
file_seek(h2, 5)
file_write(h2, [99])
file_seek(h2, 0)
alloc bs := file_read(h2)
file_close(h2)
emit "len=" + length(bs)
alloc i := 0
loop i < length(bs) {
    emit "b" + i + "=" + bs[i]
    i := i + 1
}
EOF
build_aot "$TMPDIR_TEST/rb_plus.omg" "$TMPDIR_TEST/rb_plus" >/dev/null
expected="len=10
b0=0
b1=1
b2=2
b3=3
b4=4
b5=99
b6=6
b7=7
b8=8
b9=9"
actual=$("$OMG_NATIVE" "$TMPDIR_TEST/rb_plus.omg" "$TMPDIR_TEST/rbplus_a.bin")
assert_eq "rb+ preserves contents (interpreted)" "$expected" "$actual"
actual=$("$TMPDIR_TEST/rb_plus" "$TMPDIR_TEST/rbplus_b.bin")
assert_eq "rb+ preserves contents (AOT)"  "$expected" "$actual"

# file_seek with negative offset → ValueError.
cat > "$TMPDIR_TEST/seek_neg.omg" <<'EOF'
;;;omg
alloc h := file_open(args[1], "wb+")
file_write(h, [1, 2, 3])
try {
    file_seek(h, -1)
    emit "no-error"
} except err {
    emit "caught"
}
file_close(h)
EOF
actual=$("$OMG_NATIVE" "$TMPDIR_TEST/seek_neg.omg" "$TMPDIR_TEST/neg_a.bin" 2>&1)
assert_eq "file_seek negative offset rejected (interpreted)" "caught" "$actual"
build_aot "$TMPDIR_TEST/seek_neg.omg" "$TMPDIR_TEST/seek_neg" >/dev/null
actual=$("$TMPDIR_TEST/seek_neg" "$TMPDIR_TEST/neg_b.bin" 2>&1)
assert_eq "file_seek negative offset rejected (AOT)" "caught" "$actual"

# Invalid mode → ValueError.
cat > "$TMPDIR_TEST/bad_mode.omg" <<'EOF'
;;;omg
try {
    alloc h := file_open(args[1], "zzz")
    emit "no-error"
} except err {
    emit "caught"
}
EOF
actual=$("$OMG_NATIVE" "$TMPDIR_TEST/bad_mode.omg" "$TMPDIR_TEST/bad.bin" 2>&1)
assert_eq "file_open invalid mode rejected (interpreted)" "caught" "$actual"
build_aot "$TMPDIR_TEST/bad_mode.omg" "$TMPDIR_TEST/bad_mode" >/dev/null
actual=$("$TMPDIR_TEST/bad_mode" "$TMPDIR_TEST/bad_b.bin" 2>&1)
assert_eq "file_open invalid mode rejected (built)" "caught" "$actual"

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

# === Builtins added for the HTTP server ===================================
# These exist in every way of running a program, so each is checked in
# every way this machine has: interpreted, built, built through C when
# there is a C compiler, and (for the ones JavaScript has) under node.

# `assert_every_path <name> <src> <expected> <js: yes|no> [args...]`
assert_every_path() {
    local name="$1" src="$2" expected="$3" js="$4"
    shift 4
    local stem got problems=""
    stem="$TMPDIR_TEST/$(basename "$src" .omg)"
    got=$("$OMG_NATIVE" "$src" "$@" 2>&1)
    [ "$got" = "$expected" ] || problems="$problems
      interpreted:   $(printf '%q' "$got")"
    "$OMG_NATIVE" --build "$src" "$stem-built" >/dev/null 2>&1
    got=$("$stem-built" "$@" 2>&1)
    [ "$got" = "$expected" ] || problems="$problems
      built:         $(printf '%q' "$got")"
    if [ -n "$(omg_c_compiler)" ] && [ "$(omg_default_target)" != "c" ]; then
        "$OMG_NATIVE" --build "$src" "$stem-viac" --target c >/dev/null 2>&1
        got=$("$stem-viac" "$@" 2>&1)
        [ "$got" = "$expected" ] || problems="$problems
      through C:     $(printf '%q' "$got")"
    fi
    if [ "$js" = "yes" ] && command -v node >/dev/null 2>&1; then
        "$OMG_NATIVE" --emit-js "$src" "$stem.js" >/dev/null 2>&1
        got=$(node "$stem.js" "$@" 2>&1)
        [ "$got" = "$expected" ] || problems="$problems
      JavaScript:    $(printf '%q' "$got")"
    fi
    if [ -z "$problems" ]; then
        pass "$name"
    else
        fail "$name" "expected:      $(printf '%q' "$expected")$problems"
    fi
}

section "Builtins: file_read(h, max) / file_size / file_mtime / interrupted"

cat > "$TMPDIR_TEST/file_pieces.omg" <<'EOF'
;;;omg
alloc p := args[1]
alloc out := file_open(p, "w")
file_write(out, "hello, world\n")
file_close(out)
emit file_size(p)
alloc h := file_open(p, "r")
emit file_read(h, 5)
emit file_read(h, 100)
emit length(file_read(h, 10))
file_close(h)
alloc hb := file_open(p, "rb")
emit file_read(hb, 3)
file_close(hb)
alloc m := file_mtime(p)
alloc now := time_ms()
emit m > now - 60000 and m < now + 60000
try {
    file_size(p + ".missing")
} except err {
    emit "no such file"
}
emit interrupted()
EOF
assert_every_path "a file read a piece at a time, its size and its age" \
    "$TMPDIR_TEST/file_pieces.omg" \
    "13
hello
, world

0
[104, 101, 108]
true
no such file
0" yes "$TMPDIR_TEST/file_pieces.txt"

section "Builtins: tcp_nonblocking / tcp_poll / tcp_peer / tcp_write(h, data, offset)"

# One program holds both ends of a connection, so the order of events
# is its own: nothing here depends on timing.
cat > "$TMPDIR_TEST/socket_poll.omg" <<'EOF'
;;;omg
alloc port := int(args[1])
alloc l := tcp_listen("127.0.0.1", port)
tcp_nonblocking(l, true)
emit tcp_accept(l) == false
emit tcp_poll([l], [], 0)
alloc c := tcp_connect("127.0.0.1", port)
emit length(tcp_poll([l], [], 2000)[0])
alloc s := tcp_accept(l)
emit s != false
emit tcp_peer(s)[0:10]
tcp_nonblocking(s, true)
emit tcp_read(s, 100) == false
emit length(tcp_poll([s], [], 50)[0])
emit length(tcp_poll([], [s], 1000)[1])
emit tcp_write(c, "hello")
emit length(tcp_poll([s], [], 2000)[0])
emit bytes_to_string(tcp_read(s, 100))
emit tcp_write(s, "abcdef", 2)
emit bytes_to_string(tcp_read(c, 100))
tcp_close(c)
emit length(tcp_poll([s], [], 2000)[0])
emit length(tcp_read(s, 100))
tcp_close(s)
tcp_close(l)
EOF
assert_every_path "a socket that does not block, and waiting on several" \
    "$TMPDIR_TEST/socket_poll.omg" \
    "true
[[], []]
1
true
127.0.0.1:
true
0
1
5
1
hello
4
cdef
1
0" no 19881

section "Builtins: byte lists through files and sockets"

# Raw bytes come back as byte lists, whichever builtin hands them over,
# and the builtins that write take a byte list or a list of numbers.
# is_bytes tells the two apart the only way a program can: a byte list
# will not hold 256.
cat > "$TMPDIR_TEST/bytes_files.omg" <<'EOF'
;;;omg
proc is_bytes(x) {
    alloc c := x[0:0]
    try {
        append(c, 256)
    } except e {
        return true
    }
    return false
}
alloc path := args[1]
alloc all := bytes_repeat(0, 256)
alloc i := 0
loop i < 256 {
    all[i] := i
    i := i + 1
}
alloc h := file_open(path, "wb")
file_write(h, all)
file_write(h, [255, 0, 128])
file_write(h, all[10:13])
file_write(h, to_bytes("né"))
file_close(h)
emit file_size(path)
h := file_open(path, "rb")
alloc got := file_read(h)
file_close(h)
emit is_bytes(got)
emit length(got)
emit got[0:256] == all
emit got[256:]
emit bytes_to_string(got[262:])
h := file_open(path, "rb")
alloc piece := file_read(h, 4)
emit is_bytes(piece)
emit piece
emit length(file_read(h))
emit file_read(h) == []
emit is_bytes(file_read(h, 10))
file_close(h)
h := file_open(path, "r")
emit is_bytes(string_bytes(file_read(h, 3)))
file_close(h)
# A list with a number that is no byte is refused, and so is text.
h := file_open(path, "wb")
try {
    file_write(h, [1, 2, 300])
} except e {
    emit "refused 300"
}
try {
    file_write(h, [1, "2"])
} except e {
    emit "refused a string in the list"
}
file_close(h)
EOF
assert_every_path "byte lists to a file and back" \
    "$TMPDIR_TEST/bytes_files.omg" \
    "265
true
265
true
[255, 0, 128, 10, 11, 12, 110, 195, 169]
né
true
[0, 1, 2, 3]
261
true
true
true
refused 300
refused a string in the list" yes "$TMPDIR_TEST/bytes_files.bin"

cat > "$TMPDIR_TEST/bytes_sockets.omg" <<'EOF'
;;;omg
proc is_bytes(x) {
    alloc c := x[0:0]
    try {
        append(c, 256)
    } except e {
        return true
    }
    return false
}
alloc port := int(args[1])
alloc l := tcp_listen("127.0.0.1", port)
alloc c := tcp_connect("127.0.0.1", port)
alloc s := tcp_accept(l)
emit tcp_write(c, to_bytes([0, 255, 7]))
alloc got := tcp_read(s, 100)
emit is_bytes(got)
emit got
emit tcp_write(s, [9, 8, 7, 6], 1)
emit tcp_read(c, 100)
emit tcp_write(s, to_bytes("abcdef"), 4)
emit bytes_to_string(tcp_read(c, 100))
# What was read goes straight back out, with a piece of it added on.
got := got + got[1:2]
emit tcp_write(s, got)
emit tcp_read(c, 100)
tcp_close(c)
alloc end := tcp_read(s, 100)
emit is_bytes(end)
emit end == []
tcp_close(s)
tcp_close(l)
EOF
assert_every_path "byte lists over a socket" \
    "$TMPDIR_TEST/bytes_sockets.omg" \
    "3
true
[0, 255, 7]
3
[8, 7, 6]
2
ef
4
[0, 255, 7, 255]
true
true" no 19882

# interrupted() counts requests to stop instead of letting them end the
# program. Not on Windows, where Git Bash's kill ends a native program
# outright rather than sending it anything.
if [ -z "$OMG_WINDOWS" ]; then
    cat > "$TMPDIR_TEST/interrupted.omg" <<'EOF'
;;;omg
emit interrupted()
emit "ready"
alloc n := 0
loop interrupted() == 0 and n < 100 {
    tcp_poll([], [], 100)
    n := n + 1
}
emit interrupted()
EOF
    interrupted_check() {
        local label="$1" tries=0 pid
        shift
        : > "$TMPDIR_TEST/interrupted.out"
        "$@" > "$TMPDIR_TEST/interrupted.out" 2>&1 &
        pid=$!
        while [ $tries -lt 50 ] && ! grep -q ready "$TMPDIR_TEST/interrupted.out" 2>/dev/null; do
            sleep 0.1
            tries=$((tries + 1))
        done
        kill -TERM "$pid" 2>/dev/null
        wait "$pid" 2>/dev/null
        assert_eq "interrupted() counts a request to stop, and tcp_poll wakes for it ($label)" "0
ready
1" "$(cat "$TMPDIR_TEST/interrupted.out")"
    }
    interrupted_check "interpreted" "$OMG_NATIVE" "$TMPDIR_TEST/interrupted.omg"
    "$OMG_NATIVE" --build "$TMPDIR_TEST/interrupted.omg" "$TMPDIR_TEST/interrupted-built" >/dev/null 2>&1
    interrupted_check "built" "$TMPDIR_TEST/interrupted-built"
    if [ -n "$(omg_c_compiler)" ] && [ "$(omg_default_target)" != "c" ]; then
        "$OMG_NATIVE" --build "$TMPDIR_TEST/interrupted.omg" "$TMPDIR_TEST/interrupted-viac" --target c >/dev/null 2>&1
        interrupted_check "through C" "$TMPDIR_TEST/interrupted-viac"
    fi
fi
