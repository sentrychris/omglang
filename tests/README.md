# OMG test suite

End-to-end tests for the native toolchain, the unified `omg` driver,
the REPL, and the cross-implementation parity invariants.

## Running

```sh
# Build prerequisites first
cd runtime && cargo build --release && cd ..
bootstrap/build-native-toolchain.sh

# Run everything
tests/run.sh

# Or just one suite
tests/run.sh builtins
tests/run.sh repl regression
```

Exit code is `0` on full success, `1` if anything failed, `2` if the
prerequisites aren't built.

## Layout

| File                   | What it tests                                        |
| ---------------------- | ---------------------------------------------------- |
| `tests/run.sh`         | Top-level runner; sources each suite in turn         |
| `tests/lib.sh`         | Shared assertions (`assert_eq`, `assert_stdout`, …)  |
| `tests/builtins.sh`    | New process & I/O builtins: `subprocess`, `exit`, `getpid`, `stdin_readline`, `print` |
| `tests/driver.sh`      | The five `omg` modes (run `.omg` / run `.omgb` / `--compile` / `--build` / `--help`) plus the standalone tools (`omgc`, `omgvm`, `omgcc`) |
| `tests/repl.sh`        | REPL behaviour: state persistence, multi-line input, error recovery, exit/quit/EOF, closures |
| `tests/parity.sh`      | Triple-meta fixed-point, byte-identical bytecode (Rust frontend vs `omgc`), AOT/native-interpreted corpus parity, self-rebuild |
| `tests/regression.sh`  | Specific bugs we've fixed: control-flow inside `try`, float-bits decoding in `compile_source`, `args[0]` semantics, doubled `RuntimeError:` prefix, `;;;omg` header is optional, stdout/stderr ordering |

## Adding a test

Each `*.sh` file is a standalone bash script that sources `lib.sh`. The
helpers you'll use most:

```sh
section "Group label"                     # pretty section header

# Compare a string to expected.
assert_eq "name" "expected" "actual"

# Run a command and compare its stdout.
assert_stdout "name" "expected" path/to/binary args...

# Same, but capture stderr too.
assert_combined "name" "expected" path/to/binary args...

# Run a command and check its exit code.
assert_exit_code "name" 0 path/to/binary args...

# Substring match (handy for help text, error messages).
assert_contains "name" "needle" "$haystack"
```

A typical pattern:

```sh
section "My new feature"

cat > "$TMPDIR_TEST/myfeature.omg" <<'EOF'
;;;omg
emit "hello"
EOF

assert_stdout "feature works via Rust"   "hello" "$OMG_RUST"    "$TMPDIR_TEST/myfeature.omg"
assert_stdout "feature works via native" "hello" "$OMG_NATIVE"  "$TMPDIR_TEST/myfeature.omg"
```

`$TMPDIR_TEST` is created and cleaned up by the runner — drop your
fixtures there.

## Convention

- **Use real builtins / real binaries.** Tests should exercise the
  shipped tools, not mocks.
- **Test through every applicable path** for behavioural tests
  (Rust runtime, native interpreted, native AOT) — divergences between
  paths are exactly the bugs we want to catch early.
- **Keep tests deterministic.** No timing-dependent checks; no
  network; no system state outside `$TMPDIR_TEST`.
- **Name tests for what they assert**, not what they do
  (`"persistent state survives turns"` not `"defines proc on turn 1
  then calls it on turn 2"`).

## Adding a regression test

When you fix a bug, drop a new test into `regression.sh`. Each test
should have a one-paragraph comment explaining the bug — those
comments are the institutional memory of why the test exists.
