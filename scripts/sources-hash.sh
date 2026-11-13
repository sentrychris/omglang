#!/bin/bash
# Print one SHA-256 over everything that goes into bin/: the
# OMG sources of the toolchain and the two runtime files that are
# spliced into transpiled output.
#
# build.sh records this hash in bin/SOURCES.sha256 when it
# builds the binaries; check-bin.sh compares the two, which is how CI
# notices that a change to src/ was committed without
# rebuilding the binaries that are checked in next to it.
set -e
cd "$(dirname "$0")/.."

if command -v sha256sum >/dev/null 2>&1; then
    sha() { sha256sum | cut -d' ' -f1; }
else
    sha() { shasum -a 256 | cut -d' ' -f1; }
fi

# Sorted, so the order does not depend on the locale's glob order. CRs
# are dropped so a checkout with translated line endings hashes the same.
# (src/runtime/x64/ is where the machine-code runtime will live; until
# then that pattern matches nothing, which is fine.)
for f in $(ls src/*.omg src/backends/*.omg src/browser/*.omg \
               src/runtime/omg_rt.h src/runtime/omg_rt.js \
               src/runtime/x64/*.omg 2>/dev/null | LC_ALL=C sort); do
    printf '%s\n' "$f"
    tr -d '\r' < "$f"
done | sha
