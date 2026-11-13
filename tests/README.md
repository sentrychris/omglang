# OMG test suite

End-to-end tests for the toolchain: the `omg` command, the REPL, the
back ends, and the checks that hold the different ways of running a
program to the same answers.

## Running

```sh
# Run everything (bin/omg is checked in; nothing to build first)
tests/run.sh

# Or just one suite
tests/run.sh builtins
tests/run.sh repl regression

# The toolchain on its own: no compiler of any kind needed
tests/standalone.sh
```

Exit code is `0` on full success, `1` if anything failed, `2` if there
is no `omg` that runs here.

After changing anything under `src/`, run `scripts/build.sh` first:
the tests exercise `bin/omg`.

Nothing has to be installed. Two optional tools widen the coverage,
and their checks are skipped without them: `node` (everything about
`--emit-js`) and a C compiler (everything about `--target c`).

The scripts are bash. On Windows run them from Git Bash; they then
test `bin/omg.exe`. `OMG_NATIVE` can be set to test a binary that
lives somewhere else.

## Who is the referee?

There is no second implementation to compare against. Three things
take its place:

- **Stated answers.** Every program in `tests/conformance/` comes with
  the output it must print, and is run interpreted, built to machine
  code, built through C, and as JavaScript.
- **The back ends against each other.** For the examples, the tools
  and many individual checks, what a built program prints must equal
  what the interpreter prints.
- **Fixed points.** The compiler, compiled and interpreted, must emit
  the same bytecode for its own source; and `omg` rebuilding itself
  twice must give the same file.

## Layout

| File                   | What it tests                                        |
| ---------------------- | ---------------------------------------------------- |
| `tests/run.sh`         | Top-level runner; sources each suite in turn         |
| `tests/lib.sh`         | Shared assertions (`assert_eq`, `assert_stdout`, …)  |
| `tests/builtins.sh`    | Process & I/O builtins: `subprocess`, `exit`, `getpid`, `stdin_readline`, `stdin_read`, file seeking, `print`; interpreted and built |
| `tests/driver.sh`      | Every `omg` mode: run `.omg` / run `.omgb` / `--compile` / `--build` and its `--target`s / `--emit-c` / `--emit-js` / `--disasm` / `--help` |
| `tests/repl.sh`        | REPL behaviour: state persistence, multi-line input, error recovery, exit/quit/EOF, closures |
| `tests/syntax.sh`      | Malformed programs: a one-line `SyntaxError`, exit code 1, the same message from run, `--compile` and `--build`, and nothing written |
| `tests/regression.sh`  | Specific bugs we've fixed: control-flow inside `try`, float-bits decoding in `compile_source`, `args[0]` semantics, doubled `RuntimeError:` prefix, `;;;omg` header is optional, line numbers in a file with no header, stdout/stderr ordering |
| `tests/conformance.sh` | The conformance corpus every way: interpreter, machine code, through C, JavaScript; plus stack-limit checks that only make sense for an executable, and the errors only the machine-code runtime raises (a result that does not fit in 64 bits, a list or nothing in arithmetic, a Bool in an ordering) |
| `tests/conformance/`   | The corpus itself: `NAME.omg` must print `NAME.expected`; `fail/NAME.omg` must die with the error kind on its `# expect:` line, and with the frames in `NAME.traceback` if there is one |
| `tests/tools.sh`       | The programs in `tools/`: their own test suite interpreted and built, and the HTTP server answering real requests both ways |
| `tests/db.sh`          | `omgdb`: SQL results and the on-disk format, with files written by the interpreted database read by the built one and back |
| `tests/video.sh`       | The video tools: the H.264 decoder against checksums from ffmpeg, for clips in the Baseline, Main and High profile, and the `window_` builtins and the player on a display (Xvfb on Linux, when installed) |
| `tests/parity.sh`      | The compiler compiled vs interpreted (same bytecode for its own source); every example built vs interpreted, through C, and as JS; the self-rebuild, and a second one that must be byte-identical |
| `tests/standalone.sh`  | Not part of `run.sh`. The toolchain with nothing else installed: self-rebuild, byte-identical second rebuild, the corpus interpreted and built, executables for the other system. CI runs it in a bare container and on a Windows machine with compilers taken off `PATH` |
| `tests/smoke.sh`       | Not part of `run.sh` or CI. The frozen Rust runtime's own self-check, for use after building it by hand (see `rust/README.md`) |

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

assert_stdout "feature works interpreted" "hello" "$OMG_NATIVE" "$TMPDIR_TEST/myfeature.omg"

"$OMG_NATIVE" --build "$TMPDIR_TEST/myfeature.omg" "$TMPDIR_TEST/myfeature" >/dev/null
assert_stdout "feature works built"       "hello" "$TMPDIR_TEST/myfeature"
```

`$TMPDIR_TEST` is created and cleaned up by the runner — drop your
fixtures there.

## Convention

- **Use real builtins / real binaries.** Tests should exercise the
  shipped tools, not mocks.
- **Test through every applicable path** for behavioural tests
  (interpreted and built, at least) — divergences between paths are
  exactly the bugs we want to catch early. If the behaviour belongs to
  the language rather than to one tool, put it in `tests/conformance/`
  instead: it then runs every way automatically.
- **Keep tests deterministic.** No timing-dependent checks; no
  network beyond localhost; no system state outside `$TMPDIR_TEST`.
- **Name tests for what they assert**, not what they do
  (`"persistent state survives turns"` not `"defines proc on turn 1
  then calls it on turn 2"`).

## Adding a regression test

When you fix a bug, drop a new test into `regression.sh`. Each test
should have a one-paragraph comment explaining the bug — those
comments are the institutional memory of why the test exists.
