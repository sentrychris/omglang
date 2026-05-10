#!/bin/bash
# Top-level test runner. Sources each suite in turn, aggregates
# pass/fail counts, prints a summary, and returns non-zero if anything
# failed.
#
# Usage:
#   tests/run.sh                 Run every suite
#   tests/run.sh builtins        Run a single suite (no .sh extension)
#   tests/run.sh -v              Verbose: don't suppress test internal logs
#
# Exit codes:
#   0  all tests passed
#   1  one or more tests failed
#   2  prerequisite missing (native toolchain or Rust runtime)

set -u
cd "$(dirname "$0")/.."

# Suites in the order they should run. Cheaper / more focused first;
# expensive parity checks last so a quick fail surfaces fast.
SUITES=(
    builtins
    driver
    repl
    regression
    db
    parity
)

# Allow filtering: `tests/run.sh builtins repl`.
FILTER=()
VERBOSE=0
while [ $# -gt 0 ]; do
    case "$1" in
        -v|--verbose) VERBOSE=1 ;;
        -h|--help)
            grep '^# ' "$0" | sed 's/^# //'
            exit 0
            ;;
        *) FILTER+=("$1") ;;
    esac
    shift
done

if [ ${#FILTER[@]} -gt 0 ]; then
    SUITES=("${FILTER[@]}")
fi

# Source the helper library before any suite so our colour vars and
# counters are initialised exactly once.
source tests/lib.sh

# Each suite runs in this shell so PASS_COUNT etc. accumulate. Tempdir
# is shared across suites for ease of cross-suite reuse, then cleaned.
trap "rm -rf $TMPDIR_TEST" EXIT

START=$(date +%s)
echo "OMG test suite"
echo "  repo:    $REPO_ROOT"
echo "  tempdir: $TMPDIR_TEST"

for suite in "${SUITES[@]}"; do
    suite_file="tests/$suite.sh"
    if [ ! -f "$suite_file" ]; then
        echo -e "${RED}No such suite: $suite${NC}"
        exit 2
    fi
    if [ "$VERBOSE" = 1 ]; then
        source "$suite_file"
    else
        # Source but suppress the per-test internal stderr (binaries we
        # invoke are noisy by design — e.g. native-c.omg emits a
        # "[native-c] ..." line for every transpile). Pass/fail lines
        # come from `pass`/`fail` which write to stdout.
        source "$suite_file" 2> >(grep -v '^\[native-c\]' >&2)
    fi
done

END=$(date +%s)
echo
echo "================================================"
TOTAL=$TEST_COUNT
if [ "$FAIL_COUNT" = 0 ]; then
    echo -e "${GREEN}All $TOTAL tests passed${NC} (in $((END - START))s)"
    exit 0
else
    echo -e "${RED}$FAIL_COUNT of $TOTAL tests failed${NC} (in $((END - START))s)"
    echo -e "Failures:$FAILED_TESTS"
    exit 1
fi
