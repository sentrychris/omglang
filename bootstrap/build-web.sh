#!/bin/bash
# Build the static assets for web/ — pre-transpile each example to
# JavaScript so the playground can run them without the OMG toolchain.
#
# Output: web/examples/<name>.omg + web/examples/<name>.js
#
# In-browser compilation (`vm.omg` compiled to JS, running
# `compiler.omgb` on user input) is a future step; this script ships
# the simpler "fixed corpus" demo.

set -e
cd "$(dirname "$0")/.."

NATIVE=bootstrap/bin/omg
WEB_OUT=web/examples
mkdir -p "$WEB_OUT"

# Examples that round-trip cleanly through native-js (matches the
# parity-test corpus minus self_hosted, which has a known meta-
# interpreter divergence on every native path).
EXAMPLES=(
    assignment bitwise dictionaries floats hello_world hex_to_rgb
    higher_order import_modules matrix_ops maze_solver merge_sort
    permissions prime_sieve rot_13 stack_vm stack_vm_and_asm
    tabula_recta
)

WORK=$(mktemp -d)
trap "rm -rf $WORK" EXIT

count=0
for name in "${EXAMPLES[@]}"; do
    src="examples/$name.omg"
    if [ ! -f "$src" ]; then
        echo "skip: $src (missing)"
        continue
    fi
    cp "$src" "$WEB_OUT/$name.omg"
    "$NATIVE" --compile "$src" "$WORK/$name.omgb" >/dev/null
    "$NATIVE" bootstrap/src/native-js.omg "$WORK/$name.omgb" "$WEB_OUT/$name.js" >/dev/null
    count=$((count + 1))
done

echo "Built $count example pairs in $WEB_OUT/"
echo
echo "Serve the playground:"
echo "  cd web && python3 -m http.server"
echo "  open http://localhost:8000"
