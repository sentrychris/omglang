#!/bin/bash
# Self-check of the frozen Rust runtime in rust/.
#
# The Rust runtime is where OMG started and is no longer part of the
# toolchain: nothing builds it, CI does not run this script, and
# tests/run.sh does not use it. This is the check to run by hand after
# building it, for instance before bootstrapping OMG from source alone
# (see rust/README.md). It needs nothing but that runtime, and runs the
# same way on Linux, macOS and Windows (Git Bash).
#
# It passed on the commit the runtime was frozen at. Later commits may
# use language features the runtime never learned, and parts of this
# script will then fail for that reason alone.
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
#   5. The conformance corpus (tests/conformance/) behaves the same on
#      every backend available here. tests/conformance.sh, part of
#      tests/run.sh, runs the same corpus on the native backends.
#   6. Malformed programs fail with a clean SyntaxError (exit code 1),
#      never a crash, and with the same message from both frontends.
#   7. The HTTP server in tools/ answers a request. It forks per
#      connection where fork() exists and must still serve where it
#      does not (Windows).
#
# Usage:
#   tests/smoke.sh            build first with: cargo build --release
#                             --manifest-path rust/Cargo.toml
#
# Exit codes: 0 all passed, 1 something failed, 2 runtime not built.

set -u
cd "$(dirname "$0")/.."

OMG=rust/target/release/omg
if [ ! -x "$OMG" ] && [ -x "$OMG.exe" ]; then
    OMG="$OMG.exe"
fi
if [ ! -x "$OMG" ]; then
    echo "Rust runtime missing. Run:"
    echo "  cargo build --release --manifest-path rust/Cargo.toml"
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
for check in "--verify-self-hosted src/compiler.omg" \
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
done < <(find src examples tools tests/smoke tests/conformance -name '*.omg' | sort)
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
        "$("$OMG" --rust src/vm.omg "$WORK/reg.omgb" 2>&1 | norm)"
    if command -v node >/dev/null 2>&1; then
        # The driver, run from source, reads the JS runtime (omg_rt.js)
        # from next to itself in src/.
        "$OMG" --rust src/omg.omg --emit-js "$WORK/reg.omgb" "$WORK/reg.js" \
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

# --- 5. Conformance corpus ---------------------------------------------------
echo
echo "== Conformance corpus =="
CONF_DIR="$WORK/conf"
mkdir -p "$CONF_DIR"
source tests/conformance/check.sh

conf_run() {
    local backend="$1" f="$2"
    case "$backend" in
        rust) "$OMG" --rust "$f" >"$CONF_DIR/out" 2>"$CONF_DIR/err"; CONF_RC=$? ;;
        self) "$OMG" "$f" >"$CONF_DIR/out" 2>"$CONF_DIR/err"; CONF_RC=$? ;;
        vm)
            "$OMG" --compile "$f" "$CONF_DIR/p.omgb" 2>"$CONF_DIR/err" &&
                "$OMG" --rust src/vm.omg "$CONF_DIR/p.omgb" \
                    >"$CONF_DIR/out" 2>"$CONF_DIR/err"
            CONF_RC=$? ;;
        js)
            # A build failure leaves its log where the caller looks.
            if "$OMG" --compile "$f" "$CONF_DIR/p.omgb" 2>"$CONF_DIR/err" &&
               "$OMG" --rust src/omg.omg --emit-js "$CONF_DIR/p.omgb" \
                    "$CONF_DIR/p.js" >"$CONF_DIR/err" 2>&1; then
                node "$CONF_DIR/p.js" >"$CONF_DIR/out" 2>"$CONF_DIR/err"
                CONF_RC=$?
            else
                CONF_RC=9
            fi ;;
    esac
}

conformance_backend rust "Rust VM, Rust frontend"
conformance_backend self "Rust VM, self-hosted compiler"
conformance_backend vm   "OMG-written VM"
if command -v node >/dev/null 2>&1; then
    conformance_backend js "JS transpiler"
else
    echo "  skip  JS transpiler (node not installed)"
fi

# --- 6. Malformed input fails cleanly ----------------------------------------
echo
echo "== Malformed programs fail cleanly =="
bad_program() {
    local name="$1" body="$2" want="$3" flag out rc first=""
    printf ';;;omg\n%b' "$body" > "$WORK/bad.omg"
    for flag in "--rust" ""; do
        # shellcheck disable=SC2086
        out=$("$OMG" $flag "$WORK/bad.omg" 2>&1 | norm)
        # Both frontends must word the diagnostic identically.
        if [ -z "$flag" ] && [ "$out" != "$first" ]; then
            fail "$name (same message from both frontends)" "--- rust frontend
$first
--- self-hosted
$out"
        fi
        first="$out"
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
# Elements need commas. The self-hosted compiler used to accept these.
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

# --- 7. The HTTP server answers a request -------------------------------------
# tools/lib/http.omg forks a child per connection. Windows has no
# fork(), and the server used to die there on its first request; it now
# falls back to serving in-process. This check runs on every CI
# platform, so it covers both paths.
echo
echo "== HTTP server (tools/site/http_server.omg) =="
if command -v curl >/dev/null 2>&1; then
    HTTP_PORT=19876
    mkdir -p "$WORK/webroot"
    echo "hello from omg" > "$WORK/webroot/hello.txt"
    "$OMG" --rust tools/site/http_server.omg "$WORK/webroot" "$HTTP_PORT" >"$WORK/http.log" 2>&1 &
    HTTP_PID=$!
    http_get() { curl -s -m 5 "http://127.0.0.1:$HTTP_PORT$1" 2>/dev/null | norm; }
    body=""
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        body=$(http_get /hello.txt)
        [ -n "$body" ] && break
        sleep 1
    done
    if [ "$body" = "hello from omg" ]; then
        pass "serves a static file"
    else
        fail "serves a static file" "got: '$body'
$(cat "$WORK/http.log")"
    fi
    # A second request proves the server survived the first one.
    code=$(curl -s -m 5 -o /dev/null -w '%{http_code}' "http://127.0.0.1:$HTTP_PORT/missing.txt" 2>/dev/null)
    if [ "$code" = "404" ]; then
        pass "answers 404 for a missing file, and is still running"
    else
        fail "answers 404 for a missing file, and is still running" "status: '$code'
$(cat "$WORK/http.log")"
    fi
    kill "$HTTP_PID" 2>/dev/null
    wait "$HTTP_PID" 2>/dev/null
else
    echo "  skip  HTTP server (curl not installed)"
fi

echo
echo "================================================"
if [ "$FAIL" -eq 0 ]; then
    echo "All $PASS smoke checks passed"
    exit 0
fi
echo "$FAIL of $((PASS + FAIL)) smoke checks failed"
exit 1
