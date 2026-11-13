#!/bin/bash
# The toolchain on its own: no C compiler, no Rust, no node. This is
# what a fresh clone can do on Linux or Windows (x86-64) with nothing
# installed, and CI runs it on machines set up that way.
#
#   - bin/omg rebuilds itself, and a second rebuild writes the same
#     bytes
#   - the conformance corpus passes interpreted and built to machine
#     code
#   - omg writes executables for the other system as well
#
# Usage: tests/standalone.sh
# Not part of tests/run.sh: that suite compares against the Rust
# runtime, and this one must work where there is none.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
trap 'rm -rf "$TMPDIR_TEST"' EXIT
cd "$REPO_ROOT"

section "Standalone: nothing else installed"
found=""
for tool in cc gcc clang cargo rustc; do
    command -v "$tool" >/dev/null 2>&1 && found="$found $tool"
done
if [ -z "$found" ]; then
    pass "no C compiler and no Rust on PATH"
elif [ -n "${OMG_STANDALONE_STRICT:-}" ]; then
    fail "no C compiler and no Rust on PATH" "found:$found"
else
    echo "  (found$found on PATH; the checks below do not use them)"
fi

if [ ! -x "$OMG_NATIVE" ]; then
    fail "the checked-in omg runs here" "$OMG_NATIVE is missing"
    exit 1
fi
default=$(omg_default_target)
case "$default" in
    linux-x64|windows-x64) pass "omg builds for $default by default" ;;
    *) fail "omg builds machine code by default" "default target: '${default:-unknown}'"; exit 1 ;;
esac

section "Standalone: self-rebuild"
build_out=$(scripts/build.sh 2>&1); rc=$?
if [ $rc = 0 ] && echo "$build_out" | grep -q "Self-rebuild"; then
    pass "bin/omg rebuilds itself"
else
    fail "bin/omg rebuilds itself" "$build_out"
fi
cp "$OMG_NATIVE" "$TMPDIR_TEST/omg-first"
build_out=$(scripts/build.sh 2>&1); rc=$?
if [ $rc = 0 ] && cmp -s "$OMG_NATIVE" "$TMPDIR_TEST/omg-first"; then
    pass "a second rebuild is byte-identical"
else
    fail "a second rebuild is byte-identical" "$build_out"
fi

CONF_DIR="$TMPDIR_TEST/conformance"
mkdir -p "$CONF_DIR"
source tests/conformance/check.sh

conf_run() {
    local backend="$1" f="$2"
    case "$backend" in
        native)
            "$OMG_NATIVE" "$f" >"$CONF_DIR/out" 2>"$CONF_DIR/err"
            CONF_RC=$? ;;
        aot)
            rm -f "$CONF_DIR/prog" "$CONF_DIR/prog.exe"
            if "$OMG_NATIVE" --build "$f" "$CONF_DIR/prog" >"$CONF_DIR/err" 2>&1; then
                "$CONF_DIR/prog" >"$CONF_DIR/out" 2>"$CONF_DIR/err"
                CONF_RC=$?
            else
                CONF_RC=9
            fi ;;
    esac
}

section "Standalone: conformance, interpreted"
conformance_backend native "native interpreter"

section "Standalone: conformance, built to machine code"
conformance_backend aot "omg --build"

section "Standalone: the other system"
"$OMG_NATIVE" --build examples/hello_world.omg "$TMPDIR_TEST/cross-win" --target windows-x64 >/dev/null 2>&1
case "$(file_magic "$TMPDIR_TEST/cross-win.exe")" in
    4d5a*) pass "--target windows-x64 writes a Windows executable" ;;
    *) fail "--target windows-x64 writes a Windows executable" ;;
esac
"$OMG_NATIVE" --build examples/hello_world.omg "$TMPDIR_TEST/cross-linux" --target linux-x64 >/dev/null 2>&1
case "$(file_magic "$TMPDIR_TEST/cross-linux")" in
    7f454c46) pass "--target linux-x64 writes a Linux executable" ;;
    *) fail "--target linux-x64 writes a Linux executable" ;;
esac
# The one for this system has to run, whichever it is.
ran=""
if [ "$default" = linux-x64 ]; then
    ran=$("$TMPDIR_TEST/cross-linux" 2>&1)
else
    ran=$("$TMPDIR_TEST/cross-win.exe" 2>&1 | tr -d '\r')
fi
assert_eq "and the one for this system runs" "Hello, World!" "$ran"

echo
echo "================================================"
if [ "$FAIL_COUNT" = 0 ]; then
    echo -e "${GREEN}All $TEST_COUNT standalone checks passed${NC}"
    exit 0
fi
echo -e "${RED}$FAIL_COUNT of $TEST_COUNT standalone checks failed${NC}"
echo -e "Failures:$FAILED_TESTS"
exit 1
