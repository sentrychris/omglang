#!/bin/bash
# Build the OMG native toolchain into bootstrap/native/. Produces five
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
#   bootstrap/compiler.omg  → omgc   compiler   (.omg → .omgb)        standalone
#   bootstrap/native-c.omg  → omgcc  transpiler (.omgb → .c)          standalone
#   bootstrap/vm.omg        → omgvm  bytecode VM (executes .omgb)     standalone
#   bootstrap/omg.omg       → omg    unified driver (run/compile/    primary
#                                    build/REPL all in-process)      user-facing
#
# `omg` is the "all-in-one" binary, mirroring the Rust runtime: it
# imports compiler.omg, vm.omg, and native-c.omg directly so compile,
# run, and REPL happen in-process. The standalone tools (omgc, omgvm,
# omgcc) are kept around for direct use, but day-to-day usage goes
# through `omg`.
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

# Build a binary end-to-end: source .omg -> .omgb -> .c -> ELF.
build_binary() {
    local src="$1" out="$2" base
    base=$(basename "$src" .omg)
    omg_compile  "$src"               "$WORK/$base.omgb"
    omg_transpile "$WORK/$base.omgb"  "$WORK/$base.c"
    cc -O2 -w "$WORK/$base.c" -o "$out" -lm
}

# Targets — driver ELFs first so we don't break the existing toolchain
# half-way through if a compile error appears.
echo "[2/4] Building toolchain core (omgc, omgcc, omgvm)"
build_binary bootstrap/compiler.omg  "$NATIVE_DIR/omgc"
build_binary bootstrap/native-c.omg  "$NATIVE_DIR/omgcc"
build_binary bootstrap/vm.omg        "$NATIVE_DIR/omgvm"

echo "[3/4] Building unified driver (omg)"
build_binary bootstrap/omg.omg       "$NATIVE_DIR/omg"

# Remove obsolete dispatcher-era binaries if a previous build left them
# behind. The unified `omg` does what they did, all in-process.
rm -f "$NATIVE_DIR/omg-build" "$NATIVE_DIR/omg-repl"

echo "[4/4] Installing runtime header"
cp bootstrap/omg_rt.h "$NATIVE_DIR/omg_rt.h"

echo
echo "Native toolchain in $NATIVE_DIR:"
ls -la "$NATIVE_DIR/"
echo
echo "Run a program with:    bootstrap/native/omg foo.omg"
echo "AOT-build a program:   bootstrap/native/omg --build foo.omg"
