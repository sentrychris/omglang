#!/bin/bash
# Portable smoke test for the Rust runtime.
#
# Unlike tests/run.sh (which needs the native toolchain and therefore a
# Unix host with a C compiler), this script needs nothing but the Rust
# runtime — so it runs the same way on Linux, macOS and Windows (Git
# Bash). CI runs it on all three.
#
# What it checks:
#   1. Both self-hosting fixed points (the OMG-written compiler, and the
#      OMG-written compiler on the OMG-written VM, each produce the same
#      bytes as the Rust frontend).
#   2. Both frontends produce byte-identical bytecode for every .omg file
#      in the repository.
#   3. Every example prints the same output through the Rust frontend
#      and through the self-hosted compiler.
#   4. tests/smoke/regressions.omg prints exactly regressions.expected on
#      every backend available here: Rust VM (both frontends), the
#      OMG-written VM, and — when node is installed — the JS transpiler.
#   5. Malformed programs fail with a clean SyntaxError (exit code 1),
#      never a crash.
#
# Usage:
#   tests/smoke.sh            build first with: cargo build --release
#                             --manifest-path runtime/Cargo.toml
#
# Exit codes: 0 all passed, 1 something failed, 2 runtime not built.

set -u
cd "$(dirname "$0")/.."

OMG=runtime/target/release/omg
if [ ! -x "$OMG" ] && [ -x "$OMG.exe" ]; then
    OMG="$OMG.exe"
fi
if [ ! -x "$OMG" ]; then
    echo "Rust runtime missing. Run:"
    echo "  cargo build --release --manifest-path runtime/Cargo.toml"
    exit 2
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

PASS=0
FAIL=0
pass() { PASS=$((PASS + 1)); echo "  ok    $1"; }
fail() {
    FAIL=$((FAIL + 1))
    echo "  FAIL  $1"
    if [ -n "${2:-}" ]; then
        echo "$2" | sed 's/^/        /'
    fi
    # Surface the failure in the GitHub Actions UI (and its API).
    if [ -n "${GITHUB_ACTIONS:-}" ]; then
        local detail
        detail=$(printf '%s' "${2:-}" | head -n 20 | tr '\n' '|')
        echo "::error title=smoke: $1::$detail"
    fi
}

# Program output is compared as text; tolerate a CRLF-translating stdout.
norm() { tr -d '\r'; }

echo "OMG smoke test ($("$OMG" --version))"

# --- 1. Fixed points ---------------------------------------------------------
echo
echo "== Self-hosting fixed points =="
for check in "--verify-self-hosted bootstrap/src/compiler.omg" \
             "--verify-omg-vm examples/prime_sieve.omg"; do
    # shellcheck disable=SC2086
    out=$("$OMG" $check 2>&1)
    if [ $? -eq 0 ]; then pass "$check"; else fail "$check" "$out"; fi
done

# --- 2. Frontend parity on every source file ---------------------------------
echo
echo "== Rust frontend == OMG frontend (bytecode) =="
count=0
bad=0
while IFS= read -r f; do
    count=$((count + 1))
    out=$("$OMG" --verify-self-hosted "$f" 2>&1)
    if [ $? -ne 0 ]; then
        bad=$((bad + 1))
        fail "bytecode parity: $f" "$out"
    fi
done < <(find bootstrap/src examples games tools tests/smoke -name '*.omg' | sort)
if [ "$bad" -eq 0 ]; then pass "identical bytecode for all $count .omg files"; fi

# --- 3. Examples: same output through both frontends -------------------------
echo
echo "== Examples: Rust frontend vs self-hosted compiler (output) =="
for f in examples/*.omg; do
    name=$(basename "$f" .omg)
    case "$name" in
        donut) continue ;;   # endless animation
    esac
    a=$("$OMG" --rust "$f" 2>&1 | norm); ra=$?
    b=$("$OMG" "$f" 2>&1 | norm); rb=$?
    if [ "$a" = "$b" ] && [ "$ra" = "$rb" ]; then
        pass "$name"
    else
        fail "$name" "--- rust frontend
$a
--- self-hosted
$b"
    fi
done

# --- 4. Regression program on every backend ----------------------------------
echo
echo "== Regression program =="
REG=tests/smoke/regressions.omg
EXPECTED=$(norm < tests/smoke/regressions.expected)

check_backend() {
    local name="$1" actual="$2"
    if [ "$actual" = "$EXPECTED" ]; then
        pass "$name"
    else
        fail "$name" "$(diff <(echo "$EXPECTED") <(echo "$actual") | head -n 20)"
    fi
}

check_backend "Rust VM, Rust frontend"        "$("$OMG" --rust "$REG" 2>&1 | norm)"
check_backend "Rust VM, self-hosted compiler" "$("$OMG" "$REG" 2>&1 | norm)"

"$OMG" --compile "$REG" "$WORK/reg.omgb" 2>"$WORK/compile.err"
if [ $? -eq 0 ]; then
    check_backend "OMG-written VM (vm.omg)" \
        "$("$OMG" --rust bootstrap/src/vm.omg "$WORK/reg.omgb" 2>&1 | norm)"
    if command -v node >/dev/null 2>&1; then
        # native-js.omg reads its runtime (omg_rt.js) from next to itself.
        "$OMG" --rust bootstrap/src/native-js.omg "$WORK/reg.omgb" "$WORK/reg.js" \
            >"$WORK/js.log" 2>&1
        if [ $? -eq 0 ]; then
            check_backend "JS transpiler (node)" "$(node "$WORK/reg.js" 2>&1 | norm)"
        else
            fail "JS transpiler (node)" "$(cat "$WORK/js.log")"
        fi
    else
        echo "  skip  JS transpiler (node not installed)"
    fi
else
    fail "compile regression program" "$(cat "$WORK/compile.err")"
fi

# --- 5. Malformed input fails cleanly ----------------------------------------
echo
echo "== Malformed programs fail cleanly =="
bad_program() {
    local name="$1" body="$2" want="$3" flag out rc
    printf ';;;omg\n%b' "$body" > "$WORK/bad.omg"
    for flag in "--rust" ""; do
        # shellcheck disable=SC2086
        out=$("$OMG" $flag "$WORK/bad.omg" 2>&1 | norm)
        # shellcheck disable=SC2086
        "$OMG" $flag "$WORK/bad.omg" >/dev/null 2>&1; rc=$?
        case "$out" in
            SyntaxError:*"$want"*)
                if [ "$rc" = 1 ]; then
                    pass "$name (${flag:-self-hosted})"
                else
                    fail "$name (${flag:-self-hosted})" "exit code $rc, expected 1"
                fi ;;
            *) fail "$name (${flag:-self-hosted})" "$out" ;;
        esac
    done
}

# No trailing newline on purpose: these used to index past the token
# stream and panic in the Rust parser.
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

echo
echo "================================================"
if [ "$FAIL" -eq 0 ]; then
    echo "All $PASS smoke checks passed"
    exit 0
fi
echo "$FAIL of $((PASS + FAIL)) smoke checks failed"
exit 1
