#!/bin/bash
# Spin out a Rust-free distribution of the OMG native toolchain into
# dist/omglang-native/, mirroring the layout of the standalone
# omglang-native companion repo (https://github.com/sentrychris/omglang-native).
#
# The output directory is self-contained: src/ holds the OMG sources +
# the C and JS runtime files, bin/ holds the pre-built `omg`, and the
# included build.sh self-rebuilds bin/ from src/ without requiring the
# Rust runtime. examples/, tools/, tests/, and docs/ are mirrored for use
# offline.
#
# Usage:
#   bootstrap/package.sh              # populate dist/omglang-native/ in place
#   bootstrap/package.sh --clean      # wipe dist/omglang-native/ first
#   bootstrap/package.sh --tarball    # also produce dist/omglang-native.tar.gz
#
# Prerequisites: bootstrap/bin/omg must already exist
# (run bootstrap/build.sh first).

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST_ROOT="$REPO_ROOT/dist"
DIST_DIR="$DIST_ROOT/omglang-native"
SRC_IN="$REPO_ROOT/bootstrap/src"
BIN_IN="$REPO_ROOT/bootstrap/bin"

CLEAN=0
TARBALL=0
for arg in "$@"; do
    case "$arg" in
        --clean) CLEAN=1 ;;
        --tarball) TARBALL=1 ;;
        -h|--help)
            sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) echo "unknown flag: $arg (try --help)" >&2; exit 2 ;;
    esac
done

# In-place sed that works with both GNU sed (`-i`) and BSD/macOS sed
# (`-i ''`): write to a temp file, then copy the result back into the
# original so it keeps its mode (moving the temp file over it would
# drop the executable bit). The file to edit is the last argument;
# everything before it is passed to sed unchanged.
sed_inplace() {
    local file="${!#}"
    sed "${@:1:$#-1}" "$file" > "$file.sedtmp" && cat "$file.sedtmp" > "$file" && rm -f "$file.sedtmp"
}

# === Sanity checks ===========================================================

# Everything bootstrap/src/omg.omg imports, plus the two runtime files
# the back ends paste into their output. (omg-web.omg and
# omg-explorer.omg are the browser drivers and stay behind.)
TOOLCHAIN_SRC="omg.omg compiler.omg vm.omg bytecode.omg backend.omg native-c.omg native-js.omg omg_rt.h omg_rt.js"

for f in $TOOLCHAIN_SRC; do
    if [ ! -f "$SRC_IN/$f" ]; then
        echo "ERROR: missing $SRC_IN/$f" >&2
        exit 1
    fi
done

if [ ! -x "$BIN_IN/omg" ]; then
    echo "ERROR: missing or non-executable $BIN_IN/omg" >&2
    echo "       Run bootstrap/build.sh first." >&2
    exit 1
fi

# === Output prep =============================================================

if [ "$CLEAN" = 1 ] && [ -d "$DIST_DIR" ]; then
    echo "[clean] removing $DIST_DIR"
    rm -rf "$DIST_DIR"
fi

mkdir -p "$DIST_DIR"/{src,bin,examples,tools,tests,docs}

# === src/ — OMG sources + C runtime header ==================================
# Mirrors bootstrap/src/

echo "[1/7] src/  — OMG sources + runtime files"
for f in $TOOLCHAIN_SRC; do
    cp "$SRC_IN/$f" "$DIST_DIR/src/$f"
done

# === bin/ — pre-built native toolchain ======================================

echo "[2/7] bin/  — pre-built omg"
# A dist directory populated by an older version of this script still
# holds the three binaries that `omg` has since absorbed.
rm -f "$DIST_DIR/bin/omgc" "$DIST_DIR/bin/omgcc" "$DIST_DIR/bin/omgjs"
cp "$BIN_IN/omg" "$DIST_DIR/bin/omg"
chmod +x "$DIST_DIR/bin/omg"
cp "$BIN_IN/omg_rt.h"  "$DIST_DIR/bin/omg_rt.h"
cp "$BIN_IN/omg_rt.js" "$DIST_DIR/bin/omg_rt.js"

# === examples/ ==============================================================

echo "[3/7] examples/"
cp -r "$REPO_ROOT/examples/." "$DIST_DIR/examples/"

# === tools/ =================================================================

echo "[4/7] tools/"
cp -r "$REPO_ROOT/tools/." "$DIST_DIR/tools/"

# === tests/ — adapt for Rust-free environment ===============================
# In the parent repo, tests/ exercise both the Rust runtime and the
# native toolchain. The packaged distribution has no Rust runtime, so
# we rewrite paths and drop Rust-only assertions.

echo "[5/7] tests/ — rewriting paths, stripping Rust-only checks"
for f in run.sh lib.sh builtins.sh driver.sh repl.sh regression.sh parity.sh README.md; do
    if [ -f "$REPO_ROOT/tests/$f" ]; then
        cp "$REPO_ROOT/tests/$f" "$DIST_DIR/tests/$f"
    fi
done

# run.sh lists every suite of the parent repo. Keep only the ones that
# were copied above, or a plain `tests/run.sh` in the distribution ends
# with "No such suite".
for suite in $(sed -n '/^SUITES=(/,/^)/p' "$DIST_DIR/tests/run.sh" | grep -E '^    [a-z_]+$'); do
    if [ ! -f "$DIST_DIR/tests/$suite.sh" ]; then
        sed_inplace -e "/^    $suite\$/d" "$DIST_DIR/tests/run.sh"
    fi
done

# Path rewrites: bootstrap/{bin,src} → {bin,src} (the dist has no
# bootstrap/ prefix), and bootstrap/build.sh → build.sh.
for f in "$DIST_DIR/tests"/*.sh "$DIST_DIR/tests/README.md"; do
    [ -f "$f" ] || continue
    sed_inplace \
        -e 's|bootstrap/bin/|bin/|g' \
        -e 's|bootstrap/src/|src/|g' \
        -e 's|bootstrap/build\.sh|build.sh|g' \
        -e 's|cd runtime && cargo build --release && cd \.\.|./build.sh|g' \
        -e 's|cd runtime && cargo build --release|./build.sh|g' \
        "$f"
done

# Drop Rust-runtime checks from lib.sh + add native-only helpers
# (assert_native_paths, build_aot) that test files in this dist use.
python3 - "$DIST_DIR/tests/lib.sh" <<'PY'
import sys, re
p = sys.argv[1]
src = open(p).read()
# Remove the OMG_RUST definition line.
src = re.sub(r'^OMG_RUST=.*\n', '', src, flags=re.M)
src = src.replace('either binary via $OMG_NATIVE / $OMG_RUST.', 'the binary via $OMG_NATIVE.')
# Remove the Rust-runtime check block in require_native_toolchain.
src = re.sub(
    r'    if \[ ! -x "\$OMG_RUST" \]; then\n'
    r'        echo[^\n]*\n'
    r'        echo[^\n]*\n'
    r'        exit 2\n'
    r'    fi\n',
    '',
    src,
)
# Append native-only assertion helpers used by builtins.sh in the dist.
helpers = '''
# Build an AOT binary from an .omg source. Returns the binary path so
# callers can run it.
build_aot() {
    local src="$1" out="$2"
    "$OMG_NATIVE" --build "$src" "$out" >/dev/null 2>&1
    echo "$out"
}

# Run an OMG program through both native paths (interpreted via `omg
# foo.omg` and AOT via `omg --build foo.omg`) and assert both produce
# the same expected output. Catches divergences between the two
# native backends.
assert_native_paths() {
    local name="$1" src="$2" expected="$3"
    shift 3
    local nat_out aot_bin aot_out
    nat_out=$("$OMG_NATIVE" "$src" "$@" 2>&1) || true

    aot_bin="$TMPDIR_TEST/$(basename "$src" .omg)-aot"
    "$OMG_NATIVE" --build "$src" "$aot_bin" >/dev/null 2>&1
    aot_out=$("$aot_bin" "$@" 2>&1) || true

    if [ "$nat_out" = "$expected" ] && [ "$aot_out" = "$expected" ]; then
        pass "$name"
    else
        fail "$name" "interpreted: $(printf '%q' "$nat_out")
      AOT:         $(printf '%q' "$aot_out")
      expected:    $(printf '%q' "$expected")"
    fi
}
'''
if 'assert_native_paths' not in src:
    src = src.rstrip() + '\n' + helpers
open(p, 'w').write(src)
PY

# Rewrite builtins.sh / driver.sh to drop OMG_RUST-only assertions
# and replace assert_both_paths (which compares Rust vs native) with
# assert_native_paths (which compares interpreted vs AOT).
python3 - "$DIST_DIR/tests/builtins.sh" <<'PY'
import sys, re
p = sys.argv[1]
src = open(p).read()

# Drop the inline assert_both_paths definition block (the dist's lib.sh
# provides assert_native_paths instead). Match the comment header
# through the closing brace of the function.
src = re.sub(
    r'# Build a tiny \.omg file, AOT-compile it.*?\nbuild_aot\(\) \{.*?\n\}\n\n',
    '',
    src,
    flags=re.S,
)
src = re.sub(
    r'# Run an OMG program two ways.*?\nassert_both_paths\(\) \{.*?\n\}\n\n',
    '',
    src,
    flags=re.S,
)
src = src.replace('assert_both_paths', 'assert_native_paths')

# Strip lines that reference OMG_RUST or rust_out (variables that no
# longer exist in the dist). Also follow backslash continuations.
def drop_rust_named_asserts(src):
    # Drop any assertion (across backslash continuations) whose label
    # mentions "rust" (case-insensitive). Each such test is redundant
    # because we've rewired OMG_RUST → OMG_NATIVE, leaving a sibling
    # "native"-named assertion that runs the same binary.
    lines = src.splitlines(keepends=True)
    n = len(lines)
    drop = [False] * n
    i = 0
    while i < n:
        m = re.match(r'\s*assert_[a-z_]+ "[^"]*[Rr][Uu][Ss][Tt][^"]*"', lines[i])
        if m:
            j = i
            while j < n - 1 and lines[j].rstrip().endswith('\\'):
                j += 1
            for k in range(i, j + 1):
                drop[k] = True
            i = j + 1
        else:
            i += 1
    return ''.join(line for i, line in enumerate(lines) if not drop[i])

# Rewire Rust → native: a Rust-labeled test in the dist now runs the
# native binary. The "via Rust" assertions are redundant after that
# (their native sibling runs the same binary), so drop them.
src = re.sub(r'(["\']?)\$OMG_RUST\1', r'\1$OMG_NATIVE\1', src)
src = drop_rust_named_asserts(src)

# Replace divergence comments (now context-free) with a single line.
src = re.sub(
    r'# Failed exec \(binary not found\)[^\n]*\n(# [^\n]*\n)+',
    '# Missing binary: native runtime forks and the child exits 127.\n',
    src,
)
src = re.sub(r'\n{3,}', '\n\n', src)

open(p, 'w').write(src)
PY

# Same treatment for driver.sh and repl.sh.
python3 - "$DIST_DIR/tests/driver.sh" <<'PY'
import sys, re
p = sys.argv[1]
src = open(p).read()

def drop_rust_named_asserts(src):
    # Drop any assertion (across backslash continuations) whose label
    # mentions "rust" (case-insensitive). Each such test is redundant
    # because we've rewired OMG_RUST → OMG_NATIVE, leaving a sibling
    # "native"-named assertion that runs the same binary.
    lines = src.splitlines(keepends=True)
    n = len(lines)
    drop = [False] * n
    i = 0
    while i < n:
        m = re.match(r'\s*assert_[a-z_]+ "[^"]*[Rr][Uu][Ss][Tt][^"]*"', lines[i])
        if m:
            j = i
            while j < n - 1 and lines[j].rstrip().endswith('\\'):
                j += 1
            for k in range(i, j + 1):
                drop[k] = True
            i = j + 1
        else:
            i += 1
    return ''.join(line for i, line in enumerate(lines) if not drop[i])

src = re.sub(r'(["\']?)\$OMG_RUST\1', r'\1$OMG_NATIVE\1', src)
src = drop_rust_named_asserts(src)
src = src.replace('assert_both_paths', 'assert_native_paths')
src = re.sub(r'\n{3,}', '\n\n', src)
open(p, 'w').write(src)
PY

python3 - "$DIST_DIR/tests/repl.sh" <<'PY'
import sys, re
p = sys.argv[1]
src = open(p).read()

def drop_rust_named_asserts(src):
    # Drop any assertion (across backslash continuations) whose label
    # mentions "rust" (case-insensitive). Each such test is redundant
    # because we've rewired OMG_RUST → OMG_NATIVE, leaving a sibling
    # "native"-named assertion that runs the same binary.
    lines = src.splitlines(keepends=True)
    n = len(lines)
    drop = [False] * n
    i = 0
    while i < n:
        m = re.match(r'\s*assert_[a-z_]+ "[^"]*[Rr][Uu][Ss][Tt][^"]*"', lines[i])
        if m:
            j = i
            while j < n - 1 and lines[j].rstrip().endswith('\\'):
                j += 1
            for k in range(i, j + 1):
                drop[k] = True
            i = j + 1
        else:
            i += 1
    return ''.join(line for i, line in enumerate(lines) if not drop[i])

src = re.sub(r'(["\']?)\$OMG_RUST\1', r'\1$OMG_NATIVE\1', src)
src = drop_rust_named_asserts(src)
src = re.sub(r'\n{3,}', '\n\n', src)
open(p, 'w').write(src)
PY

# Drop sections of parity.sh + regression.sh that compare against Rust.
# Parity is now between native interpreted and native AOT only.
python3 - "$DIST_DIR/tests/parity.sh" <<'PY'
import sys, re
p = sys.argv[1]
src = open(p).read()
# Drop the triple-meta fixed-point section (needs $OMG_RUST --verify-omg-vm).
src = re.sub(
    r'# === Triple-meta fixed-point check.*?(?=# === )',
    '',
    src,
    flags=re.S,
)
# Drop the native-compiler-vs-Rust-frontend section.
src = re.sub(
    r'# === native compiler vs Rust frontend.*?(?=# === )',
    '',
    src,
    flags=re.S,
)
# In the AOT corpus parity section, the "Rust runtime" reference becomes
# "native interpreter".
src = src.replace(
    '# === AOT corpus parity ================================================\n'
    'section "Parity: AOT (omg --build) vs Rust runtime"',
    '# === AOT vs interpreted parity ========================================\n'
    'section "Parity: AOT (omg --build) vs native interpreted"',
)
src = src.replace(
    'rust_out=$("$OMG_RUST" "$src" 2>&1)\n    aot_out=$("$bin" 2>&1)\n    if [ "$rust_out" = "$aot_out" ]; then\n        pass "AOT == Rust: $name"',
    'nat_out=$("$OMG_NATIVE" "$src" 2>&1)\n    aot_out=$("$bin" 2>&1)\n    if [ "$nat_out" = "$aot_out" ]; then\n        pass "AOT == native: $name"',
)
src = src.replace('fail "AOT == Rust:', 'fail "AOT == native:')
# Same for the JavaScript section: compare node's output with the
# native interpreter's.
src = src.replace(
    'section "Parity: native-js (node) vs Rust runtime"',
    'section "Parity: native-js (node) vs native interpreted"',
)
src = src.replace(
    'rust_out=$("$OMG_RUST" "$src" 2>&1)\n        js_out=$(node "$jsfile" 2>&1)\n        if [ "$rust_out" = "$js_out" ]; then\n            pass "JS == Rust: $name"',
    'nat_out=$("$OMG_NATIVE" "$src" 2>&1)\n        js_out=$(node "$jsfile" 2>&1)\n        if [ "$nat_out" = "$js_out" ]; then\n            pass "JS == native: $name"',
)
src = src.replace('fail "JS == Rust:', 'fail "JS == native:')
# Drop the native-vs-Rust interpreted section entirely (no Rust here).
src = re.sub(
    r'# === Native interpreted parity.*?(?=# === )',
    '',
    src,
    flags=re.S,
)
if 'OMG_RUST' in src:
    sys.exit('package.sh: tests/parity.sh still refers to $OMG_RUST after rewriting')
open(p, 'w').write(src)
PY

python3 - "$DIST_DIR/tests/regression.sh" <<'PY'
import sys, re
p = sys.argv[1]
src = open(p).read()

def drop_rust_named_asserts(src):
    lines = src.splitlines(keepends=True)
    n = len(lines)
    drop = [False] * n
    i = 0
    while i < n:
        m = re.match(r'\s*assert_[a-z_]+ "[^"]*Rust[^"]*"', lines[i])
        if m:
            j = i
            while j < n - 1 and lines[j].rstrip().endswith('\\'):
                j += 1
            for k in range(i, j + 1):
                drop[k] = True
            i = j + 1
        else:
            i += 1
    return ''.join(line for i, line in enumerate(lines) if not drop[i])

# OMG_RUST refs collapse to OMG_NATIVE. That turns the
# compiler-fixed-point check into "compile compiler.omg twice with the
# native omg, expect identical bytes" — still meaningful without a Rust
# comparison.
src = re.sub(r'(["\']?)\$OMG_RUST\1', r'\1$OMG_NATIVE\1', src)
src = drop_rust_named_asserts(src)
# Drop any orphan top-level `actual=$(...)` line followed only by an
# empty line — those used to feed a now-stripped Rust-named assertion
# and the captured value is no longer referenced.
src = re.sub(
    r'^actual=\$\([^\n]*\)\n(?=\s*\n)',
    '',
    src,
    flags=re.M,
)
src = re.sub(r'\n{3,}', '\n\n', src)
open(p, 'w').write(src)
PY

# === docs/ ==================================================================

echo "[6/7] docs/  — native edition only"
for f in 01-quickstart.md 02-architecture.md 03-language-tour.md 04-pipeline.md 05-extending.md 06-runtime.md 07-debugging.md README.md; do
    if [ -f "$REPO_ROOT/docs/native/$f" ]; then
        cp "$REPO_ROOT/docs/native/$f" "$DIST_DIR/docs/$f"
    fi
done

# Rewrite paths so docs work standalone (no bootstrap/ prefix; no
# runtime/target/release/omg references).
for f in "$DIST_DIR/docs"/*.md; do
    sed_inplace \
        -e 's|bootstrap/bin/|bin/|g' \
        -e 's|bootstrap/src/|src/|g' \
        -e 's|bootstrap/build\.sh|./build.sh|g' \
        -e 's|\.\./\.\./bootstrap/src/|../src/|g' \
        -e 's|\.\./\.\./bootstrap/bin/|../bin/|g' \
        -e 's|cd runtime && cargo build --release && cd \.\.|./build.sh|g' \
        -e 's|cd runtime && cargo build --release|./build.sh|g' \
        -e 's|runtime/target/release/omg|bin/omg|g' \
        -e 's|(\.\./toolchain\.md)|(https://github.com/sentrychris/omglang/blob/main/docs/toolchain.md)|g' \
        "$f"
done

# === build.sh — slim self-rebuild driver ====================================

echo "[7/7] build.sh + README.md"
cat > "$DIST_DIR/build.sh" <<'EOF'
#!/bin/bash
# Rebuild the OMG native toolchain from the OMG sources in src/.
# The toolchain is one program, bin/omg, and it compiles itself:
#
#     bin/omg --emit-c src/omg.omg omg.c     # OMG -> C
#     cc -O3 omg.c -o bin/omg                # C -> executable
#
# src/omg.omg imports the compiler, the VM and both back ends, so that
# one file is the whole toolchain. The repo ships a pre-built bin/omg
# for first use, so this only needs to run after editing anything in
# src/.
set -e

cd "$(dirname "$0")"

SRC_DIR=src
BIN_DIR=bin
mkdir -p "$BIN_DIR"

if ! [ -x "$BIN_DIR/omg" ] || ! "$BIN_DIR/omg" --help 2>/dev/null | grep -q -- '--emit-c'; then
    echo "ERROR: $BIN_DIR/omg is missing or does not run on this machine." >&2
    echo "" >&2
    echo "This repo ships a pre-built Linux x86-64 binary for first use." >&2
    echo "On another platform, or if you removed it, bootstrap once from" >&2
    echo "the parent repo (https://github.com/sentrychris/omglang), whose" >&2
    echo "Rust runtime can build bin/omg from these same sources." >&2
    exit 1
fi

if ! command -v cc >/dev/null 2>&1; then
    echo "ERROR: no C compiler found (need 'cc' on PATH)." >&2
    exit 1
fi

echo "[1/3] Self-rebuild: $BIN_DIR/omg compiles its own source"

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Install the runtime files FIRST: omg pastes bin/omg_rt.h into the C
# it generates, so an edit to src/omg_rt.h has to be in place before
# the new binary is generated.
echo "[2/3] Installing runtime files"
cp "$SRC_DIR/omg_rt.h"  "$BIN_DIR/omg_rt.h"
cp "$SRC_DIR/omg_rt.js" "$BIN_DIR/omg_rt.js"

echo "[3/3] Building omg"
"$BIN_DIR/omg" --emit-c "$SRC_DIR/omg.omg" "$WORK/omg.c"
cc -O3 -w "$WORK/omg.c" -o "$WORK/omg.bin" -lm
mv -f "$WORK/omg.bin" "$BIN_DIR/omg"

echo
echo "Native toolchain in $BIN_DIR:"
ls -la "$BIN_DIR/"
echo
echo "Run a program with:    $BIN_DIR/omg foo.omg"
echo "Build an executable:   $BIN_DIR/omg --build foo.omg"
EOF
chmod +x "$DIST_DIR/build.sh"

# === README.md ==============================================================

cat > "$DIST_DIR/README.md" <<'EOF'
# OMG (native edition)

A self-hosted compiler and runtime for the OMG language, written in
OMG itself with a small C runtime.

This directory is a Rust-free distribution of the
[`omglang`](https://github.com/sentrychris/omglang) project's native
pipeline. The compiler, the bytecode VM, the C and JavaScript back
ends, and the `omg` command that ties them together are **all OMG
source** (in [`src/`](src/)). The C runtime header (`src/omg_rt.h`)
provides the value representation, refcounting, and OS-facing builtins
that any native binary links against; `src/omg_rt.js` does the same
job for JavaScript output. That's the entire non-OMG surface.

It was produced by `bootstrap/package.sh` in the parent repo. To
regenerate it, run that script from a clean checkout of
[`omglang`](https://github.com/sentrychris/omglang).

## Get going

```sh
# Compile and run an OMG program
bin/omg examples/hello_world.omg

# Build a standalone executable (no Rust, no VM)
bin/omg --build examples/hello_world.omg hello
./hello

# Or stop at the generated source
bin/omg --emit-c  examples/hello_world.omg hello.c
bin/omg --emit-js examples/hello_world.omg hello.js && node hello.js

# Interactive REPL
bin/omg
```

Drop `bin/` on your `$PATH` if you'd like to call it as just `omg`.

## What's in `bin/`

One program and the two runtime files it needs:

| File        | Role                                                                 |
| ----------- | -------------------------------------------------------------------- |
| `omg`       | the whole toolchain: run, REPL, `--compile`, `--build`, `--emit-c`, `--emit-js`, `--disasm` |
| `omg_rt.h`  | C runtime, pasted at the top of every `.c` that `omg` generates      |
| `omg_rt.js` | JS runtime, pasted at the top of every `.js` that `omg` generates    |

`omg` is compiled from the OMG sources in [`src/`](src/), by itself.
It runs `.omg` and `.omgb` files directly: the compiler and the
bytecode VM are part of it.

## Rebuild the toolchain

If you change anything in `src/`, rebuild:

```sh
./build.sh
```

It self-rebuilds: the existing `bin/omg` compiles a fresh one from the
updated sources (`omg --emit-c src/omg.omg`, then `cc`).

## Run the test suite

```sh
tests/run.sh                   # everything
tests/run.sh builtins repl     # filter to specific suites
```

Tests cover the native-only paths (interpreted vs AOT). For the full
Rust-vs-native parity check, use the parent
[`omglang`](https://github.com/sentrychris/omglang) repo.

## Layout

```
omglang-native/
├── README.md         this file
├── build.sh          rebuild bin/ from src/ (self-bootstrapping)
├── src/              all OMG source + the two runtime files
│   ├── omg.omg          the `omg` command; imports everything below
│   ├── compiler.omg     source -> bytecode
│   ├── vm.omg           runs bytecode
│   ├── bytecode.omg     reads a .omgb file back in
│   ├── native-c.omg     bytecode -> C
│   ├── native-js.omg    bytecode -> JavaScript
│   ├── backend.omg      what the two back ends share
│   ├── omg_rt.h         C runtime
│   └── omg_rt.js        JS runtime
├── bin/              pre-built omg + omg_rt.h + omg_rt.js
├── examples/         small standalone OMG programs
├── tools/            command-line utilities written in OMG
├── tests/            end-to-end test suite (native-only)
└── docs/             detailed docs
```

## License

MIT. See the parent
[`omglang`](https://github.com/sentrychris/omglang) repository for the
full project.
EOF

# === Tarball (optional) =====================================================

if [ "$TARBALL" = 1 ]; then
    echo
    echo "[+] producing $DIST_ROOT/omglang-native.tar.gz"
    (cd "$DIST_ROOT" && tar -czf omglang-native.tar.gz omglang-native)
fi

# === Summary ================================================================

echo
echo "Distribution ready at: $DIST_DIR"
echo
echo "Layout:"
(cd "$DIST_DIR" && find . -maxdepth 2 -mindepth 1 | sort | sed 's|^\./|  |')
echo
echo "Try it:"
echo "  $DIST_DIR/bin/omg $DIST_DIR/examples/hello_world.omg"
