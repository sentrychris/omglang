#!/bin/bash
# Build the OMG native toolchain into bootstrap/bin/. The toolchain is
# one program, `omg`, plus the two runtime files it pastes into the
# code it generates (omg_rt.h for C, omg_rt.js for JavaScript). After
# this completes, the Rust runtime is no longer required to compile or
# run OMG programs.
#
# Works on Linux and macOS (any Unix with a C compiler). The binary
# checked into bootstrap/bin/ is Linux x86-64; on other hosts the first
# run bootstraps from the Rust runtime and replaces it with a native
# one — don't commit that back.
#
# The whole build is two commands:
#
#     omg --emit-c bootstrap/src/omg.omg omg.c     # OMG -> C
#     cc -O3 omg.c -o bootstrap/bin/omg            # C -> executable
#
# and the only question is which `omg` runs the first one:
#   1. Self-rebuild: bootstrap/bin/omg already runs here, so it
#      compiles its own source.
#   2. Bootstrap: there is no usable bootstrap/bin/omg yet, so the Rust
#      runtime at runtime/target/release/omg runs omg.omg from source.
#
# bootstrap/src/omg.omg imports the compiler, the VM and both back ends
# (compiler.omg, vm.omg, native-c.omg, native-js.omg, plus the shared
# bytecode.omg and backend.omg), so that one file is the whole
# toolchain.
set -e

cd "$(dirname "$0")/.."

SRC_DIR=bootstrap/src
BIN_DIR=bootstrap/bin
mkdir -p "$BIN_DIR"

# The binaries checked into bootstrap/bin/ are Linux x86-64 ELFs. On any
# other host (macOS, Linux on ARM, ...) they exist and are executable
# but can't actually run, so "is the file there" isn't enough — ask each
# one for its version and see whether the kernel will have it.
runs_here() {
    [ -x "$1" ] && "$1" --version >/dev/null 2>&1
}

if ! command -v cc >/dev/null 2>&1; then
    echo "No C compiler found (need 'cc' on PATH)."
    echo "  Linux:  install gcc or clang (e.g. apt install build-essential)"
    echo "  macOS:  xcode-select --install"
    echo "  Windows: the native toolchain needs a POSIX environment; use WSL,"
    echo "           or run OMG through the Rust runtime (runtime/), which"
    echo "           builds natively on Windows."
    exit 1
fi

# A bootstrap/bin/omg from before --emit-c existed can run here and
# still be unable to build the toolchain, so ask it.
can_emit_c() {
    runs_here "$1" && "$1" --help 2>/dev/null | grep -q -- '--emit-c'
}

if can_emit_c "$BIN_DIR/omg"; then
    echo "[1/3] Self-rebuild: $BIN_DIR/omg compiles its own source"
    OMG=("$BIN_DIR/omg")
elif [ -x runtime/target/release/omg ]; then
    echo "[1/3] Bootstrapping from the Rust runtime"
    echo "      (no $BIN_DIR/omg that can do it on this host: $(uname -sm))"
    # The Rust runtime runs the driver from source. `--rust` picks its
    # Rust frontend to load omg.omg, which takes milliseconds; the
    # compile of the toolchain itself is still done by compiler.omg.
    OMG=(runtime/target/release/omg --rust "$SRC_DIR/omg.omg")
else
    echo "Need either a $BIN_DIR/omg that runs on this host ($(uname -sm)),"
    echo "or the Rust runtime at runtime/target/release/omg."
    echo "First-time build: cd runtime && cargo build --release"
    exit 1
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Install the runtime files FIRST. The driver reads omg_rt.h (and
# omg_rt.js) from the directory it lives in and pastes it into what it
# generates, so a change to either must be in place before the rebuilt
# binary is generated — otherwise it would be built on a stale runtime.
# (When bootstrapping, the driver runs from bootstrap/src/ and reads
# the originals there.)
echo "[2/3] Installing runtime files"
cp "$SRC_DIR/omg_rt.h"  "$BIN_DIR/omg_rt.h"
cp "$SRC_DIR/omg_rt.js" "$BIN_DIR/omg_rt.js"

echo "[3/3] Building omg"
"${OMG[@]}" --emit-c "$SRC_DIR/omg.omg" "$WORK/omg.c"
# -O3 unlocks aggressive inlining of the OMG runtime helpers (omg_eq,
# omg_index, omg_dict_find, etc.) which compound in the VM's dispatch
# chain. The generated file is a single translation unit with omg_rt.h
# pasted in, so -flto would add build time and nothing else.
# Build to a temp name and move into place: when self-rebuilding, the
# target is the very program that just ran, and some systems refuse to
# overwrite a busy executable.
cc -O3 -w "$WORK/omg.c" -o "$WORK/omg.bin" -lm
mv -f "$WORK/omg.bin" "$BIN_DIR/omg"

# Earlier versions of the toolchain were four binaries. `omg` does all
# of it now; clear out leftovers from an older build.
rm -f "$BIN_DIR/omgc" "$BIN_DIR/omgcc" "$BIN_DIR/omgjs"

# Record which sources this binary was built from, so that
# check-bin.sh (and CI) can tell when the checked-in binary has
# fallen behind bootstrap/src/.
bootstrap/sources-hash.sh > "$BIN_DIR/SOURCES.sha256"

echo
echo "Native toolchain in $BIN_DIR:"
ls -la "$BIN_DIR/"
echo
echo "Run a program with:    $BIN_DIR/omg foo.omg"
echo "Build an executable:   $BIN_DIR/omg --build foo.omg"
