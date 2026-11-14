#!/bin/bash
# Bring the OMG site up to date with this checkout.
#
# The site (https://versyx.digital/omglang/) has the playground, and it
# is not in this repository. Its playground loads three files that are
# made from the sources here. This script builds the three files, tests
# them against the site's own loader, and copies them over:
#
#     build/web/omg-web.js       ->  <site>/assets/omg-web.js
#     build/web/omg-explorer.js  ->  <site>/assets/omg-explorer.js
#     build/web/omg-rt.js        ->  <site>/assets/omg-rt.js
#
# Nothing else on the site is touched. Its pages, and the programs in
# its examples/ (some of them changed to suit a browser), are its own.
#
# Usage:
#     scripts/update-site.sh [site-dir]
#
# The site directory is the folder that holds the site's index.html
# and assets/. It is taken from the argument, or from OMG_SITE, or is
# ../rowles-portfolio/public/omglang beside this checkout. On Windows
# run this from Git Bash, like the other scripts.
#
# It writes files and nothing else: committing and deploying the site
# is done there, by hand. Run it after anything under src/ changes
# (and scripts/build.sh has been run), so that the playground does not
# go on running an older compiler.

set -e
cd "$(dirname "$0")/.."

SITE=${1:-${OMG_SITE:-../rowles-portfolio/public/omglang}}
if [ ! -f "$SITE/assets/omg-worker.js" ]; then
    echo "No OMG site at $SITE (looked for assets/omg-worker.js)."
    echo "Give the folder that holds the site's index.html and assets/:"
    echo "  scripts/update-site.sh path/to/public/omglang"
    echo "or set OMG_SITE."
    exit 1
fi
echo "Site: $SITE"

scripts/build-web.sh build/web

# The site rewrites a few lines of the runtime to load a bundle, and
# stops loading if one of them has changed. So run the new bundles
# through the site's loader before they go anywhere.
if command -v node >/dev/null 2>&1; then
    echo "Testing the bundles with the site's loader ..."
    if ! node tests/web-bundles.mjs build/web --worker "$SITE/assets/omg-worker.js" > build/web/test.log 2>&1; then
        grep -A 4 '^\[FAIL\]' build/web/test.log | head -n 60
        tail -n 1 build/web/test.log
        echo "Nothing was copied. The whole report is in build/web/test.log."
        exit 1
    fi
    echo "  $(tail -n 1 build/web/test.log)"
else
    echo "node is not installed, so the bundles were NOT tested with the site's loader."
fi

# Line endings: a checkout on Windows may have CRLF in the runtime.
# What is published has LF, wherever it was built.
changed=0
put() {
    local from="$1" to="$2" was
    tr -d '\r' < "$from" > "$to.new"
    if [ -f "$to" ] && cmp -s "$to.new" "$to"; then
        rm -f "$to.new"
        echo "  unchanged  ${to#"$SITE"/}"
    else
        was=new
        [ -f "$to" ] && was="$(wc -c < "$to" | tr -d ' ') bytes"
        mv -f "$to.new" "$to"
        echo "  updated    ${to#"$SITE"/} ($was -> $(wc -c < "$to" | tr -d ' ') bytes)"
        changed=$((changed + 1))
    fi
}

echo "Copying:"
for name in omg-web.js omg-explorer.js omg-rt.js; do
    put "build/web/$name" "$SITE/assets/$name"
done

echo
if [ "$changed" = 0 ]; then
    echo "The site already has what this checkout builds."
else
    echo "$changed file(s) changed in $SITE."
    echo "Commit and deploy the site from its own repository."
fi
