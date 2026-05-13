#!/bin/bash
# Build the OMG native toolchain into bootstrap/bin/. Produces four
# native ELF binaries plus the C runtime header. After this completes,
# the Rust runtime is no longer required to compile or run OMG programs.
#
# Two modes:
#   1. Bootstrap: native toolchain doesn't yet exist — uses the Rust
#      runtime at runtime/target/release/omg.
#   2. Self-rebuild: native toolchain already exists — uses the
#      existing omgc + omgcc to rebuild themselves.
#
# Sources compiled (each .omg → .omgb → .c → ELF):
#   bootstrap/src/compiler.omg  → omgc   compiler   (.omg → .omgb)     standalone
#   bootstrap/src/native-c.omg  → omgcc  C transpiler  (.omgb → .c)    standalone
#   bootstrap/src/native-js.omg → omgjs  JS transpiler (.omgb → .js)   standalone
#   bootstrap/src/omg.omg       → omg    unified driver (run/compile/  primary
#                                        build/REPL all in-process)    user-facing
#
# `omg` is the "all-in-one" binary, mirroring the Rust runtime: it
# imports compiler.omg, vm.omg, and native-c.omg directly so compile,
# run, and REPL happen in-process. The standalone tools (omgc, omgcc)
# are kept around for build pipelines, but day-to-day usage goes
# through `omg`.
#
# Note: vm.omg is *not* compiled to a standalone ELF — bin/omg imports
# it in-process so `omg foo.omgb` covers the bytecode-runner case, and
# the verify-omg-vm flow uses vm.omgb embedded in rust_omg.
set -e

cd "$(dirname "$0")/.."

SRC_DIR=bootstrap/src
BIN_DIR=bootstrap/bin
mkdir -p "$BIN_DIR"

if [ -x "$BIN_DIR/omgc" ] && [ -x "$BIN_DIR/omgcc" ]; then
    echo "[1/4] Self-rebuild from existing native toolchain"
    DRIVER=native
elif [ -x runtime/target/release/omg ]; then
    echo "[1/4] Bootstrapping from Rust runtime"
    DRIVER=rust
else
    echo "Need either $BIN_DIR/{omgc,omgcc} or runtime/target/release/omg."
    echo "First-time build: cd runtime && cargo build --release"
    exit 1
fi

WORK=$(mktemp -d)
trap "rm -rf $WORK" EXIT

# Compile .omg -> .omgb using the chosen driver.
omg_compile() {
    if [ "$DRIVER" = native ]; then
        "$BIN_DIR/omgc" "$1" "$2"
    else
        runtime/target/release/omg --compile "$1" "$2"
    fi
}

# Transpile .omgb -> .c using the chosen driver.
omg_transpile() {
    if [ "$DRIVER" = native ]; then
        "$BIN_DIR/omgcc" "$1" "$2"
    else
        runtime/target/release/omg "$SRC_DIR/native-c.omg" "$1" "$2"
    fi
}

# Build a binary end-to-end: source .omg -> .omgb -> .c -> ELF.
build_binary() {
    local src="$1" out="$2" base
    base=$(basename "$src" .omg)
    omg_compile  "$src"               "$WORK/$base.omgb"
    omg_transpile "$WORK/$base.omgb"  "$WORK/$base.c"
    cc -O2 -w "$WORK/$base.c" -o "$out" -lm
}

# Install the runtime headers FIRST. omgcc reads omg_rt.h via
# bin_dir/omg_rt.h to splice into transpiled C output, and native-js
# reads bin_dir/omg_rt.js the same way for transpiled JS output, so
# any change to either source must take effect on the *first* rebuild —
# otherwise the rebuilt binaries read stale copies in bin/.
echo "[2/4] Installing runtime headers"
cp "$SRC_DIR/omg_rt.h"  "$BIN_DIR/omg_rt.h"
cp "$SRC_DIR/omg_rt.js" "$BIN_DIR/omg_rt.js"

echo "[3/4] Building toolchain core (omgc, omgcc, omgjs)"
build_binary "$SRC_DIR/compiler.omg"  "$BIN_DIR/omgc"
build_binary "$SRC_DIR/native-c.omg"  "$BIN_DIR/omgcc"
build_binary "$SRC_DIR/native-js.omg" "$BIN_DIR/omgjs"

echo "[4/4] Building unified driver (omg)"
build_binary "$SRC_DIR/omg.omg"      "$BIN_DIR/omg"

echo
echo "Native toolchain in $BIN_DIR:"
ls -la "$BIN_DIR/"
echo
echo "Run a program with:    $BIN_DIR/omg foo.omg"
echo "AOT-build a program:   $BIN_DIR/omg --build foo.omg"
