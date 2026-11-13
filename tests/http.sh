#!/bin/bash
# The HTTP server library, tools/lib/http.omg.
#
# tests/http/app.omg is an application with a route for each thing the
# library can do. It is started here as a built executable (twice: once
# with ordinary limits, once with limits small enough to run into) and
# once interpreted, and is asked things with curl and, where curl is
# too polite, with tests/http/client.omg, which sends exactly the
# bytes it is told to.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

cd "$REPO_ROOT"

HTTP_W="$TMPDIR_TEST/http"
HTTP_PORT=19878
HTTP_STRICT_PORT=19879
HTTP_INTERP_PORT=19880
HTTP_PID=""

# --- helpers ---------------------------------------------------------------

# The body of an answer.
http_body() { curl -s -m 15 "$@" 2>/dev/null | tr -d '\r'; }
# Its status code.
http_code() { curl -s -m 15 -o /dev/null -w '%{http_code}' "$@" 2>/dev/null; }
# Its status line and headers.
http_head() { curl -s -m 15 -D - -o /dev/null "$@" 2>/dev/null | tr -d '\r'; }
# One header's value out of what http_head printed.
http_field() { printf '%s\n' "$2" | grep -i "^$1:" | head -n 1 | sed 's/^[^:]*: *//'; }

# `http_raw PORT [NAME]`: say the script on standard input to the
# server, and print what it said back.
http_raw() {
    local script="$HTTP_W/${2:-script}.txt"
    cat > "$script"
    "$HTTP_W/client" "$1" "$script" 2>&1 | tr -d '\r'
}

# `http_start LOG PORT MODE command...`: start an app and wait until it
# answers. Leaves its process id in HTTP_PID.
http_start() {
    local log="$1" port="$2" mode="$3"
    shift 3
    if [ -n "$mode" ]; then
        "$@" "$port" "$HTTP_W/www" "$mode" > "$log" 2>&1 &
    else
        "$@" "$port" "$HTTP_W/www" > "$log" 2>&1 &
    fi
    HTTP_PID=$!
    local tries=0
    while [ $tries -lt 75 ]; do
        [ "$(http_code "http://127.0.0.1:$port/hello/ready")" = "200" ] && return 0
        sleep 0.2
        tries=$((tries + 1))
    done
    return 1
}

# `http_wait_exit PID SECONDS`: leaves in HTTP_RC the exit status once
# it has gone, or "running" if it has not within SECONDS. (Not to be
# called in $(...): only the shell that started a process can wait for
# it.)
http_wait_exit() {
    local pid="$1" tries=$(($2 * 5))
    HTTP_RC=running
    while [ $tries -gt 0 ]; do
        if ! kill -0 "$pid" 2>/dev/null; then
            wait "$pid" 2>/dev/null
            HTTP_RC=$?
            return
        fi
        sleep 0.2
        tries=$((tries - 1))
    done
}

http_stop() {
    [ -n "$HTTP_PID" ] || return 0
    kill "$HTTP_PID" 2>/dev/null
    wait "$HTTP_PID" 2>/dev/null
    HTTP_PID=""
}

# --- what is asked of every app, interpreted or built ----------------------

http_common_checks() {
    local label="$1" base="http://127.0.0.1:$2" port="$2" out

    assert_eq "a route with a parameter ($label)" "hello, world" "$(http_body "$base/hello/world")"
    assert_eq "a static file ($label)" "hello from omg" "$(http_body "$base/static/hello.txt")"
    curl -s -m 15 --data-binary "@$HTTP_W/blob.bin" "$base/echo" -o "$HTTP_W/echo.out" 2>/dev/null
    if cmp -s "$HTTP_W/blob.bin" "$HTTP_W/echo.out"; then
        pass "a posted body comes back byte for byte ($label)"
    else
        fail "a posted body comes back byte for byte ($label)"
    fi
    out=$(http_raw "$port" <<'EOF'
send GET /hello/one HTTP/1.1\r\nHost: t\r\n\r\nGET /hello/two HTTP/1.1\r\nHost: t\r\n\r\nGET /hello/three HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n
recv 3000
EOF
)
    assert_eq "three requests sent at once are answered in order ($label)" \
        "hello, one hello, two hello, three [closed]" \
        "$(printf '%s\n' "$out" | grep -E '^hello|^\[closed' | tr '\n' ' ' | sed 's/ $//')"
}

# --- the suite -------------------------------------------------------------

http_suite() {
    local base="http://127.0.0.1:$HTTP_PORT" strict="http://127.0.0.1:$HTTP_STRICT_PORT"
    local out head code etag modified i ok started took pids

    mkdir -p "$HTTP_W/www/sub" "$HTTP_W/www/dir"
    echo "hello from omg" > "$HTTP_W/www/hello.txt"
    echo "<h1>index of sub</h1>" > "$HTTP_W/www/sub/index.html"
    echo "a" > "$HTTP_W/www/dir/a.txt"
    echo "b" > "$HTTP_W/www/dir/b & c.txt"
    echo "secret" > "$HTTP_W/www/.secret"
    echo "body { color: red }" > "$HTTP_W/www/style.css"
    head -c 1048576 /dev/urandom > "$HTTP_W/www/big.bin"
    head -c 100000 /dev/urandom > "$HTTP_W/blob.bin"

    section "HTTP: the test programs build"
    "$OMG_NATIVE" --build tests/http/app.omg "$HTTP_W/app" >"$HTTP_W/build.log" 2>&1
    "$OMG_NATIVE" --build tests/http/client.omg "$HTTP_W/client" >>"$HTTP_W/build.log" 2>&1
    if is_executable "$HTTP_W/app" && is_executable "$HTTP_W/client"; then
        pass "tests/http/app.omg and client.omg build"
    else
        fail "tests/http/app.omg and client.omg build" "$(cat "$HTTP_W/build.log")"
        return
    fi

    if ! http_start "$HTTP_W/app.log" "$HTTP_PORT" "" "$HTTP_W/app"; then
        fail "the built app starts" "$(cat "$HTTP_W/app.log")"
        http_stop
        return
    fi
    pass "the built app starts"
    local main_pid=$HTTP_PID

    # === Routing ==========================================================
    section "HTTP: routing"
    http_common_checks "built" "$HTTP_PORT"
    assert_eq "a parameter is URL-decoded" "hello, a b" "$(http_body "$base/hello/a%20b")"
    assert_eq "the query string is decoded" "a=1 2 b=é path=/query" "$(http_body "$base/query?a=1+2&b=%C3%A9")"
    assert_eq "no such route is 404" "404" "$(http_code "$base/nope")"
    head=$(http_head -X POST "$base/hello/x")
    assert_contains "the wrong method is 405" "HTTP/1.1 405" "$head"
    assert_eq "405 says which methods are allowed" "GET, HEAD, OPTIONS" "$(http_field Allow "$head")"
    head=$(curl -s -m 15 -I "$base/hello/world" 2>/dev/null | tr -d '\r')
    assert_contains "HEAD is answered from the GET route" "HTTP/1.1 200" "$head"
    assert_eq "HEAD gives the length the body would have" "13" "$(http_field Content-Length "$head")"
    assert_eq "HEAD sends no body" "" "$(curl -s -m 15 -I "$base/hello/world" -o /dev/null -w '%{size_download}' 2>/dev/null | sed 's/^0$//')"
    head=$(http_head "$base/moved")
    assert_contains "a redirect has its status" "HTTP/1.1 302" "$head"
    assert_eq "a redirect has its Location" "/hello/there" "$(http_field Location "$head")"
    head=$(http_head -X OPTIONS "$base/hello/x")
    assert_eq "OPTIONS lists the methods of a path" "GET, HEAD, OPTIONS" "$(http_field Allow "$head")"

    # === Requests and responses ===========================================
    section "HTTP: requests and responses"
    assert_eq "a body sent in chunks is put back together" "100000" \
        "$(http_body -H 'Transfer-Encoding: chunked' --data-binary "@$HTTP_W/blob.bin" "$base/size")"
    assert_eq "PUT reaches its route" "abc" "$(http_body -X PUT --data-binary 'abc' "$base/echo")"
    assert_eq "a posted form is decoded" "name=Chris R msg=a&b" "$(http_body -d 'name=Chris+R&msg=a%26b' "$base/form")"
    assert_eq "cookies are parsed" "sid=abc theme=dark" "$(http_body -H 'Cookie: sid=abc; theme=dark' "$base/cookies")"
    head=$(http_head "$base/headers")
    assert_eq "a header given as a list is sent once per value" "2" "$(printf '%s\n' "$head" | grep -ci '^set-cookie:')"
    assert_eq "a handler's own header is sent" "yes" "$(http_field X-Custom "$head")"
    assert_eq "an after hook sees the response" "after" "$(http_field X-Stamp "$head")"
    assert_contains "every answer has a Date" "GMT" "$(http_field Date "$head")"
    assert_eq "a before hook can answer first" "403 blocked" \
        "$(http_code -H 'X-Block: 1' "$base/hello/x") $(http_body -H 'X-Block: 1' "$base/hello/x")"
    assert_eq "a handler that fails is a 500" "500" "$(http_code "$base/boom")"
    assert_eq "a handler that returns nothing is a 500" "500" "$(http_code "$base/none")"
    assert_eq "a header value with a line break in it is refused" "500" "$(http_code "$base/bad-header")"
    assert_eq "and the server is still answering" "hello, still" "$(http_body "$base/hello/still")"
    head=$(http_head "$base/utf8")
    assert_eq "Content-Length counts bytes, not characters" "11" "$(http_field Content-Length "$head")"
    assert_eq "non-ASCII text arrives whole" "héllo ☕" "$(http_body "$base/utf8")"
    assert_eq "a large text body arrives whole" "300000" "$(curl -s -m 15 "$base/big/text/300000" 2>/dev/null | wc -c | tr -d ' ')"
    assert_eq "a large byte body arrives whole" "200000" "$(curl -s -m 15 "$base/big/bytes/200000" 2>/dev/null | wc -c | tr -d ' ')"

    # === Many connections at once =========================================
    section "HTTP: many connections at once"
    took=$(curl -s -m 15 -o /dev/null -w '%{time_total}' "$base/later/300" 2>/dev/null)
    case "$took" in
        0.[3-9]*|1.*) pass "an answer given later arrives later" ;;
        *) fail "an answer given later arrives later" "took: $took s" ;;
    esac
    # Twenty requests that each take 800 ms. One at a time that is 16
    # seconds; a server that holds them all at once needs about one.
    started=$SECONDS
    pids=""
    for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
        curl -s -m 15 "$base/later/800" > "$HTTP_W/later.$i" 2>/dev/null &
        pids="$pids $!"
    done
    # A request with nothing to wait for is not kept waiting by them.
    assert_eq "a quick request is not held up by slow ones" "hello, quick" "$(http_body -m 2 "$base/hello/quick")"
    ok=0
    wait $pids 2>/dev/null
    took=$((SECONDS - started))
    for i in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
        [ "$(tr -d '\r' < "$HTTP_W/later.$i")" = "after 800" ] && ok=$((ok + 1))
    done
    assert_eq "twenty slow requests at once are all answered" "20" "$ok"
    if [ "$took" -le 10 ]; then
        pass "and take the time of one, not of twenty"
    else
        fail "and take the time of one, not of twenty" "took: $took s"
    fi
    # A client that dawdles over its request head.
    http_raw "$HTTP_PORT" slow > "$HTTP_W/slow.out" <<'EOF' &
send GET /hello/slow HTTP/1.1\r\n
pause 800
send Host: t\r\nConnection: close\r\n\r\n
recv 2000
EOF
    pids=$!
    sleep 0.2
    assert_eq "a slow client does not hold up the others" "hello, fast" "$(http_body -m 2 "$base/hello/fast")"
    wait $pids 2>/dev/null
    assert_contains "and is answered itself when it finishes asking" "hello, slow" "$(cat "$HTTP_W/slow.out")"
    head=$(http_head "$base/ticks")
    assert_eq "a streamed answer is sent in chunks" "chunked" "$(http_field Transfer-Encoding "$head")"
    assert_eq "a streamed answer arrives in order" "tick 1 tick 2 tick 3" "$(http_body "$base/ticks" | tr '\n' ' ' | sed 's/ $//')"

    # === Static files ====================================================
    section "HTTP: static files"
    head=$(http_head "$base/static/hello.txt")
    assert_eq "a text file has its type" "text/plain; charset=utf-8" "$(http_field Content-Type "$head")"
    assert_eq "and its length" "15" "$(http_field Content-Length "$head")"
    assert_eq "and says ranges are welcome" "bytes" "$(http_field Accept-Ranges "$head")"
    assert_eq "a stylesheet has its type" "text/css; charset=utf-8" "$(http_field Content-Type "$(http_head "$base/static/style.css")")"
    assert_eq "a directory serves its index.html" "<h1>index of sub</h1>" "$(http_body "$base/static/sub/")"
    head=$(http_head "$base/static/sub")
    assert_contains "a directory without its slash is redirected" "HTTP/1.1 301" "$head"
    assert_eq "to the same path with one" "/static/sub/" "$(http_field Location "$head")"
    out=$(http_body "$base/static/dir/")
    assert_contains "a directory without an index is listed" 'href="a.txt"' "$out"
    assert_contains "names in a listing are escaped" 'b &amp; c.txt' "$out"
    assert_eq "a file with a space in its name is found" "b" "$(http_body "$base/static/dir/b%20%26%20c.txt")"
    assert_eq "a missing file is 404" "404" "$(http_code "$base/static/missing.txt")"
    assert_eq "a dotfile is not served" "404" "$(http_code "$base/static/.secret")"
    code=$(http_code --path-as-is "$base/static/../app.omg")
    case "$code" in
        400|404) pass "a path cannot climb out of the folder" ;;
        *) fail "a path cannot climb out of the folder" "status: $code" ;;
    esac
    code=$(http_code --path-as-is "$base/static/%2e%2e/%2e%2e/etc/passwd")
    case "$code" in
        400|404) pass "nor with the dots encoded" ;;
        *) fail "nor with the dots encoded" "status: $code" ;;
    esac
    assert_eq "POST to a file is 405" "405" "$(http_code -X POST "$base/static/hello.txt")"
    assert_eq "a route can send a file" "hello from omg" "$(http_body "$base/one-file")"

    head=$(http_head "$base/static/hello.txt")
    etag=$(http_field ETag "$head")
    modified=$(http_field Last-Modified "$head")
    assert_contains "a file has an ETag" '"' "$etag"
    assert_eq "If-None-Match with that ETag is 304" "304" "$(http_code -H "If-None-Match: $etag" "$base/static/hello.txt")"
    assert_eq "with another ETag it is sent again" "200" "$(http_code -H 'If-None-Match: "other"' "$base/static/hello.txt")"
    assert_eq "If-Modified-Since its own date is 304" "304" "$(http_code -H "If-Modified-Since: $modified" "$base/static/hello.txt")"
    assert_eq "If-Modified-Since long ago is sent again" "200" \
        "$(http_code -H 'If-Modified-Since: Sun, 06 Nov 1994 08:49:37 GMT' "$base/static/hello.txt")"

    head=$(http_head -H 'Range: bytes=0-4' "$base/static/hello.txt")
    assert_contains "a range is 206" "HTTP/1.1 206" "$head"
    assert_eq "and says which bytes it is" "bytes 0-4/15" "$(http_field Content-Range "$head")"
    assert_eq "and is those bytes" "hello" "$(http_body -H 'Range: bytes=0-4' "$base/static/hello.txt")"
    assert_eq "a range from the end" "omg" "$(http_body -H 'Range: bytes=-4' "$base/static/hello.txt")"
    assert_eq "a range to the end" "from omg" "$(http_body -H 'Range: bytes=6-' "$base/static/hello.txt")"
    head=$(http_head -H 'Range: bytes=100-' "$base/static/hello.txt")
    assert_contains "a range past the end is 416" "HTTP/1.1 416" "$head"
    assert_eq "and says how long the file is" "bytes */15" "$(http_field Content-Range "$head")"
    assert_eq "If-Range with a stale tag sends the whole file" "200" \
        "$(http_code -H 'Range: bytes=0-4' -H 'If-Range: "stale"' "$base/static/hello.txt")"

    curl -s -m 30 "$base/static/big.bin" -o "$HTTP_W/big.out" 2>/dev/null
    if cmp -s "$HTTP_W/www/big.bin" "$HTTP_W/big.out"; then
        pass "a megabyte file arrives byte for byte"
    else
        fail "a megabyte file arrives byte for byte"
    fi
    # Files go out 64 KiB at a time; this range straddles the first seam.
    curl -s -m 15 -H 'Range: bytes=65530-65545' "$base/static/big.bin" -o "$HTTP_W/range.out" 2>/dev/null
    tail -c +65531 "$HTTP_W/www/big.bin" | head -c 16 > "$HTTP_W/range.want"
    if cmp -s "$HTTP_W/range.want" "$HTTP_W/range.out"; then
        pass "a range in the middle of a large file is the right bytes"
    else
        fail "a range in the middle of a large file is the right bytes"
    fi

    # === Exactly what goes over the wire ==================================
    section "HTTP: the protocol, byte by byte"
    out=$(http_raw "$HTTP_PORT" <<'EOF'
send GET /hello/first HTTP/1.1\r\nHost: t\r\n\r\n
recv 300
send GET /hello/second HTTP/1.1\r\nHost: t\r\n\r\n
recv 300
EOF
)
    assert_eq "a connection is kept for the next request" "hello, first hello, second" \
        "$(printf '%s\n' "$out" | grep -E '^hello|^\[closed' | tr '\n' ' ' | sed 's/ $//')"
    out=$(http_raw "$HTTP_PORT" <<'EOF'
send GET /hello/old HTTP/1.0\r\n\r\n
recv 2000
EOF
)
    assert_contains "HTTP/1.0 is answered" "hello, old" "$out"
    assert_contains "and the connection is closed after it" "[closed]" "$out"
    out=$(http_raw "$HTTP_PORT" <<'EOF'
send GET /hello/bare HTTP/1.1\nHost: t\nConnection: close\n\n
recv 2000
EOF
)
    assert_contains "bare line feeds are accepted" "hello, bare" "$out"
    out=$(http_raw "$HTTP_PORT" <<'EOF'
send THIS IS NOT HTTP\r\n\r\n
recv 2000
EOF
)
    assert_contains "nonsense is 400" "HTTP/1.1 400" "$out"
    assert_contains "and the connection is closed" "[closed]" "$out"
    out=$(http_raw "$HTTP_PORT" <<'EOF'
send GET /hello/x HTTP/1.1\r\n\r\n
recv 2000
EOF
)
    assert_contains "HTTP/1.1 without a Host is 400" "HTTP/1.1 400" "$out"
    out=$(http_raw "$HTTP_PORT" <<'EOF'
send GET /hello/x HTTP/2.0\r\nHost: t\r\n\r\n
recv 2000
EOF
)
    assert_contains "a version it does not speak is 505" "HTTP/1.1 505" "$out"
    out=$(http_raw "$HTTP_PORT" <<'EOF'
send POST /echo HTTP/1.1\r\nHost: t\r\nContent-Length: 3\r\nContent-Length: 4\r\n\r\nabcd
recv 2000
EOF
)
    assert_contains "two different Content-Lengths are 400" "HTTP/1.1 400" "$out"
    out=$(http_raw "$HTTP_PORT" <<'EOF'
send POST /echo HTTP/1.1\r\nHost: t\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nhel
pause 100
send lo\r\n6;note=x\r\n world\r\n0\r\nTrailer: yes\r\n\r\n
recv 2000
EOF
)
    assert_contains "a chunked body split across packets is put back together" "hello world" "$out"
    out=$(http_raw "$HTTP_PORT" <<'EOF'
send POST /echo HTTP/1.1\r\nHost: t\r\nContent-Length: 5\r\nExpect: 100-continue\r\nConnection: close\r\n\r\n
recv 300
send hello
recv 2000
EOF
)
    assert_contains "Expect: 100-continue is told to go on" "HTTP/1.1 100 Continue" "$out"
    assert_contains "and the body is then read" "hello" "$(printf '%s\n' "$out" | tail -n 2)"
    assert_contains "requests are written to the access log" '"GET /hello/world HTTP/1.1" 200 13' "$(cat "$HTTP_W/app.log")"

    # === Limits and timeouts ==============================================
    section "HTTP: limits and timeouts"
    if ! http_start "$HTTP_W/strict.log" "$HTTP_STRICT_PORT" strict "$HTTP_W/app"; then
        fail "the app starts with small limits" "$(cat "$HTTP_W/strict.log")"
    else
        local strict_pid=$HTTP_PID
        head -c 5000 /dev/zero > "$HTTP_W/zeros.bin"
        assert_eq "a body over the limit is 413" "413" "$(http_code --data-binary "@$HTTP_W/zeros.bin" "$strict/size")"
        assert_eq "a body under it is taken" "4000" "$(head -c 4000 /dev/zero | http_body --data-binary @- "$strict/size")"
        assert_eq "a chunked body over the limit is 413" "413" \
            "$(http_code -H 'Transfer-Encoding: chunked' --data-binary "@$HTTP_W/zeros.bin" "$strict/size")"
        assert_eq "a request head over the limit is 431" "431" \
            "$(http_code -H "X-Long: $(head -c 3000 /dev/zero | tr '\0' 'x')" "$strict/hello/x")"
        out=$(http_raw "$HTTP_STRICT_PORT" <<'EOF'
send GET /hello/idle HTTP/1.1\r\nHost: t\r\n\r\n
recv 3000
EOF
)
        assert_contains "an idle connection is answered" "hello, idle" "$out"
        assert_contains "and then closed when nothing more comes" "[closed]" "$out"
        out=$(http_raw "$HTTP_STRICT_PORT" <<'EOF'
send GET /hello/half HTTP/1.1\r\nHost: t\r\n
recv 3000
EOF
)
        assert_contains "a request head that never finishes is 408" "HTTP/1.1 408" "$out"
        assert_contains "and the connection is closed" "[closed]" "$out"
        out=$(http_raw "$HTTP_STRICT_PORT" <<'EOF'
send POST /size HTTP/1.1\r\nHost: t\r\nContent-Length: 10\r\n\r\nabc
recv 3000
EOF
)
        assert_contains "a body that never finishes is 408" "HTTP/1.1 408" "$out"
        code=$(http_code "$strict/never")
        assert_eq "a request the handler never answers is 503" "503" "$code"
        out=$(http_raw "$HTTP_STRICT_PORT" <<'EOF'
send GET /hello/1 HTTP/1.1\r\nHost: t\r\n\r\nGET /hello/2 HTTP/1.1\r\nHost: t\r\n\r\nGET /hello/3 HTTP/1.1\r\nHost: t\r\n\r\nGET /hello/4 HTTP/1.1\r\nHost: t\r\n\r\n
recv 3000
EOF
)
        assert_eq "a connection is closed after its allowance of requests" "hello, 1 hello, 2 hello, 3 [closed]" \
            "$(printf '%s\n' "$out" | grep -E '^hello|^\[closed' | tr '\n' ' ' | sed 's/ $//')"
        assert_eq "and the last answer says so" "1" "$(printf '%s\n' "$out" | grep -ci '^connection: close')"

        # === Stopping on a signal =========================================
        # Not on Windows: Git Bash's kill ends a native program outright.
        if [ -z "$OMG_WINDOWS" ]; then
            section "HTTP: stopping on a signal"
            curl -s -m 15 "$strict/later/400" > "$HTTP_W/inflight.out" 2>/dev/null &
            local curl_pid=$!
            sleep 0.15
            kill -TERM "$strict_pid" 2>/dev/null
            wait "$curl_pid" 2>/dev/null
            assert_eq "a request in flight is finished first" "after 400" "$(tr -d '\r' < "$HTTP_W/inflight.out")"
            http_wait_exit "$strict_pid" 10
            assert_eq "then the server stops by itself" "0" "$HTTP_RC"
            assert_contains "and listen() returns to the program" "stopped" "$(tail -n 1 "$HTTP_W/strict.log")"
            assert_eq "new connections are refused once it has" "000" "$(http_code -m 2 "$strict/hello/x")"
        else
            kill "$strict_pid" 2>/dev/null
            wait "$strict_pid" 2>/dev/null
        fi
        HTTP_PID=""
    fi

    # === Stopping from inside =============================================
    section "HTTP: stopping"
    assert_eq "the answer to the request that stops it is still sent" "bye" "$(http_body -X POST "$base/quit")"
    http_wait_exit "$main_pid" 10
    assert_eq "stop() ends the server" "0" "$HTTP_RC"
    assert_contains "and listen() returns to the program" "stopped" "$(tail -n 1 "$HTTP_W/app.log")"

    # === Interpreted ======================================================
    section "HTTP: the same app, interpreted"
    if ! http_start "$HTTP_W/interp.log" "$HTTP_INTERP_PORT" "" "$OMG_NATIVE" tests/http/app.omg; then
        fail "the interpreted app starts" "$(cat "$HTTP_W/interp.log")"
        http_stop
        return
    fi
    local interp_pid=$HTTP_PID
    http_common_checks "interpreted" "$HTTP_INTERP_PORT"
    assert_eq "a range (interpreted)" "hello" "$(http_body -H 'Range: bytes=0-4' "http://127.0.0.1:$HTTP_INTERP_PORT/static/hello.txt")"
    assert_eq "an answer given later (interpreted)" "after 100" "$(http_body "http://127.0.0.1:$HTTP_INTERP_PORT/later/100")"
    assert_eq "a streamed answer (interpreted)" "tick 1 tick 2 tick 3" \
        "$(http_body "http://127.0.0.1:$HTTP_INTERP_PORT/ticks" | tr '\n' ' ' | sed 's/ $//')"
    assert_eq "stop() (interpreted)" "bye" "$(http_body -X POST "http://127.0.0.1:$HTTP_INTERP_PORT/quit")"
    http_wait_exit "$interp_pid" 10
    assert_eq "the interpreted app ends" "0" "$HTTP_RC"
    HTTP_PID=""
}

if command -v curl >/dev/null 2>&1; then
    http_suite
    http_stop
else
    section "HTTP"
    echo "  (skipped: curl not installed)"
fi
