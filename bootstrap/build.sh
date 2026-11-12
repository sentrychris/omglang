#!/bin/bash
# Build the OMG native toolchain into bootstrap/bin/. The toolchain is
# one program, `omg`, plus the two runtime files it pastes into the
# code it generates (omg_rt.h for C, omg_rt.js for JavaScript). After
# this completes, the Rust runtime is no longer required to compile or
# run OMG programs.
#
# Works on Linux, macOS and Windows:
#
#   Linux, macOS  any C compiler installed as `cc`.
#   Windows       run this from Git Bash with GCC or Clang from
#                 MinGW-w64 on PATH (MSYS2, WinLibs, w64devkit,
#                 llvm-mingw), or use bootstrap/build.ps1, which does
#                 the same from PowerShell. The result is omg.exe.
#
# Two binaries are checked into bootstrap/bin/: `omg` (Linux x86-64)
# and `omg.exe` (Windows x86-64). On Linux x86-64 this script rebuilds
# the first, and the second too when the MinGW-w64 cross compiler
# (x86_64-w64-mingw32-gcc) is installed: the generated C is the same
# for every system, so both come from one omg.c. On any other host the
# script builds a binary for that host — don't commit that back.
#
# The whole build is two commands:
#
#     omg --emit-c bootstrap/src/omg.omg omg.c     # OMG -> C
#     cc -O3 omg.c -o bootstrap/bin/omg -lm        # C -> executable
#
# (on Windows: gcc -O3 omg.c -o omg.exe -static -lm -lws2_32)
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

# Which system is this? Git Bash and MSYS2 mean "build a
# Windows program": the file is omg.exe, the compiler is called gcc or
# clang (there is no `cc` in most MinGW installs), and the link is
# static so the result does not depend on the compiler's DLLs.
case "$(uname -s)" in
    MINGW*|MSYS*) WINDOWS=1 ;;
    *) WINDOWS= ;;
esac

if [ -n "$WINDOWS" ]; then
    EXE=.exe
    CC_CANDIDATES="gcc clang cc"
    LINK_FLAGS="-static -lm -lws2_32"
else
    EXE=
    CC_CANDIDATES="cc"
    LINK_FLAGS="-lm"
fi
OMG_BIN="$BIN_DIR/omg$EXE"
RUST_OMG="runtime/target/release/omg$EXE"

# A binary checked into bootstrap/bin/ for another system (the Linux
# one on macOS, say) exists and is executable but can't actually run,
# so "is the file there" isn't enough — ask it for its version and see
# whether the system will have it.
runs_here() {
    [ -x "$1" ] && "$1" --version >/dev/null 2>&1
}

CC=
for c in $CC_CANDIDATES; do
    if command -v "$c" >/dev/null 2>&1; then
        CC=$c
        break
    fi
done
if [ -z "$CC" ]; then
    echo "No C compiler found (looked for: $CC_CANDIDATES)."
    echo "  Linux:   install gcc or clang (e.g. apt install build-essential)"
    echo "  macOS:   xcode-select --install"
    echo "  Windows: install GCC or Clang from MinGW-w64 and put its bin"
    echo "           folder on PATH (for example: winget install"
    echo "           BrechtSanders.WinLibs.POSIX.UCRT, or MSYS2's"
    echo "           mingw-w64-ucrt-x86_64-gcc)."
    exit 1
fi

# A bootstrap/bin/omg from before --emit-c existed can run here and
# still be unable to build the toolchain, so ask it.
can_emit_c() {
    runs_here "$1" && "$1" --help 2>/dev/null | grep -q -- '--emit-c'
}

if can_emit_c "$OMG_BIN"; then
    echo "[1/3] Self-rebuild: $OMG_BIN compiles its own source"
    OMG=("$OMG_BIN")
elif [ -x "$RUST_OMG" ]; then
    echo "[1/3] Bootstrapping from the Rust runtime"
    echo "      (no $OMG_BIN that can do it on this host: $(uname -sm))"
    # The Rust runtime runs the driver from source. `--rust` picks its
    # Rust frontend to load omg.omg, which takes milliseconds; the
    # compile of the toolchain itself is still done by compiler.omg.
    OMG=("$RUST_OMG" --rust "$SRC_DIR/omg.omg")
else
    echo "Need either a $OMG_BIN that runs on this host ($(uname -sm)),"
    echo "or the Rust runtime at $RUST_OMG."
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

echo "[3/3] Building omg$EXE with $CC"
"${OMG[@]}" --emit-c "$SRC_DIR/omg.omg" "$WORK/omg.c"
# -O3 unlocks aggressive inlining of the OMG runtime helpers (omg_eq,
# omg_index, omg_dict_find, etc.) which compound in the VM's dispatch
# chain. The generated file is a single translation unit with omg_rt.h
# pasted in, so -flto would add build time and nothing else.
# Build to a temp name and move into place: when self-rebuilding, the
# target is the very program that just ran, and some systems refuse to
# overwrite a busy executable.
# shellcheck disable=SC2086
"$CC" -O3 -w "$WORK/omg.c" -o "$WORK/omg-new$EXE" $LINK_FLAGS
mv -f "$WORK/omg-new$EXE" "$OMG_BIN"

# Earlier versions of the toolchain were four binaries. `omg` does all
# of it now; clear out leftovers from an older build.
rm -f "$BIN_DIR/omgc" "$BIN_DIR/omgcc" "$BIN_DIR/omgjs"

# Record which sources each binary was built from, so that
# check-bin.sh (and CI) can tell when a checked-in binary has fallen
# behind bootstrap/src/. One stamp per binary: SOURCES.sha256 for
# `omg`, SOURCES-windows.sha256 for `omg.exe`.
if [ -n "$WINDOWS" ]; then
    bootstrap/sources-hash.sh > "$BIN_DIR/SOURCES-windows.sha256"
else
    bootstrap/sources-hash.sh > "$BIN_DIR/SOURCES.sha256"
fi

# The checked-in omg.exe. The C that `omg` writes does not depend on
# the system it was written on, so the Windows binary is the same
# omg.c put through the MinGW-w64 cross compiler. Only on Linux
# x86-64, where the checked-in pair is built.
MINGW_CC=x86_64-w64-mingw32-gcc
if [ "$(uname -sm)" = "Linux x86_64" ]; then
    if command -v "$MINGW_CC" >/dev/null 2>&1; then
        echo "      and omg.exe with $MINGW_CC"
        "$MINGW_CC" -O3 -w "$WORK/omg.c" -o "$WORK/omg-new.exe" -static -lm -lws2_32
        mv -f "$WORK/omg-new.exe" "$BIN_DIR/omg.exe"
        bootstrap/sources-hash.sh > "$BIN_DIR/SOURCES-windows.sha256"
    else
        echo "      (omg.exe not rebuilt: $MINGW_CC is not installed;"
        echo "       apt install gcc-mingw-w64-x86-64 to rebuild it too)"
    fi
fi

echo
echo "Native toolchain in $BIN_DIR:"
ls -la "$BIN_DIR/"
echo
if [ -n "$WINDOWS" ]; then
    echo "Run a program with:    bootstrap\\bin\\omg.exe foo.omg"
    echo "Build an executable:   bootstrap\\bin\\omg.exe --build foo.omg"
else
    echo "Run a program with:    $BIN_DIR/omg foo.omg"
    echo "Build an executable:   $BIN_DIR/omg --build foo.omg"
fi
