#!/bin/bash
# Build the static assets for web/.
#
# Primary artifact:
#   web/omg-web.js — the OMG-in-OMG compiler + VM + driver, compiled
#                    to a single JS bundle. This is what the playground
#                    actually loads and re-evaluates per Run click;
#                    user-typed source goes in via globalThis.args[1],
#                    output comes back through the redirected omg_emit.
#
# Reference artifacts (web/examples/):
#   <name>.omg + <name>.js — pre-built example pairs so visitors can
#                            inspect what native-js.omg's output looks
#                            like without running the build themselves.

set -e
cd "$(dirname "$0")/.."

NATIVE=bootstrap/bin/omg
WEB_OUT=web/examples
mkdir -p "$WEB_OUT"
WORK=$(mktemp -d)
trap "rm -rf $WORK" EXIT

# === Primary: the meta-circular bundle ====================================

echo "Building web/omg-web.js (compiler + VM + driver) ..."
"$NATIVE" --compile bootstrap/src/omg-web.omg "$WORK/omg-web.omgb"
"$NATIVE" bootstrap/src/native-js.omg "$WORK/omg-web.omgb" web/omg-web.js
echo "  $(wc -c < web/omg-web.js | tr -d ' ') bytes"

# === Reference example pairs ==============================================

EXAMPLES=(
    assignment bitwise dictionaries floats hello_world hex_to_rgb
    higher_order import_modules matrix_ops maze_solver merge_sort
    permissions prime_sieve rot_13 stack_vm stack_vm_and_asm
    tabula_recta
)

count=0
for name in "${EXAMPLES[@]}"; do
    src="examples/$name.omg"
    if [ ! -f "$src" ]; then
        continue
    fi
    cp "$src" "$WEB_OUT/$name.omg"
    "$NATIVE" --compile "$src" "$WORK/$name.omgb" >/dev/null
    "$NATIVE" bootstrap/src/native-js.omg "$WORK/$name.omgb" "$WEB_OUT/$name.js" >/dev/null
    count=$((count + 1))
done
echo "Built $count reference example pairs in $WEB_OUT/"

echo
echo "Serve the playground:"
echo "  cd web && python3 -m http.server"
echo "  open http://localhost:8000"
