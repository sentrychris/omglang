# The browser bundles: the compiler and the VM (and, for the explorer,
# the C and JS back ends) put through the JS back end by
# scripts/build-web.sh, then loaded and run the way the site's
# playground loads and runs them. The checks themselves are in
# tests/web-bundles.mjs, which says what they are.
#
# Nothing of this is checked in: the bundles are built here, into the
# test's own temp directory, by the omg under test. Needs node, like
# everything else about JavaScript; skipped without it.

section "Web bundles"

if command -v node >/dev/null 2>&1; then
    WEB_OUT="$TMPDIR_TEST/web"
    if OMG_NATIVE="$OMG_NATIVE" "$REPO_ROOT/scripts/build-web.sh" "$WEB_OUT" >"$TMPDIR_TEST/web-build.log" 2>&1; then
        pass "scripts/build-web.sh builds the bundles"

        # The build must not leave this machine's paths in what it
        # writes, or two machines would publish different files: the
        # source files are named from the repository root.
        if grep -q -F '"src/compiler.omg"' "$WEB_OUT/omg-web.js" &&
           grep -q -F '"src/compiler.omg"' "$WEB_OUT/omg-explorer.js"; then
            pass "the bundles name their sources from the repository root"
        else
            fail "the bundles name their sources by this checkout's path" \
                "$(grep -m 3 -F 'compiler.omg"' "$WEB_OUT/omg-web.js")"
        fi

        if node "$REPO_ROOT/tests/web-bundles.mjs" "$WEB_OUT" >"$TMPDIR_TEST/web-bundles.log" 2>&1; then
            pass "tests/web-bundles.mjs: $(tail -n 1 "$TMPDIR_TEST/web-bundles.log")"
        else
            fail "tests/web-bundles.mjs"
            grep -A 4 '^\[FAIL\]' "$TMPDIR_TEST/web-bundles.log" | head -n 60
            tail -n 3 "$TMPDIR_TEST/web-bundles.log"
        fi
    else
        fail "scripts/build-web.sh builds the bundles"
        tail -n 20 "$TMPDIR_TEST/web-build.log"
    fi
else
    echo "  (skipped: node not installed)"
fi
