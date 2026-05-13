# Shared test helpers. Sourced by every tests/*.sh file.
# Defines assertion primitives, output capture, and pass/fail tracking.
# Tests call `pass` / `fail` directly; the runner aggregates counts.

# Colour codes (only when running on a tty).
if [ -t 1 ]; then
    RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; NC='\033[0m'
else
    RED=''; GREEN=''; YELLOW=''; NC=''
fi

# Counters & failure log live in env vars so subshells can update them.
TEST_COUNT=${TEST_COUNT:-0}
PASS_COUNT=${PASS_COUNT:-0}
FAIL_COUNT=${FAIL_COUNT:-0}
FAILED_TESTS=${FAILED_TESTS:-}

# Repo root, derived from this file's location. Sourcing tests can call
# any binary via $OMG_NATIVE / $OMG_RUST / $OMGC etc.
TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$TESTS_DIR/.." && pwd)"
OMG_NATIVE="$REPO_ROOT/bootstrap/bin/omg"
OMGC_NATIVE="$REPO_ROOT/bootstrap/bin/omgc"
OMGCC_NATIVE="$REPO_ROOT/bootstrap/bin/omgcc"
OMGJS_NATIVE="$REPO_ROOT/bootstrap/bin/omgjs"
OMG_RUST="$REPO_ROOT/runtime/target/release/omg"

# Per-suite tempdir. Cleaned up by the runner; tests just write into it.
TMPDIR_TEST="${TMPDIR_TEST:-/tmp/omg-tests-$$}"
mkdir -p "$TMPDIR_TEST"

# `pass` and `fail` update the running counts. Tests should call these
# rather than printing pass/fail themselves.
pass() {
    PASS_COUNT=$((PASS_COUNT+1))
    TEST_COUNT=$((TEST_COUNT+1))
    echo -e "  ${GREEN}✓${NC} $1"
}

fail() {
    FAIL_COUNT=$((FAIL_COUNT+1))
    TEST_COUNT=$((TEST_COUNT+1))
    FAILED_TESTS="$FAILED_TESTS\n  - $1"
    echo -e "  ${RED}✗${NC} $1"
    if [ -n "${2:-}" ]; then
        echo -e "      ${YELLOW}$2${NC}"
    fi
}

# Pretty section header.
section() {
    echo
    echo -e "${YELLOW}== $1 ==${NC}"
}

# `assert_eq <name> <expected> <actual>` — pass if expected == actual.
assert_eq() {
    local name="$1" expected="$2" actual="$3"
    if [ "$expected" = "$actual" ]; then
        pass "$name"
    else
        fail "$name" "expected: $(printf '%q' "$expected")
      got:      $(printf '%q' "$actual")"
    fi
}

# `assert_contains <name> <substring> <haystack>` — pass if substring is in haystack.
assert_contains() {
    local name="$1" needle="$2" haystack="$3"
    case "$haystack" in
        *"$needle"*) pass "$name" ;;
        *) fail "$name" "expected to contain: $(printf '%q' "$needle")
      got: $(printf '%q' "$haystack")" ;;
    esac
}

# `assert_exit_code <name> <expected> <command...>` — run command, check
# its exit status.
assert_exit_code() {
    local name="$1" expected="$2"
    shift 2
    "$@" >/dev/null 2>&1
    local actual=$?
    if [ "$actual" = "$expected" ]; then
        pass "$name"
    else
        fail "$name" "exit code expected=$expected actual=$actual"
    fi
}

# `assert_stdout <name> <expected> <command...>` — run command, compare
# its stdout to expected.
assert_stdout() {
    local name="$1" expected="$2"
    shift 2
    local actual
    actual=$("$@" 2>/dev/null)
    assert_eq "$name" "$expected" "$actual"
}

# `assert_combined <name> <expected> <command...>` — like assert_stdout
# but captures stderr too. Useful for error-reporting tests.
assert_combined() {
    local name="$1" expected="$2"
    shift 2
    local actual
    actual=$("$@" 2>&1)
    assert_eq "$name" "$expected" "$actual"
}

# `require_native_toolchain` — bail out early if the native binaries
# aren't present. Saves time and produces a useful error.
require_native_toolchain() {
    for bin in "$OMG_NATIVE" "$OMGC_NATIVE" "$OMGCC_NATIVE" "$OMGJS_NATIVE"; do
        if [ ! -x "$bin" ]; then
            echo -e "${RED}Native toolchain missing.${NC} Run:"
            echo "  bootstrap/build.sh"
            exit 2
        fi
    done
    if [ ! -x "$OMG_RUST" ]; then
        echo -e "${RED}Rust runtime missing.${NC} Run:"
        echo "  cd runtime && cargo build --release"
        exit 2
    fi
}
