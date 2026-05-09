#!/bin/bash
# Tests for the REPL embedded in the unified `omg` driver. Each test
# pipes a sequence of input lines and checks the output. The REPL is
# stateful, so the test fixture matters — a `proc` defined on turn 1
# must still be callable on turn 5.
#
# Pattern: extract the REPL's own output (after `>>> `, ignoring banner
# and prompt lines) and compare to a known-good string.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

# Strip the REPL banner and prompts (`>>> ` and `... `) so we can
# compare just the user-visible output. The banner is always two lines.
strip_repl() {
    sed -e '1,2d' -e 's/^\(>>> \| *\(>>> \|\.\.\. \)*\)//g' | sed '/^$/d'
}

# Run a session and capture (banner-stripped) stdout + stderr combined.
run_repl() {
    echo "$1" | "$OMG_NATIVE" 2>&1 | strip_repl
}

section "REPL: basic eval"

# Single-turn arithmetic.
assert_eq "single expression evaluation" "42" \
    "$(run_repl 'emit 6 * 7
quit')"

# Two emits in one turn produce two output lines.
assert_eq "multi-statement turn" "1
2" \
    "$(run_repl 'emit 1
emit 2
quit')"

section "REPL: persistent state"

# A binding from turn 1 is visible on turn 2.
assert_eq "alloc persists across turns" "12" \
    "$(run_repl 'alloc x := 12
emit x
quit')"

# Reassignment via := updates the persistent binding.
assert_eq ":= reassigns persistent binding" "5
8" \
    "$(run_repl 'alloc n := 5
emit n
n := 8
emit n
quit')"

# A proc defined on turn 1 is callable later.
assert_eq "proc survives across turns" "100" \
    "$(run_repl 'proc square(x) { return x * x }
emit square(10)
quit')"

# Closure: returned proc still has access to its captured env.
assert_eq "closures across turns" "15" \
    "$(run_repl 'proc make_adder(n) { proc inner(x) { return x + n } return inner }
alloc add5 := make_adder(5)
emit add5(10)
quit')"

# Mutating a list defined in a previous turn.
assert_eq "mutable list across turns" "[1, 2, 3, 4]" \
    "$(run_repl 'alloc xs := [1, 2, 3]
xs := xs + [4]
emit xs
quit')"

section "REPL: multi-line input"

# A proc body spanning several lines: the REPL should buffer until
# braces balance, then compile the whole block at once.
assert_eq "multi-line proc body" "55" \
    "$(run_repl 'proc fib(n) {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}
emit fib(10)
quit')"

# Multi-line list literal.
assert_eq "multi-line list literal" "[10, 20, 30]" \
    "$(run_repl 'alloc xs := [
    10,
    20,
    30
]
emit xs
quit')"

# Multi-line dict.
assert_eq "multi-line dict literal" "Ada" \
    "$(run_repl 'alloc d := {
    name: "Ada",
    age: 36
}
emit d.name
quit')"

section "REPL: error recovery"

# An undefined name triggers an error but the REPL keeps going.
assert_eq "undefined name doesn't kill REPL" "UndefinedIdentError: nope
99" \
    "$(run_repl 'alloc keep := 99
emit nope
emit keep
quit')"

# Division by zero is catchable in subsequent turns.
assert_eq "div0 reported, REPL continues" "ZeroDivisionError: integer division or modulo by zero
hello" \
    "$(run_repl 'alloc x := 1 / 0
emit "hello"
quit')"

# Compile errors don't kill the REPL either.
assert_contains "syntax error keeps REPL alive" "ok" \
    "$(run_repl 'this is not valid OMG
emit "ok"
quit')"

section "REPL: termination"

# `exit` ends the session cleanly.
assert_exit_code "exit cleanly" 0 \
    bash -c "echo 'exit' | '$OMG_NATIVE'"

# `quit` does the same.
assert_exit_code "quit cleanly" 0 \
    bash -c "echo 'quit' | '$OMG_NATIVE'"

# EOF (closed stdin without quit) also exits 0.
assert_exit_code "EOF cleanly" 0 \
    bash -c "echo 'emit 1' | '$OMG_NATIVE'"

# `exit` only triggers at the start of a turn, not mid-line. (A user
# typing `alloc x := exit` would not exit the REPL.)
assert_eq "alloc x := \"exit\" doesn't exit REPL" "ok" \
    "$(run_repl 'alloc s := "exit"
emit "ok"
quit')"
