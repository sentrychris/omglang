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
# Every program in the corpus states what it must print, so each back
# end is judged against the same answer and, through it, against the
# others.
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

# === What the x86-64 runtime is stricter about =========================
# A sum, difference or product that does not fit in 64 bits is a
# ValueError there (C wraps round, JavaScript goes on as a float), a
# list or nothing in arithmetic is a TypeError, and so is a Bool in an
# ordering. The back end guesses that the operands of arithmetic in a
# loop are whole numbers, so these are also the cases where its guess
# is right about the kind and the result still has to be refused.
section "Conformance: x86-64 strictness"
case "$(omg_default_target)" in
linux-x64|windows-x64)
cat > "$CONF_DIR/strict.omg" <<'OMG'
;;;omg
proc kind(err) {
    alloc s := "" + err
    alloc i := 0
    loop i < length(s) {
        if s[i] == ":" {
            return s[0:i]
        }
        i := i + 1
    }
    return s
}
proc nothing() {
}
alloc nil := nothing()

alloc BIG := 9223372036854775807

proc grow(start, step, n) {
    alloc v := start
    alloc i := 0
    alloc msg := "fits"
    try {
        loop i < n {
            v := v + step
            i := i + 1
        }
    } except err {
        msg := kind(err)
    }
    return [v, i, msg]
}
emit grow(1, 1, 5)
emit grow(BIG - 3, 1, 5)
emit grow(0 - BIG, -1, 5)

proc scale(start, n) {
    alloc v := start
    alloc i := 0
    alloc msg := "fits"
    try {
        loop i < n {
            v := v * 1024
            i := i + 1
        }
    } except err {
        msg := kind(err)
    }
    return [v, i, msg]
}
emit scale(3, 4)
emit scale(3, 9)
emit scale(-3, 9)

proc negate(x) {
    try {
        return 0 - x - 1
    } except err {
        return kind(err)
    }
}
emit [negate(5), negate(BIG), negate(0 - BIG), negate(0 - BIG - 1)]

# What only this runtime refuses: a list or nothing in arithmetic, and
# a Bool in an ordering.
proc total_safe(xs) {
    alloc acc := 0
    alloc bad := 0
    alloc i := 0
    loop i < length(xs) {
        try {
            acc := acc + xs[i] * 2
        } except err {
            bad := bad + 1
        }
        i := i + 1
    }
    return [acc, bad]
}
emit total_safe([1, nil, 3, [4], 5])

proc bump_all(xs) {
    alloc out := []
    alloc i := 0
    loop i < length(xs) {
        try {
            append(out, xs[i] + 1)
        } except err {
            append(out, kind(err))
        }
        i := i + 1
    }
    return out
}
emit bump_all([10, [1], nil, 2.5])

proc count_below(xs, limit) {
    alloc n := 0
    alloc i := 0
    loop i < length(xs) {
        try {
            if xs[i] < limit {
                n := n + 1
            }
        } except err {
            n := n + 100
        }
        i := i + 1
    }
    return n
}
emit count_below([1, nil, 9, true, 2], 5)
OMG
strict_expected="[6, 5, fits]
[9223372036854775807, 3, ValueError]
[-9223372036854775808, 1, ValueError]
[3298534883328, 4, fits]
[3458764513820540928, 6, ValueError]
[-3458764513820540928, 6, ValueError]
[-6, -9223372036854775808, 9223372036854775806, ValueError]
[18, 2]
[11, TypeError, TypeError, 3.5]
202"
assert_stdout "strict arithmetic (native interpreter)" "$strict_expected" \
    "$OMG_NATIVE" "$CONF_DIR/strict.omg"
"$OMG_NATIVE" --build "$CONF_DIR/strict.omg" "$CONF_DIR/strict" >/dev/null 2>&1
assert_stdout "strict arithmetic (AOT)" "$strict_expected" \
    "$CONF_DIR/strict"
;;
*)
    echo "  (skipped: --build goes through C here)"
;;
esac
