#!/bin/bash
# Build the three files that put OMG in a browser.
#
#   omg-web.js       the compiler, the VM and a small driver, compiled
#                    to one JavaScript file: runs a program
#   omg-explorer.js  the same plus the C and JS back ends: reports
#                    every stage (tokens, AST, bytecode, C, JavaScript)
#                    and then runs the program
#   omg-rt.js        src/runtime/omg_rt.js under the name a page asks
#                    for: the runtime that goes in front of JavaScript
#                    the explorer generated, so that it can be run
#
# They are written to build/web/, or to the directory given:
#
#     scripts/build-web.sh [out-dir]
#
# Nothing here is checked in. The playground that loads these files is
# the one on the site (https://versyx.digital/omglang/playground/),
# which lives in another repository: scripts/update-site.sh builds
# them, tests them and copies them there. tests/web.sh builds and
# tests them as part of the suite.

set -e
cd "$(dirname "$0")/.."

OUT=${1:-build/web}
mkdir -p "$OUT"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Pick a toolchain: the native `omg` (omg.exe on Windows), or the one
# named by OMG_NATIVE, as in the tests. If none runs on this host (the
# checked-in ones are for Linux and Windows on x86-64; on macOS or
# ARM, scripts/build.sh makes one) and someone has built the frozen
# Rust runtime by hand, that works as well: it compiles to the same
# bytecode and runs the same OMG-written JS back end through the same
# driver, src/omg.omg.
can_emit_js() {
    [ -x "$1" ] && "$1" --help 2>/dev/null | grep -q -- '--emit-js'
}
NATIVE_OMG=${OMG_NATIVE:-bin/omg}
case "$(uname -s)" in
    MINGW*|MSYS*) [ -f "$NATIVE_OMG.exe" ] && NATIVE_OMG="$NATIVE_OMG.exe" ;;
esac
RUST_OMG=rust/target/release/omg
if [ ! -x "$RUST_OMG" ] && [ -x "$RUST_OMG.exe" ]; then
    RUST_OMG="$RUST_OMG.exe"
fi
if can_emit_js "$NATIVE_OMG"; then
    echo "Using the native toolchain ($NATIVE_OMG)"
    omg_compile() { "$NATIVE_OMG" --compile "$1" "$2"; }
    omg_to_js()   { "$NATIVE_OMG" --emit-js "$1" "$2"; }
elif [ -x "$RUST_OMG" ]; then
    echo "Using the Rust runtime ($RUST_OMG)"
    omg_compile() { "$RUST_OMG" --compile "$1" "$2"; }
    omg_to_js()   { "$RUST_OMG" --rust src/omg.omg --emit-js "$1" "$2"; }
else
    echo "Need a bin/omg that runs on this host: see scripts/build.sh, and"
    echo "docs/01-quickstart.md (\"Other systems\") if there is none yet."
    exit 1
fi

# The compiler records each source file's absolute path for tracebacks,
# which would bake this machine's checkout location into the published
# bundles (and make them differ on every machine). Rewrite those paths
# relative to the repository root. `pwd -W` is the Windows-style path
# under Git Bash, which is the form the compiler sees there.
REPO_ROOT=$(pwd -W 2>/dev/null || pwd)
#
# The first line of the output names the bytecode file it came from, which
# lives in a random temp directory; keep just the file name so repeated
# builds produce identical output.
strip_root() {
    sed -e "s|$REPO_ROOT/||g" \
        -e '1s|from .*/\([^/]*\.omgb\)$|from \1|' \
        "$1" > "$1.tmp" && mv -f "$1.tmp" "$1"
}

# One bundle: src/browser/<name>.omg to bytecode, bytecode to
# JavaScript. The JS back end inlines every module the driver imports,
# so the compiler and the VM come along.
bundle() {
    echo "Building $OUT/$1.js ($2) ..."
    omg_compile "src/browser/$1.omg" "$WORK/$1.omgb"
    omg_to_js "$WORK/$1.omgb" "$WORK/$1.js"
    strip_root "$WORK/$1.js"
    mv -f "$WORK/$1.js" "$OUT/$1.js"
    echo "  $(wc -c < "$OUT/$1.js" | tr -d ' ') bytes"
}

bundle omg-web      "compiler + VM + driver"
bundle omg-explorer "compiler + VM + the C and JS back ends + driver"

cp src/runtime/omg_rt.js "$OUT/omg-rt.js"
echo "Copied src/runtime/omg_rt.js to $OUT/omg-rt.js"
