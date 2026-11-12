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

WEB_OUT=web/examples
mkdir -p "$WEB_OUT"
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Pick a toolchain. Prefer the native `omg`; if it can't run on this
# host (the checked-in one is Linux x86-64 — so: macOS, Windows under
# Git Bash, ARM) fall back to the Rust runtime, which compiles to the
# same bytecode and runs the same OMG-written JS back end (through the
# same driver, bootstrap/src/omg.omg), just hosted on the Rust VM.
can_emit_js() {
    [ -x "$1" ] && "$1" --help 2>/dev/null | grep -q -- '--emit-js'
}
RUST_OMG=runtime/target/release/omg
if [ ! -x "$RUST_OMG" ] && [ -x "$RUST_OMG.exe" ]; then
    RUST_OMG="$RUST_OMG.exe"
fi
if can_emit_js bootstrap/bin/omg; then
    echo "Using the native toolchain (bootstrap/bin/omg)"
    omg_compile() { bootstrap/bin/omg --compile "$1" "$2"; }
    omg_to_js()   { bootstrap/bin/omg --emit-js "$1" "$2"; }
elif [ -x "$RUST_OMG" ]; then
    echo "Using the Rust runtime ($RUST_OMG)"
    omg_compile() { "$RUST_OMG" --compile "$1" "$2"; }
    omg_to_js()   { "$RUST_OMG" --rust bootstrap/src/omg.omg --emit-js "$1" "$2"; }
else
    echo "Need either a bootstrap/bin/omg that runs on this host (bootstrap/build.sh)"
    echo "or the Rust runtime (cargo build --release --manifest-path runtime/Cargo.toml)."
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

# === Primary: the meta-circular bundle ====================================

echo "Building web/omg-web.js (compiler + VM + driver) ..."
omg_compile bootstrap/src/omg-web.omg "$WORK/omg-web.omgb"
omg_to_js "$WORK/omg-web.omgb" web/omg-web.js
strip_root web/omg-web.js
echo "  $(wc -c < web/omg-web.js | tr -d ' ') bytes"

# === Compiler Explorer bundle =============================================
# Same shape as omg-web.js but the driver also imports native-c.omg and
# native-js.omg, so the bundle exposes both back ends to the
# browser (web/explorer.html). Bigger bundle (~3.5 MB) — only loaded on
# the explorer page.

echo "Building web/omg-explorer.js (compiler + VM + transpilers + driver) ..."
omg_compile bootstrap/src/omg-explorer.omg "$WORK/omg-explorer.omgb"
omg_to_js "$WORK/omg-explorer.omgb" web/omg-explorer.js
strip_root web/omg-explorer.js
echo "  $(wc -c < web/omg-explorer.js | tr -d ' ') bytes"

# === Reference example pairs ==============================================

EXAMPLES=(
    assignment bitwise dictionaries floats hex_to_rgb
    higher_order matrix_ops maze_solver merge_sort
    permissions prime_sieve rot_13 stack_vm stack_vm_and_asm
    tabula_recta
)

count=0
manifest_entries=()
for name in "${EXAMPLES[@]}"; do
    src="examples/$name.omg"
    if [ ! -f "$src" ]; then
        continue
    fi
    cp "$src" "$WEB_OUT/$name.omg"
    omg_compile "$src" "$WORK/$name.omgb"
    omg_to_js "$WORK/$name.omgb" "$WEB_OUT/$name.js"
    strip_root "$WEB_OUT/$name.js"
    manifest_entries+=("\"$name\"")
    count=$((count + 1))
done

# Manifest consumed by web/app.js and web/explorer.app.js to populate the
# "Examples" dropdown. Single source of truth — names come from the
# EXAMPLES array above, filtered to what actually exists on disk.
manifest_csv=$(IFS=,; echo "${manifest_entries[*]}")
printf '[%s]\n' "$manifest_csv" > "$WEB_OUT/manifest.json"

echo "Built $count reference example pairs in $WEB_OUT/ (+ manifest.json)"

echo
echo "Serve the playground:"
echo "  cd web && python3 -m http.server"
echo "  open http://localhost:8000"
