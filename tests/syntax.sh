#!/bin/bash
# Malformed programs fail cleanly: a SyntaxError on stderr, exit code
# 1, never a crash, and the same diagnostic from every command that
# compiles (run, --compile, --build).

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

cd "$REPO_ROOT"

section "Syntax: malformed programs fail cleanly"

bad_program() {
    local name="$1" body="$2" want="$3" out rc other
    printf ';;;omg\n%b' "$body" > "$TMPDIR_TEST/bad.omg"
    out=$("$OMG_NATIVE" "$TMPDIR_TEST/bad.omg" 2>&1 | tr -d '\r')
    "$OMG_NATIVE" "$TMPDIR_TEST/bad.omg" >/dev/null 2>&1; rc=$?
    case "$out" in
        SyntaxError:*"$want"*)
            if [ "$rc" = 1 ]; then
                pass "$name"
            else
                fail "$name" "exit code $rc, expected 1"
            fi ;;
        *) fail "$name" "$out" ;;
    esac
    # Nothing is written when the program does not compile, and the
    # other commands say the same thing.
    rm -f "$TMPDIR_TEST/bad.omgb" "$TMPDIR_TEST/bad-built" "$TMPDIR_TEST/bad-built.exe"
    other=$("$OMG_NATIVE" --compile "$TMPDIR_TEST/bad.omg" "$TMPDIR_TEST/bad.omgb" 2>&1 | tr -d '\r')
    if [ "$other" != "$out" ] || [ -f "$TMPDIR_TEST/bad.omgb" ]; then
        fail "$name (--compile says the same and writes nothing)" "$other"
    fi
    other=$("$OMG_NATIVE" --build "$TMPDIR_TEST/bad.omg" "$TMPDIR_TEST/bad-built" 2>&1 | tr -d '\r')
    if [ "$other" != "$out" ] || [ -f "$TMPDIR_TEST/bad-built" ] || [ -f "$TMPDIR_TEST/bad-built.exe" ]; then
        fail "$name (--build says the same and writes nothing)" "$other"
    fi
}

# No trailing newline on purpose: these used to index past the token
# stream.
bad_program "truncated import"          'import'                      "'import'"
bad_program "truncated alloc"           'alloc'                       "'alloc'"
bad_program "truncated attribute"       'emit x.'                     "'.'"
bad_program "return at top level"       'return 5\n'                  "'return' outside of a proc"
bad_program "break outside a loop"      'break\n'                     "'break' outside of loop"
bad_program "unterminated comment"      'emit 1\n/* never closed\n'   "Unterminated block comment"
bad_program "unterminated string"       'emit "abc\n'                 "Unterminated string"
bad_program "int literal out of range"  'emit 9223372036854775808\n'  "Invalid integer literal"
bad_program "non-ASCII identifier"      'alloc caf\xc3\xa9 := 1\n'    "Unexpected character"
bad_program "duplicate top-level alloc" 'alloc x := 1\nalloc x := 2\n' "already declared"
# Elements need commas.
bad_program "list without commas"       'emit [1 2 3]\n'              "Expected ']'"
bad_program "dict without commas"       'emit {a: 1 b: 2}\n'          "Expected '}'"
bad_program "call without commas"       'emit pow(2 3)\n'             "Expected ')'"
bad_program "slice without a start"     'emit "abc"[:2]\n'            "Unexpected token ':'"
# A nested proc cannot use a variable that its enclosing proc declares
# further down: the closure is built before that variable exists.
bad_program "nested proc reads a later local" \
    'proc outer() {\n  proc get() {\n    return late\n  }\n  alloc late := 1\n  return get()\n}\n' \
    "'late' is used in a nested proc before the enclosing proc declares it on line 4"
bad_program "nested proc calls a later sibling" \
    'proc outer() {\n  proc a() {\n    return b()\n  }\n  proc b() {\n    return 1\n  }\n  return a()\n}\n' \
    "'b' is used in a nested proc before the enclosing proc declares it on line 4"
bad_program "nested proc called with the wrong argument count" \
    'proc outer() {\n  proc f(a) {\n    return f(1, 2)\n  }\n  return f(1)\n}\n' \
    "function 'f' expects 1 argument, got 2"
