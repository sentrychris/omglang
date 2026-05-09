#!/bin/bash
# Build the OMG native toolchain (omgc + omgcc + omg_rt.h) into
# bootstrap/native/. After this completes, the Rust runtime is no
# longer required to compile or run OMG programs.
#
# Two modes:
#   1. Bootstrap: native toolchain doesn't yet exist — uses the Rust
#      runtime at runtime/target/release/omg.
#   2. Self-rebuild: native toolchain already exists — uses the
#      existing omgc + omgcc to rebuild themselves.
set -e

cd "$(dirname "$0")/.."

NATIVE_DIR=bootstrap/native
mkdir -p "$NATIVE_DIR"

if [ -x "$NATIVE_DIR/omgc" ] && [ -x "$NATIVE_DIR/omgcc" ]; then
    echo "[1/4] Self-rebuild from existing native toolchain"
    DRIVER=native
elif [ -x runtime/target/release/omg ]; then
    echo "[1/4] Bootstrapping from Rust runtime"
    DRIVER=rust
else
    echo "Need either bootstrap/native/{omgc,omgcc} or runtime/target/release/omg."
    echo "First-time build: cd runtime && cargo build --release"
    exit 1
fi

WORK=$(mktemp -d)
trap "rm -rf $WORK" EXIT

# Compile .omg -> .omgb using the chosen driver.
omg_compile() {
    if [ "$DRIVER" = native ]; then
        "$NATIVE_DIR/omgc" "$1" "$2"
    else
        runtime/target/release/omg --compile "$1" "$2"
    fi
}

# Transpile .omgb -> .c using the chosen driver.
omg_transpile() {
    if [ "$DRIVER" = native ]; then
        "$NATIVE_DIR/omgcc" "$1" "$2"
    else
        runtime/target/release/omg bootstrap/native-c.omg "$1" "$2"
    fi
}

echo "[2/4] Compiling compiler.omg + native-c.omg + vm.omg to bytecode"
omg_compile bootstrap/compiler.omg  "$WORK/omgc.omgb"
omg_compile bootstrap/native-c.omg  "$WORK/omgcc.omgb"
omg_compile bootstrap/vm.omg        "$WORK/omgvm.omgb"

echo "[3/4] Transpiling to C"
omg_transpile "$WORK/omgc.omgb"  "$WORK/omgc.c"
omg_transpile "$WORK/omgcc.omgb" "$WORK/omgcc.c"
omg_transpile "$WORK/omgvm.omgb" "$WORK/omgvm.c"

echo "[4/4] cc -O2"
cc -O2 -w "$WORK/omgc.c"  -o "$NATIVE_DIR/omgc"  -lm
cc -O2 -w "$WORK/omgcc.c" -o "$NATIVE_DIR/omgcc" -lm
cc -O2 -w "$WORK/omgvm.c" -o "$NATIVE_DIR/omgvm" -lm
cp bootstrap/omg_rt.h "$NATIVE_DIR/omg_rt.h"

echo
echo "Native toolchain in $NATIVE_DIR:"
ls -la "$NATIVE_DIR/"
echo
echo "Run a program with:    bootstrap/native/omg foo.omg"
echo "AOT-build a program:   bootstrap/native/omg --build foo.omg"
