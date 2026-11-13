#!/bin/bash
# Fail if the binaries checked into bin/ were not built from
# the sources in src/ as they are now.
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
# Usage: scripts/check-bin.sh
# Exit codes: 0 current, 1 stale or never stamped.
set -u
cd "$(dirname "$0")/.."

want=$(scripts/sources-hash.sh) || exit 1
stale=0

check() {
    local binary="$1" stamp="$2" have
    have=$(tr -d '\r\n ' < "$stamp" 2>/dev/null)
    if [ ! -f "bin/$binary" ]; then
        echo "bin/$binary is missing."
        stale=1
    elif [ "$want" = "$have" ]; then
        echo "bin/$binary is current ($want)"
    else
        echo "bin/$binary is stale: it was not built from the current src."
        echo "  sources hash now: $want"
        echo "  built from:       ${have:-<no stamp>}"
        stale=1
    fi
}

check omg     bin/SOURCES.sha256
check omg.exe bin/SOURCES-windows.sha256

for f in omg_rt.h omg_rt.js x64/core.omg x64/float.omg x64/builtins.omg x64/linux.omg x64/windows.omg; do
    if ! cmp -s "src/runtime/$f" "bin/$f"; then
        echo "bin/$f differs from src/runtime/$f"
        stale=1
    fi
done

if [ "$stale" != 0 ]; then
    echo
    echo "Rebuild and commit the result, on Linux x86-64 (that builds omg.exe"
    echo "too, and needs nothing installed):"
    echo "  scripts/build.sh"
    echo "  git add bin"
    exit 1
fi
exit 0
