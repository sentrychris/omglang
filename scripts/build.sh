#!/bin/bash
# Build the OMG native toolchain into bin/. The toolchain is one
# program, `omg`, plus the runtime files it adds to what it generates:
# x64/*.omg for machine code, omg_rt.h for C, omg_rt.js for JavaScript.
#
# On Linux and Windows (x86-64) nothing but OMG takes part: `omg`
# writes executables itself, its own included.
#
#     omg --build src/omg.omg bin/omg
#
# Two binaries are checked into bin/: `omg` (Linux x86-64) and
# `omg.exe` (Windows x86-64). On Linux x86-64 this script rebuilds
# both, since `omg` can write a Windows executable from anywhere. On
# Windows (run this from Git Bash, or use scripts/build.ps1 from
# PowerShell) it rebuilds omg.exe.
#
# On every other system (macOS, Linux on ARM, ...) the build goes
# through C and needs a C compiler installed as `cc`:
#
#     omg --emit-c src/omg.omg omg.c     # OMG -> C
#     cc -O3 omg.c -o bin/omg -lm        # C -> executable
#
# `scripts/build.sh --target c` takes that route on Linux and Windows
# x86-64 too. A binary built through C is for this machine; don't
# commit it back.
#
# The only question is which `omg` does the building:
#   1. Self-rebuild: bin/omg already runs here, so it builds its own
#      source.
#   2. Bootstrap: there is no usable bin/omg, and someone has built
#      the frozen Rust runtime (see rust/README.md), so
#      rust/target/release/omg runs omg.omg from source. This is the
#      route from source alone, and nothing in daily use takes it.
#
# src/omg.omg imports the compiler, the VM and the three back ends
# (compiler.omg, vm.omg, native-x64.omg, native-c.omg, native-js.omg,
# plus the shared bytecode.omg and backend.omg), so that one file is
# the whole toolchain.
set -e

cd "$(dirname "$0")/.."

SRC_DIR=src
BIN_DIR=bin
mkdir -p "$BIN_DIR"

# Which system is this? Git Bash and MSYS2 mean "build a Windows
# program": the file is omg.exe.
case "$(uname -s)" in
    MINGW*|MSYS*) WINDOWS=1 ;;
    *) WINDOWS= ;;
esac

# What to build for. The x86-64 back end covers Linux and Windows;
# everything else goes through C.
TARGET=c
case "$(uname -sm)" in
    "Linux x86_64") TARGET=linux-x64 ;;
    MINGW*x86_64|MSYS*x86_64) TARGET=windows-x64 ;;
esac
if [ "$1" = "--target" ]; then
    case "$2" in
        c) TARGET=c ;;
        "$TARGET") ;;
        *) echo "scripts/build.sh: can build for $TARGET or c here, not '$2'"; exit 1 ;;
    esac
elif [ -n "$1" ]; then
    echo "usage: scripts/build.sh [--target c]"
    exit 1
fi

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
RUST_OMG="rust/target/release/omg$EXE"

# A binary checked into bin/ for another system (the Linux one on
# macOS, say) exists and is executable but can't actually run, so "is
# the file there" isn't enough — ask it for its version and see
# whether the system will have it.
runs_here() {
    [ -x "$1" ] && "$1" --version >/dev/null 2>&1
}

CC=
if [ "$TARGET" = c ]; then
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
fi

# A bin/omg from before the step it is needed for existed can run here
# and still be unable to build the toolchain, so ask it.
can_do() {
    runs_here "$1" && "$1" --help 2>/dev/null | grep -q -- "$2"
}
if [ "$TARGET" = c ]; then
    NEEDS=--emit-c
else
    NEEDS=--target
fi

if can_do "$OMG_BIN" "$NEEDS"; then
    echo "[1/3] Self-rebuild: $OMG_BIN builds its own source"
    OMG=("$OMG_BIN")
elif [ -x "$RUST_OMG" ]; then
    echo "[1/3] Bootstrapping from the Rust runtime"
    echo "      (no $OMG_BIN that can do it on this host: $(uname -sm))"
    # The Rust runtime runs the driver from source. `--rust` picks its
    # Rust frontend to load omg.omg, which takes milliseconds; the
    # compile of the toolchain itself is still done by compiler.omg.
    OMG=("$RUST_OMG" --rust "$SRC_DIR/omg.omg")
else
    echo "There is no $OMG_BIN that runs on this host ($(uname -sm))."
    echo "On a system other than Linux or Windows x86-64, make a first one"
    echo "from generated C: docs/01-quickstart.md, \"Other systems\"."
    echo "(Or build the frozen Rust runtime: rust/README.md.)"
    exit 1
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Install the runtime files FIRST. The driver reads them from the
# directory it lives in and builds them into what it generates, so a
# change to any of them must be in place before the rebuilt binary is
# generated — otherwise it would be built on a stale runtime. (When
# bootstrapping, the driver runs from src/ and reads the originals
# there.)
echo "[2/3] Installing runtime files"
cp "$SRC_DIR/runtime/omg_rt.h"  "$BIN_DIR/omg_rt.h"
cp "$SRC_DIR/runtime/omg_rt.js" "$BIN_DIR/omg_rt.js"
rm -rf "$BIN_DIR/x64"
mkdir -p "$BIN_DIR/x64"
cp "$SRC_DIR"/runtime/x64/*.omg "$BIN_DIR/x64/"

if [ "$TARGET" = c ]; then
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
else
    echo "[3/3] Building omg$EXE"
    # Twice. The first pass is the new source built by whatever `omg`
    # was at hand; the second is the new source built by itself, which
    # is what gets installed. A third pass would write the same bytes:
    # tests/parity.sh checks exactly that.
    mkdir -p "$WORK/stage1/x64"
    cp "$BIN_DIR"/x64/*.omg "$WORK/stage1/x64/"
    "${OMG[@]}" --build "$SRC_DIR/omg.omg" "$WORK/stage1/omg$EXE" --target "$TARGET" >/dev/null
    # Build to a temp name and move into place: the target may be the
    # very program that started this, and some systems refuse to
    # overwrite a busy executable.
    "$WORK/stage1/omg$EXE" --build "$SRC_DIR/omg.omg" "$WORK/omg-new$EXE" --target "$TARGET" >/dev/null
    if [ "$TARGET" = linux-x64 ]; then
        # The checked-in omg.exe: the same source, built for Windows.
        echo "      and omg.exe"
        "$WORK/stage1/omg" --build "$SRC_DIR/omg.omg" "$WORK/omg-new.exe" --target windows-x64 >/dev/null
        mv -f "$WORK/omg-new.exe" "$BIN_DIR/omg.exe"
        scripts/sources-hash.sh > "$BIN_DIR/SOURCES-windows.sha256"
    fi
    mv -f "$WORK/omg-new$EXE" "$OMG_BIN"
fi

# Earlier versions of the toolchain were four binaries. `omg` does all
# of it now; clear out leftovers from an older build.
rm -f "$BIN_DIR/omgc" "$BIN_DIR/omgcc" "$BIN_DIR/omgjs"

# Record which sources each binary was built from, so that
# check-bin.sh (and CI) can tell when a checked-in binary has fallen
# behind src/. One stamp per binary: SOURCES.sha256 for
# `omg`, SOURCES-windows.sha256 for `omg.exe`.
if [ -n "$WINDOWS" ]; then
    scripts/sources-hash.sh > "$BIN_DIR/SOURCES-windows.sha256"
else
    scripts/sources-hash.sh > "$BIN_DIR/SOURCES.sha256"
fi

echo
echo "Native toolchain in $BIN_DIR:"
ls -la "$BIN_DIR/"
echo
if [ -n "$WINDOWS" ]; then
    echo "Run a program with:    bin\\omg.exe foo.omg"
    echo "Build an executable:   bin\\omg.exe --build foo.omg"
else
    echo "Run a program with:    $BIN_DIR/omg foo.omg"
    echo "Build an executable:   $BIN_DIR/omg --build foo.omg"
fi
