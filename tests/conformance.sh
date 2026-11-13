#!/bin/bash
# The conformance corpus (tests/conformance/) on the native backends:
#
#   - the native interpreter   bin/omg prog.omg
#   - ahead-of-time compiled   bin/omg --build prog.omg out
#   - the same through C       bin/omg --build prog.omg out --target c
#                              (where --build does not already mean
#                              that, and a C compiler is installed)
#   - compiled to JavaScript   bin/omg --emit-js prog.omg out.js, run under node
#
# tests/smoke.sh runs the same corpus on the backends that need nothing
# but the Rust runtime (the Rust VM through both frontends, the
# OMG-written VM, and the JS transpiler), so between the two scripts
# every way of running an OMG program executes every program in it.
#
# The last section holds the few checks that only make sense for a
# native executable, such as running out of machine stack.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

cd "$REPO_ROOT"

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
            # A build failure leaves its log where the caller looks.
            rm -f "$CONF_DIR/prog"
            if "$OMG_NATIVE" --build "$f" "$CONF_DIR/prog" >"$CONF_DIR/err" 2>&1; then
                "$CONF_DIR/prog" >"$CONF_DIR/out" 2>"$CONF_DIR/err"
                CONF_RC=$?
            else
                CONF_RC=9
            fi ;;
        aot_c)
            rm -f "$CONF_DIR/prog"
            if "$OMG_NATIVE" --build "$f" "$CONF_DIR/prog" --target c >"$CONF_DIR/err" 2>&1; then
                "$CONF_DIR/prog" >"$CONF_DIR/out" 2>"$CONF_DIR/err"
                CONF_RC=$?
            else
                CONF_RC=9
            fi ;;
        js)
            if "$OMG_NATIVE" --emit-js "$f" "$CONF_DIR/p.js" >"$CONF_DIR/err" 2>&1; then
                node "$CONF_DIR/p.js" >"$CONF_DIR/out" 2>"$CONF_DIR/err"
                CONF_RC=$?
            else
                CONF_RC=9
            fi ;;
    esac
}

section "Conformance: native interpreter"
conformance_backend native "native interpreter"

section "Conformance: AOT"
conformance_backend aot "AOT"

# The C back end is the route to every system the x86-64 one does not
# cover, and a second opinion on it where it does.
section "Conformance: AOT through C"
if [ "$(omg_default_target)" = c ]; then
    echo "  (same as the section above: --build goes through C here)"
elif [ -z "$(omg_c_compiler)" ]; then
    echo "  (skipped: no C compiler installed)"
else
    conformance_backend aot_c "AOT through C"
fi

section "Conformance: native JS back end"
if command -v node >/dev/null 2>&1; then
    conformance_backend js "omg --emit-js"
else
    echo "  (skipped: node not installed)"
fi

# === Native-only behaviour ============================================
section "Conformance: native stack limits"

# Recursion deep enough to exhaust the machine stack used to kill an AOT
# binary with a segfault and no message. It is now an ordinary
# RuntimeError: reported with exit code 1 when nothing catches it...
cat > "$CONF_DIR/deep.omg" <<'OMG'
;;;omg
proc depth(n) {
    if n == 0 {
        return 0
    }
    return 1 + depth(n - 1)
}
emit depth(200)
emit depth(10000000)
emit "unreachable"
OMG
"$OMG_NATIVE" --build "$CONF_DIR/deep.omg" "$CONF_DIR/deep" >/dev/null 2>&1
out=$("$CONF_DIR/deep" 2>"$CONF_DIR/deep.err"); rc=$?
assert_eq "AOT: runaway recursion exits with code 1" "1" "$rc"
assert_eq "AOT: output before the overflow is kept" "200" "$out"
assert_eq "AOT: runaway recursion is a RuntimeError" \
    "RuntimeError: maximum recursion depth exceeded" \
    "$(tail -n 1 "$CONF_DIR/deep.err")"

# ...and catchable, after which the program carries on with a usable
# stack.
cat > "$CONF_DIR/deep_caught.omg" <<'OMG'
;;;omg
proc depth(n) {
    if n == 0 {
        return 0
    }
    return 1 + depth(n - 1)
}
try {
    emit depth(10000000)
} except err {
    emit err
}
emit depth(200)
OMG
"$OMG_NATIVE" --build "$CONF_DIR/deep_caught.omg" "$CONF_DIR/deep_caught" >/dev/null 2>&1
assert_stdout "AOT: runaway recursion can be caught" \
    "RuntimeError: maximum recursion depth exceeded
200" \
    "$CONF_DIR/deep_caught"

# The interpreter keeps its call stack on the heap, so the same depth
# that stops an AOT binary is fine there. (100,000 levels took minutes
# before the VM's stacks grew by doubling.)
cat > "$CONF_DIR/deep_ok.omg" <<'OMG'
;;;omg
proc depth(n) {
    if n == 0 {
        return 0
    }
    return 1 + depth(n - 1)
}
emit depth(100000)
OMG
assert_stdout "native interpreter: 100,000-deep recursion" "100000" \
    "$OMG_NATIVE" "$CONF_DIR/deep_ok.omg"
assert_stdout "Rust VM: 100,000-deep recursion" "100000" \
    "$OMG_RUST" "$CONF_DIR/deep_ok.omg"
