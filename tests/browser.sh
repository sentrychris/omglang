#!/bin/bash
# The web browser in tools/browser.
#
# tests/browser/check.omg asks each part one question and prints the
# answer; the answers are compared with ones that were worked out
# somewhere else: the hashes, ciphers and public-key sums with Python's
# hashlib and the "cryptography" package, the PNG and GIF pictures with
# Pillow, the certificates with openssl (tests/browser/README.md says
# how each file was made). The handshakes are held with `openssl
# s_server` where there is an openssl, and plain HTTP with
# tests/browser/server.omg, which answers awkwardly on purpose. The
# JavaScript engine is compared with Node, and pages with scripts with
# Chromium: what each printed is kept in tests/browser/js/want and
# tests/browser/want/js-*.txt.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

cd "$REPO_ROOT"

BR_W="$TMPDIR_TEST/browser"
BR_PORT=19971
BR_TLS_PORT=19972
mkdir -p "$BR_W"

br() { "$BR_W/check" "$@" 2>&1 | tr -d '\r'; }

section "Browser: its parts (tools/browser)"

"$OMG_NATIVE" --build tests/browser/check.omg "$BR_W/check" >/dev/null 2>&1
if is_executable "$BR_W/check"; then
    pass "the checker builds"
else
    fail "the checker builds"
fi

# 101 vectors: SHA-2, HMAC, HKDF, both ciphers (and that a changed
# byte is noticed), X25519 with the two examples of RFC 7748, key
# agreement and ECDSA on P-256 and P-384, RSA signatures old and PSS.
assert_eq "hashes, ciphers and public-key sums give the reference answers" \
    "hash: 30
hmac: 15
hkdf: 9
aead: 24
x25519: 5
ecdh: 4
ecdsa: 6
rsa: 4
pss: 4
wrong: 0" \
    "$(br crypto tests/browser/vectors.txt)"

# Certificates: a root (RSA), an issuer under it (P-384) and a server
# certificate under that (P-256), all good for a hundred years. The
# time is given, so the answers do not change as the years pass.
BR_PKI=tests/browser/pki
BR_NOW=1800000000
assert_eq "a chain that leads to a trusted root is good" "good" \
    "$(br cert $BR_PKI/chain.pem site.test $BR_NOW $BR_PKI/root.pem)"
assert_eq "a * in a certificate stands for one label" "good" \
    "$(br cert $BR_PKI/chain.pem a.wild.test $BR_NOW $BR_PKI/root.pem)"
assert_eq "... and for one only" "its certificate is for site.test, not for a.b.wild.test" \
    "$(br cert $BR_PKI/chain.pem a.b.wild.test $BR_NOW $BR_PKI/root.pem)"
assert_eq "a certificate can name an address" "good" \
    "$(br cert $BR_PKI/chain.pem 127.0.0.1 $BR_NOW $BR_PKI/root.pem)"
assert_eq "a certificate for another name is refused" "its certificate is for site.test, not for other.test" \
    "$(br cert $BR_PKI/chain.pem other.test $BR_NOW $BR_PKI/root.pem)"
assert_eq "a root nobody vouched for is refused" \
    "its certificate was issued by OMG Test Issuer, which this browser does not know" \
    "$(br cert $BR_PKI/chain.pem site.test $BR_NOW)"
assert_contains "a certificate from the future is refused" "is not valid before" \
    "$(br cert $BR_PKI/chain.pem site.test 1000000000 $BR_PKI/root.pem)"
assert_contains "a certificate that ran out is refused" "(site.test) ran out on" \
    "$(br cert $BR_PKI/chain-short.pem site.test 1900000000 $BR_PKI/root.pem)"
assert_eq "a chain with a link missing is refused" \
    "its certificate was issued by OMG Test Issuer, which this browser does not know" \
    "$(br cert $BR_PKI/chain-alone.pem site.test $BR_NOW $BR_PKI/root.pem)"
assert_eq "a certificate with one bit changed is refused" \
    "a signature in its chain of certificates is wrong" \
    "$(br cert $BR_PKI/chain-altered.pem site.test $BR_NOW $BR_PKI/root.pem)"
assert_eq "a certificate signed by one that may not sign is refused" \
    "a certificate in its chain (site.test) is not allowed to sign others" \
    "$(br cert $BR_PKI/chain-below.pem below.test $BR_NOW $BR_PKI/root.pem)"
assert_eq "a certificate signed with RSA-PSS is good" "good" \
    "$(br cert $BR_PKI/rsa.pem rsa.test $BR_NOW $BR_PKI/root.pem)"
assert_eq "a self-signed certificate is refused" \
    "its certificate was issued by site.test, which this browser does not know" \
    "$(br cert $BR_PKI/lone.pem site.test $BR_NOW $BR_PKI/root.pem)"
assert_eq "... unless it is itself on the list" "good" \
    "$(br cert $BR_PKI/lone.pem site.test $BR_NOW $BR_PKI/lone.pem)"

assert_eq "addresses are taken apart and links resolved" \
    "$(tr -d '\r' < tests/browser/want/urls.txt)" "$(br url tests/browser/urls.txt)"
assert_eq "the HTML parser builds the expected tree" \
    "$(tr -d '\r' < tests/browser/want/dom.txt)" "$(br dom tests/browser/pages/dom.html)"
assert_eq "a gzip file unpacks to what was packed" \
    "14740 cd6584ac61db682c9ff7e4e8a0cb9db59aa6015c9b924caef45b7f770628d936" \
    "$(br gunzip tests/browser/pages/twenty.gz)"

# PNG and GIF decode exactly, so these are Pillow's pixels. JPEG
# decoders may differ by a unit or two; those three are this decoder's
# own, kept after they were seen to be within 2 of Pillow's.
while read -r br_name br_want; do
    [ -z "$br_name" ] && continue
    assert_eq "picture $br_name decodes to the expected pixels" "44x30 $br_want" \
        "$(br image tests/browser/pages/$br_name)"
done <<'EOF'
rgb.png 8487c7c62d42b3f8b4e0387d4adb1b43df3a57c10da26e61089e0b95e92f491f
rgba.png 5f1bddfb75910a8cddc2525ab76927785261c348e08bf42542e349e0ea35864c
palette.png 5c4144e7dd69fc685a475a85a4ba276eb5e9da2b6da2f0de76d8fe846bdeb43f
gray.png ca3b073eab02fb43e969c8e0b29004185082fff7dd470bfb4b791cbd4a2f87c4
plain.gif 6773c563c0daf7af7b8eb6f74a03094d9d246aebb55c7efaccf35f0d250d64cb
see-through.gif b509e7948256cfe3cf6b01c52b3a5eaa71d115bad4a61f9169288c4859932f71
baseline.jpg 8427e5bcbd1278f74c13f96ab99ab7a0b79d8495cd4f4a0d91ad455fdc33e5bc
progressive.jpg 5c4771c11fcb68a950e38c06d51d3721cb4df054ce06bcd44c2a7727b4411847
gray.jpg d4835f941e0d9fae85208274c4d4f0fd1af6a7ebabf1e6daab2a3f65104dcf2d
EOF

# === Layout and painting =====================================================
section "Browser: layout and painting"

"$OMG_NATIVE" --build tools/browser/browser.omg "$BR_W/browser" >/dev/null 2>&1
if is_executable "$BR_W/browser"; then
    pass "the browser builds"
else
    fail "the browser builds"
fi

# The pages used here hold no text, so that nothing depends on which
# fonts a machine has. Still the layout asks for a font to know how
# tall a line is: with no font at all there is nothing to test.
if [ "$(br fonts)" = "found" ]; then
    # collapsing margins, floats and clear, absolute positions, auto
    # margins, flex rows and columns, a grid with fr tracks and a
    # spanning item, a table, inline blocks, min/max, border-box
    assert_eq "boxes go where CSS says (800 wide)" \
        "$(tr -d '\r' < tests/browser/want/boxes.txt)" "$(br boxes tests/browser/pages/boxes.html 800)"
    assert_contains "percentages follow the width of the page" "block div#fr 276,94 114x40" \
        "$(br boxes tests/browser/pages/boxes.html 400)"

    "$BR_W/browser" tests/browser/pages/paint.html --page-only --size 240x190 \
        --screenshot "$BR_W/paint.png" >/dev/null 2>&1
    assert_eq "a page is painted to the same pixels everywhere" \
        "b85dbcc9f1f5c18db816c69f16b63a36bd5e4ddf2f3350f72f114469dda66b43" "$(br sum "$BR_W/paint.png")"

    "$BR_W/browser" tests/browser/pages/dom.html --screenshot "$BR_W/dom.png" >/dev/null 2>&1
    assert_eq "a page with text is drawn (the window as a PNG file)" "89504e47" \
        "$(head -c 4 "$BR_W/dom.png" 2>/dev/null | od -An -tx1 | tr -d ' \n')"
    assert_exit_code "a page that is not there gives an error page, not a crash" 0 \
        "$BR_W/browser" "$BR_W/no-such-page.html" --screenshot "$BR_W/none.png"
else
    echo "  (no TrueType font found on this machine: layout and painting not tested)"
fi
assert_exit_code "an unknown option is refused" 2 "$BR_W/browser" --no-such-option

# === HTTP ====================================================================
section "Browser: HTTP"

"$OMG_NATIVE" --build tests/browser/server.omg "$BR_W/server" >/dev/null 2>&1
"$BR_W/server" $BR_PORT > "$BR_W/server.log" 2>&1 &
BR_PID=$!
br_tries=0
while [ $br_tries -lt 50 ]; do
    [ "$(curl -s -m 2 "http://127.0.0.1:$BR_PORT/ready" 2>/dev/null)" != "" ] && break
    sleep 0.2
    br_tries=$((br_tries + 1))
done
BR_AT="http://127.0.0.1:$BR_PORT"

assert_contains "a page is fetched" "200 text/html $BR_AT/hello
<p>hello" "$(br fetch $BR_AT/hello)"
assert_eq "a body sent in chunks is put together" "200 text/plain $BR_AT/chunked
in three parts" "$(br fetch $BR_AT/chunked)"
assert_eq "a gzipped body is unpacked" "200 text/plain $BR_AT/gzip
squeezed, then sent" "$(br fetch $BR_AT/gzip)"
assert_eq "a redirect is followed, and the address is the new one" "200 text/plain $BR_AT/landed?from=redirect
GET /landed?from=redirect [] " "$(br fetch $BR_AT/redirect)"
assert_eq "a redirect that never ends is given up" \
    "failed: the site keeps sending the browser somewhere else" "$(br fetch $BR_AT/loop)"
assert_eq "cookies come back, but only where their path says" "200 text/plain $BR_AT/show
cookies: taste=ginger" "$(br fetch $BR_AT/cookie)"
assert_eq "a body that ends when the connection does is read whole" "200 text/plain $BR_AT/until-close
all there is" "$(br fetch $BR_AT/until-close)"
assert_eq "an error status still has a body" "404 text/plain $BR_AT/missing
nothing here" "$(br fetch $BR_AT/missing)"
assert_contains "a server that is not there is reported" "failed: could not connect" \
    "$(br fetch http://127.0.0.1:19970/)"

# === JavaScript ==============================================================
section "Browser: JavaScript"

# The engine alone first: each script of tests/browser/js prints
# lines, and want/ holds what Node prints for the same script
# (differences.js is the exception: it holds what this engine does
# differently, on purpose or not yet, and its answers are its own).
"$OMG_NATIVE" --build tools/browser/js/run.omg "$BR_W/js" >/dev/null 2>&1
if is_executable "$BR_W/js"; then
    pass "the JavaScript engine builds"
else
    fail "the JavaScript engine builds"
fi
for br_js in tests/browser/js/*.js; do
    br_name=$(basename "$br_js" .js)
    assert_eq "script $br_name.js prints what it should" \
        "$(tr -d '\r' < "tests/browser/js/want/$br_name.txt")" \
        "$("$BR_W/js" "$br_js" 2>&1 | tr -d '\r')"
done

# Then pages, whose scripts print with console.log; want/js-*.txt is
# what Chromium prints for the same page. The boxes these pages ask
# the size of hold no text. `br_page ADDRESS`: what the page printed,
# without what the browser itself says about a file it could not get.
br_page() {
    "$BR_W/browser" --console --dump --wait 3000 "$1" 2>&1 | tr -d '\r' |
        sed '/^<html>/,$d' | grep -v '^Could not load the script'
}
if [ "$(br fonts)" = "found" ]; then
    assert_eq "scripts read and change the page as in Chromium" \
        "$(tr -d '\r' < tests/browser/want/js-dom.txt)" "$(br_page tests/browser/pages/js/dom.html)"
    assert_eq "events, forms, timers, styles and sizes as in Chromium" \
        "$(tr -d '\r' < tests/browser/want/js-events.txt)" "$(br_page tests/browser/pages/js/events.html)"
    # the test server hands the page out, so that its scripts may ask
    # the same server for more
    assert_eq "fetch, XMLHttpRequest, cookies and added scripts as in Chromium" \
        "$(tr -d '\r' < tests/browser/want/js-net.txt)" "$(br_page "$BR_AT/page/net.html")"
    assert_eq "with --no-js the scripts of a page do not run" "" \
        "$("$BR_W/browser" --no-js --console --dump tests/browser/pages/js/dom.html 2>&1 | tr -d '\r' | sed '/^<html>/,$d')"
else
    echo "  (no TrueType font found on this machine: pages with scripts not tested)"
fi

curl -s -m 5 "$BR_AT/quit" >/dev/null 2>&1
kill $BR_PID 2>/dev/null
wait $BR_PID 2>/dev/null

# === TLS =====================================================================
section "Browser: TLS"

# Against openssl's test server, with a key made on the spot. Not on
# Windows, where starting and stopping it from here is not dependable.
if [ -z "$OMG_WINDOWS" ] && command -v openssl >/dev/null 2>&1; then
    openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes \
        -keyout "$BR_W/ec.key" -out "$BR_W/ec.pem" -days 2 -subj "/CN=localhost" \
        -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" >/dev/null 2>&1
    openssl req -x509 -newkey rsa:2048 -nodes \
        -keyout "$BR_W/rsa.key" -out "$BR_W/rsa.pem" -days 2 -subj "/CN=localhost" \
        -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" >/dev/null 2>&1

    # `br_tls NAME WANT CERT s_server-options...`: start the server,
    # fetch a page from it, and compare the two lines that say how.
    br_tls() {
        local name="$1" want="$2" cert="$3"
        shift 3
        openssl s_server -accept $BR_TLS_PORT -cert "$BR_W/$cert.pem" -key "$BR_W/$cert.key" \
            -www -quiet "$@" >/dev/null 2>&1 &
        local pid=$!
        local tries=0 got=""
        while [ $tries -lt 25 ]; do
            got=$(br fetch "https://127.0.0.1:$BR_TLS_PORT/" "$BR_W/$cert.pem" | head -n 2)
            case "$got" in *"could not connect"*) sleep 0.2 ;; *) break ;; esac
            tries=$((tries + 1))
        done
        kill $pid 2>/dev/null
        wait $pid 2>/dev/null
        assert_eq "$name" "$want" "$got"
        BR_TLS_PORT=$((BR_TLS_PORT + 1))
    }
    br_ok() { printf '200 text/html https://127.0.0.1:%s/\n%s' "$BR_TLS_PORT" "$1"; }

    br_tls "TLS 1.3 with ChaCha20-Poly1305 and X25519" "$(br_ok "TLS 1.3, ChaCha20-Poly1305")" ec -tls1_3
    br_tls "TLS 1.3 with AES-128-GCM, an RSA certificate" "$(br_ok "TLS 1.3, AES-128-GCM")" rsa \
        -tls1_3 -ciphersuites TLS_AES_128_GCM_SHA256
    br_tls "TLS 1.3 with AES-256-GCM, after being asked for a P-384 key" "$(br_ok "TLS 1.3, AES-256-GCM")" ec \
        -tls1_3 -ciphersuites TLS_AES_256_GCM_SHA384 -groups P-384
    br_tls "TLS 1.3 after being asked for a P-256 key" "$(br_ok "TLS 1.3, ChaCha20-Poly1305")" rsa \
        -tls1_3 -groups P-256
    br_tls "TLS 1.2 with ECDSA and AES-128-GCM" "$(br_ok "TLS 1.2, AES-128-GCM")" ec \
        -tls1_2 -cipher ECDHE-ECDSA-AES128-GCM-SHA256
    br_tls "TLS 1.2 with RSA and AES-256-GCM" "$(br_ok "TLS 1.2, AES-256-GCM")" rsa \
        -tls1_2 -cipher ECDHE-RSA-AES256-GCM-SHA384
    br_tls "TLS 1.2 with ChaCha20-Poly1305 on P-384" "$(br_ok "TLS 1.2, ChaCha20-Poly1305")" rsa \
        -tls1_2 -cipher ECDHE-RSA-CHACHA20-POLY1305 -curves P-384
    br_tls "a server that asks for a client certificate gets none, politely" \
        "$(br_ok "TLS 1.3, ChaCha20-Poly1305")" ec -tls1_3 -verify 1
    br_tls "a server with only RSA key exchange is refused" \
        "failed: the secure connection to 127.0.0.1 failed: the server refused: no cipher or version in common" rsa \
        -tls1_2 -cipher AES128-GCM-SHA256
    br_tls "a server with only TLS 1.1 is refused" \
        "failed: the secure connection to 127.0.0.1 failed: the server refused: no TLS version in common" rsa \
        -tls1_1 -cipher DEFAULT@SECLEVEL=0

    # the same server, but its certificate is not on the list
    openssl s_server -accept $BR_TLS_PORT -cert "$BR_W/ec.pem" -key "$BR_W/ec.key" -www -quiet >/dev/null 2>&1 &
    br_pid=$!
    sleep 0.5
    assert_eq "a server whose certificate nobody vouches for is refused" \
        "failed: the secure connection to 127.0.0.1 failed: its certificate was issued by localhost, which this browser does not know" \
        "$(br fetch "https://127.0.0.1:$BR_TLS_PORT/")"
    kill $br_pid 2>/dev/null
    wait $br_pid 2>/dev/null
else
    echo "  (no openssl here, or Windows: the handshakes are not tested)"
fi
