#!/bin/bash
# The programs in tools/ are the largest OMG programs outside the
# toolchain, so they are run as a test of it: interpreted and as built
# executables.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

cd "$REPO_ROOT"

# === tools/test-all.omg ===================================================
section "Tools: tools/test-all.omg"

# It runs all 14 command-line tools against sample input and prints a
# summary line. Its sample files go in /tmp, so not on Windows.
if [ -n "$OMG_WINDOWS" ]; then
    echo "  (skipped on Windows: test-all.omg keeps its samples in /tmp)"
else
    want="RESULTS: 14 passed, 0 failed (14 total)"
    interp_out=$("$OMG_NATIVE" tools/test-all.omg 2>&1)
    assert_contains "every tool passes (interpreted)" "$want" "$interp_out"
    "$OMG_NATIVE" --build tools/test-all.omg "$TMPDIR_TEST/test-all" >/dev/null 2>&1
    built_out=$("$TMPDIR_TEST/test-all" 2>&1)
    assert_contains "every tool passes (built)" "$want" "$built_out"
    assert_eq "interpreted and built print the same" "$interp_out" "$built_out"
fi

# === The HTTP server answers a request ====================================
# tools/lib/http.omg forks a child per connection. Windows has no
# fork(), and the server used to die there on its first request; it
# falls back to serving in-process. This runs on every CI platform, so
# it covers both paths.
section "Tools: HTTP server (tools/site/http_server.omg)"

http_check() {
    local label="$1" port="$2"
    shift 2
    mkdir -p "$TMPDIR_TEST/webroot"
    echo "hello from omg" > "$TMPDIR_TEST/webroot/hello.txt"
    "$@" "$TMPDIR_TEST/webroot" "$port" >"$TMPDIR_TEST/http.log" 2>&1 &
    local pid=$! body="" code
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        body=$(curl -s -m 5 "http://127.0.0.1:$port/hello.txt" 2>/dev/null | tr -d '\r')
        [ -n "$body" ] && break
        sleep 1
    done
    if [ "$body" = "hello from omg" ]; then
        pass "serves a static file ($label)"
    else
        fail "serves a static file ($label)" "got: '$body'
$(cat "$TMPDIR_TEST/http.log")"
    fi
    # A second request proves the server survived the first one.
    code=$(curl -s -m 5 -o /dev/null -w '%{http_code}' "http://127.0.0.1:$port/missing.txt" 2>/dev/null)
    if [ "$code" = "404" ]; then
        pass "answers 404 for a missing file, and is still running ($label)"
    else
        fail "answers 404 for a missing file, and is still running ($label)" "status: '$code'
$(cat "$TMPDIR_TEST/http.log")"
    fi
    kill "$pid" 2>/dev/null
    wait "$pid" 2>/dev/null
}

if command -v curl >/dev/null 2>&1; then
    http_check "interpreted" 19876 "$OMG_NATIVE" tools/site/http_server.omg
    if "$OMG_NATIVE" --build tools/site/http_server.omg "$TMPDIR_TEST/http_server" >/dev/null 2>&1; then
        http_check "built" 19877 "$TMPDIR_TEST/http_server"
    else
        fail "http_server builds"
    fi
else
    echo "  (skipped: curl not installed)"
fi
