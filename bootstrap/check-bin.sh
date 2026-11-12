#!/bin/bash
# Fail if the binaries checked into bootstrap/bin/ were not built from
# the sources in bootstrap/src/ as they are now.
#
# The binaries are committed so that a fresh clone has a working native
# toolchain without needing the Rust runtime. That only holds if they
# are rebuilt whenever their sources change, and nothing used to check
# it: they could (and did) fall several fixes behind the sources.
#
# Usage: bootstrap/check-bin.sh
# Exit codes: 0 current, 1 stale or never stamped.
set -u
cd "$(dirname "$0")/.."

STAMP=bootstrap/bin/SOURCES.sha256
want=$(bootstrap/sources-hash.sh) || exit 1
have=$(tr -d '\r\n ' < "$STAMP" 2>/dev/null)

if [ "$want" = "$have" ]; then
    echo "bootstrap/bin is current ($want)"
    exit 0
fi

echo "bootstrap/bin is stale: it was not built from the current bootstrap/src."
echo "  sources hash now:   $want"
echo "  binaries built from: ${have:-<no stamp>}"
echo
echo "Rebuild and commit the result (on Linux x86-64, twice, so the"
echo "committed binaries are the toolchain compiling itself):"
echo "  cargo build --release --manifest-path runtime/Cargo.toml"
echo "  bootstrap/build.sh && bootstrap/build.sh"
echo "  git add bootstrap/bin"
for f in omg_rt.h omg_rt.js; do
    if ! cmp -s "bootstrap/src/$f" "bootstrap/bin/$f"; then
        echo "  (bootstrap/bin/$f differs from bootstrap/src/$f)"
    fi
done
exit 1
