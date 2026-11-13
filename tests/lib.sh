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

# Repo root, derived from this file's location. Sourcing tests call
# the toolchain as $OMG_NATIVE, which can be set from the environment
# to test one that lives somewhere else.
TESTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$TESTS_DIR/.." && pwd)"
OMG_NATIVE="${OMG_NATIVE:-$REPO_ROOT/bin/omg}"

# Non-empty on a Windows host (Git Bash), where a few tests have to
# allow for the way Windows spells the path of a running program.
case "$(uname -s 2>/dev/null)" in
    MINGW*|MSYS*) OMG_WINDOWS=1 ;;
    *) OMG_WINDOWS="${OMG_WINDOWS:-}" ;;
esac

# Per-suite tempdir. Cleaned up by the runner; tests just write into it.
TMPDIR_TEST="${TMPDIR_TEST:-/tmp/omg-tests-$$}"
mkdir -p "$TMPDIR_TEST"

# Git Bash rewrites a POSIX-looking argument ("/tmp/x.omg") into a
# Windows path before a native program sees it, so a program that
# prints its own arguments would not print what the test typed. Use
# the "C:/dir/file" spelling throughout: it is valid in the shell and
# in Windows, and is passed through unchanged.
if [ -n "$OMG_WINDOWS" ] && command -v cygpath >/dev/null 2>&1; then
    REPO_ROOT=$(cygpath -m "$REPO_ROOT")
    TMPDIR_TEST=$(cygpath -m "$TMPDIR_TEST")
    case "$OMG_NATIVE" in /*) OMG_NATIVE=$(cygpath -m "$OMG_NATIVE") ;; esac
fi
# The files are omg.exe on Windows. (bin/omg exists there
# too: it is the Linux binary, which is checked in beside omg.exe.)
if [ -n "$OMG_WINDOWS" ]; then
    [ -f "$OMG_NATIVE.exe" ] && OMG_NATIVE="$OMG_NATIVE.exe"
fi

# What `omg --build` writes when no --target is given: linux-x64 or
# windows-x64 where omg writes machine code itself, c where it goes
# through a C compiler (macOS, and any omg that was itself built
# through C). Its help text says which.
omg_default_target() {
    "$OMG_NATIVE" --help 2>/dev/null | sed -n 's/.*(default here: \(.*\)).*/\1/p' | tr -d '\r'
}

# The C compiler `omg --build --target c` would find, or nothing.
omg_c_compiler() {
    local c
    if [ -n "$OMG_WINDOWS" ]; then
        for c in gcc clang cc; do
            command -v "$c" >/dev/null 2>&1 && { echo "$c"; return; }
        done
    else
        command -v cc >/dev/null 2>&1 && echo cc
    fi
}

# The first four bytes of a file, in hex: 7f454c46 for ELF, 4d5a.... for
# a Windows executable.
file_magic() {
    od -An -tx1 -N4 "$1" 2>/dev/null | tr -d ' \n'
}

# `is_executable <path>`: true if <path>, or <path>.exe, can be run.
is_executable() {
    [ -x "$1" ] || [ -x "$1.exe" ]
}

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
    # Under GitHub Actions, also raise an annotation so the failing test
    # (and the first lines of its detail) shows up on the run summary
    # without digging through the log.
    if [ -n "${GITHUB_ACTIONS:-}" ]; then
        local detail
        detail=$(printf '%s' "${2:-}" | head -n 12 | tr '\n' '|')
        echo "::error title=test failed: $1::$detail"
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

# `assert_program_path <name> <expected> <actual>`: the path a built
# program reports for itself (args[0]). Elsewhere that is exactly the
# path it was started with. Windows hands a program its path in its own
# spelling (backslashes, ".exe", sometimes a different prefix), so
# there only the file name is compared.
assert_program_path() {
    local name="$1" expected="$2" actual="$3"
    if [ -z "$OMG_WINDOWS" ]; then
        assert_eq "$name" "$expected" "$actual"
        return
    fi
    local base
    base=$(basename "$expected")
    case "$actual" in
        *"$base"|*"$base.exe"|*"$base.EXE") pass "$name" ;;
        *) fail "$name" "expected a path ending in: $base (or $base.exe)
      got:      $actual" ;;
    esac
}

# `require_native_toolchain` — bail out early if the native `omg`
# isn't present. Saves time and produces a useful error.
require_native_toolchain() {
    if [ ! -x "$OMG_NATIVE" ]; then
        echo -e "${RED}Native toolchain missing.${NC} Run:"
        echo "  scripts/build.sh"
        exit 2
    fi
}
