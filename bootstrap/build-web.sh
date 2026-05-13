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

OMG=bootstrap/bin/omg
OMGJS_NATIVE=bootstrap/bin/omgjs
WEB_OUT=web/examples
mkdir -p "$WEB_OUT"
WORK=$(mktemp -d)
trap "rm -rf $WORK" EXIT

# === Primary: the meta-circular bundle ====================================

echo "Building web/omg-web.js (compiler + VM + driver) ..."
"$OMG" --compile bootstrap/src/omg-web.omg "$WORK/omg-web.omgb"
"$OMGJS_NATIVE" "$WORK/omg-web.omgb" web/omg-web.js
echo "  $(wc -c < web/omg-web.js | tr -d ' ') bytes"

# === Compiler Explorer bundle =============================================
# Same shape as omg-web.js but the driver also imports native-c.omg and
# native-js.omg, so the bundle exposes every transpiler stage to the
# browser (web/explorer.html). Bigger bundle (~3.5 MB) — only loaded on
# the explorer page.

echo "Building web/omg-explorer.js (compiler + VM + transpilers + driver) ..."
"$OMG" --compile bootstrap/src/omg-explorer.omg "$WORK/omg-explorer.omgb"
"$OMGJS_NATIVE" "$WORK/omg-explorer.omgb" web/omg-explorer.js
echo "  $(wc -c < web/omg-explorer.js | tr -d ' ') bytes"

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
    "$OMG" --compile "$src" "$WORK/$name.omgb" >/dev/null
    "$OMGJS_NATIVE" "$WORK/$name.omgb" "$WEB_OUT/$name.js" >/dev/null
    count=$((count + 1))
done
echo "Built $count reference example pairs in $WEB_OUT/"

echo
echo "Serve the playground:"
echo "  cd web && python3 -m http.server"
echo "  open http://localhost:8000"
