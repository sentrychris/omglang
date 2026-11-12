#!/bin/bash
# Fail if the binaries checked into bootstrap/bin/ were not built from
# the sources in bootstrap/src/ as they are now.
#
# The binaries are committed so that a fresh clone has a working native
# toolchain without needing the Rust runtime: `omg` for Linux x86-64
# and `omg.exe` for Windows x86-64. That only holds if they are rebuilt
# whenever their sources change, and nothing used to check it: they
# could (and did) fall several fixes behind the sources.
#
# build.sh stamps each binary with a hash of the sources it was built
# from (SOURCES.sha256 for omg, SOURCES-windows.sha256 for omg.exe);
# this compares both stamps with the sources.
#
# Usage: bootstrap/check-bin.sh
# Exit codes: 0 current, 1 stale or never stamped.
set -u
cd "$(dirname "$0")/.."

want=$(bootstrap/sources-hash.sh) || exit 1
stale=0

check() {
    local binary="$1" stamp="$2" have
    have=$(tr -d '\r\n ' < "$stamp" 2>/dev/null)
    if [ ! -f "bootstrap/bin/$binary" ]; then
        echo "bootstrap/bin/$binary is missing."
        stale=1
    elif [ "$want" = "$have" ]; then
        echo "bootstrap/bin/$binary is current ($want)"
    else
        echo "bootstrap/bin/$binary is stale: it was not built from the current bootstrap/src."
        echo "  sources hash now: $want"
        echo "  built from:       ${have:-<no stamp>}"
        stale=1
    fi
}

check omg     bootstrap/bin/SOURCES.sha256
check omg.exe bootstrap/bin/SOURCES-windows.sha256

for f in omg_rt.h omg_rt.js; do
    if ! cmp -s "bootstrap/src/$f" "bootstrap/bin/$f"; then
        echo "bootstrap/bin/$f differs from bootstrap/src/$f"
        stale=1
    fi
done

if [ "$stale" != 0 ]; then
    echo
    echo "Rebuild and commit the result. On Linux x86-64, with the MinGW-w64"
    echo "cross compiler installed so that omg.exe is rebuilt too, and twice,"
    echo "so that the committed binaries are the toolchain compiling itself:"
    echo "  sudo apt install gcc-mingw-w64-x86-64"
    echo "  cargo build --release --manifest-path runtime/Cargo.toml"
    echo "  bootstrap/build.sh && bootstrap/build.sh"
    echo "  git add bootstrap/bin"
    exit 1
fi
exit 0
