# 09 — The HTTP server

`tools/lib/http.omg` is an HTTP/1.1 server written in OMG. It is a
library: a program imports it, gives it routes, and calls `listen()`.

It is one process running one loop. Every socket is non-blocking, and
the loop sleeps until one of them can be read or written, so a slow
client holds up nobody else and a thousand open connections cost a
thousand small records. The same code runs on Linux, macOS and
Windows, interpreted or built.

- [Serving a folder](#serving-a-folder)
- [A server with routes](#a-server-with-routes)
- [The request](#the-request)
- [The response](#the-response)
- [Routes](#routes)
- [Static files](#static-files)
- [Answering later, and in pieces](#answering-later-and-in-pieces)
- [Hooks and errors](#hooks-and-errors)
- [Options](#options)
- [Stopping](#stopping)
- [What it does with the protocol](#what-it-does-with-the-protocol)
- [How fast it is](#how-fast-it-is)
- [The builtins underneath](#the-builtins-underneath)
- [What it does not do](#what-it-does-not-do)

## Serving a folder

```sh
omg tools/site/http_server.omg              # this folder, on http://127.0.0.1:9876
omg tools/site/http_server.omg public 8000  # another folder, another port
```

| Option            | Meaning |
| ----------------- | ------- |
| `--host ADDRESS`  | The address to listen on. The default, `127.0.0.1`, is this machine only; `0.0.0.0` is every network interface. |
| `--list`          | Show a listing for a directory that has no `index.html`. |
| `--cache SECONDS` | Let browsers keep a file this long without asking again. By default they ask every time and get a short "not modified" when nothing changed. |
| `--quiet`         | No access log. |

Ctrl-C stops it, after the requests in flight have been answered.

Built, it is a single file of about 450 KB with nothing to install:

```sh
omg --build tools/site/http_server.omg serve
./serve public 8000
```

## A server with routes

```omg
;;;omg
import "tools/lib/http.omg" as http     # a path from this file to the library

alloc app := http.server({port: 8080})

proc hello(req) {
    return http.text(200, "hello, " + req.params.name + "\n")
}

app.get("/hello/:name", hello)
app.files("/", "public", {})
app.listen()
```

A handler is a proc that takes a request and returns a response.
`app.listen()` runs until the server is stopped.

`tools/site/demo_app.omg` is a longer example: a page, a small
collection kept in memory with `GET` and `POST`, an answer that comes
two seconds later, and one that arrives a line at a time.

Handlers run inside the loop, one at a time. That is what makes the
server simple to program (no two handlers ever touch the same variable
at once), and it has one consequence: a handler that computes for a
second holds up every other connection for that second. One that has
to *wait* for something should
[answer later](#answering-later-and-in-pieces) instead.

## The request

| Field          | Example                 | |
| -------------- | ----------------------- | --- |
| `method`       | `"GET"`                 | |
| `target`       | `"/a%20b?x=1"`          | As the client sent it. |
| `path`         | `"/a b"`                | Decoded, with no `.` or `..` left in it. |
| `query_string` | `"x=1"`                 | |
| `query`        | `{x: "1"}`              | Decoded. A name given twice keeps its last value. |
| `params`       | `{name: "world"}`       | What the route's pattern captured. |
| `headers`      | `{"content-type": "…"}` | Names in lower case. A header sent twice has its values joined with `, ` (`; ` for `Cookie`). |
| `body`         | `[104, 105]`            | A byte list; empty when there is none. |
| `version`      | `"1.1"`                 | Or `"1.0"`. |
| `peer`         | `"127.0.0.1:53012"`     | Who is asking. |

| Helper                          | Gives |
| ------------------------------- | ----- |
| `http.header(req, "User-Agent")`| A header by name, in any case; `""` if it was not sent. |
| `http.body_text(req)`           | The body as a string. |
| `http.form(req)`                | The fields of a posted HTML form, as a dict. |
| `http.cookies(req)`             | The cookies sent, as a dict. |
| `http.get(dict, key, default)`  | `dict[key]`, or `default` when there is no such key. Useful for `req.query` and `req.params`. |

## The response

A response is a dict: `{status: 200, headers: {...}, body: ...}`. The
body is a string, a byte list or a list of bytes. These build the usual ones:

| Builder                                 | Gives |
| --------------------------------------- | ----- |
| `http.text(status, body)`               | `text/plain; charset=utf-8` |
| `http.html(status, body)`               | `text/html; charset=utf-8` |
| `http.json(status, body)`               | `application/json`. `body` is JSON text you have made; this labels it. |
| `http.bytes(status, content_type, body)`| A byte list (or a list of bytes) with the type you name. |
| `http.empty(status)`                    | No body: `204`, say. |
| `http.redirect(status, location)`       | `302` is the usual status. |
| `http.error_page(status, detail)`       | The small page the server itself sends for errors. |
| `http.response(status, headers, body)`  | Exactly what you say. |
| `http.send_file(req, path)`             | A file from disk, with everything [static files](#static-files) get. |

The server adds `Date`, `Server`, `Content-Length` and, when it is
closing the connection, `Connection: close`. To add a header of your
own, put it in `headers`; to send one more than once, give a list:

```omg
return http.response(200, {"Set-Cookie": ["a=1; Path=/", "b=2; Path=/"]}, "ok\n")
```

A header value with a line break in it is refused (the request gets a
500 and the log says why), so text from a request cannot smuggle a
header into a response.

Other helpers: `http.html_escape(text)`, `http.url_decode(text, plus)`,
`http.url_encode_path(text)`, `http.parse_query(text)`,
`http.http_date(ms)` and `http.parse_http_date(text)`.

## Routes

```omg
app.get("/users/:id", show_user)
app.post("/users", add_user)
app.put("/users/:id", replace_user)
app.patch("/users/:id", change_user)
app.delete("/users/:id", remove_user)
app.any("/echo", echo)                    # every method
app.route("REPORT", "/calendar", report)  # a method by name
```

In a pattern, `:name` captures one segment of the path and `*name` at
the end captures all that is left, slashes included (`*` alone calls it
`rest`). Captures arrive decoded in `req.params`. Routes are tried in
the order they were added and the first match answers.

- A path no route matches is a **404**. `app.not_found(handler)`
  replaces the page.
- A path that routes match, but not for this method, is a **405** with
  an `Allow` header listing the methods that would work.
- `HEAD` is answered from the `GET` route, with the same headers and no
  body.
- `OPTIONS` is answered with the `Allow` list, unless a route takes it.

## Static files

```omg
app.files("/assets", "public/assets", {})
app.files("/", "public", {listing: true, cache_control: "max-age=3600"})
```

`app.files(prefix, folder, options)` serves the files in a folder
under a URL prefix.

| Option          | Default        | |
| --------------- | -------------- | --- |
| `index`         | `"index.html"` | Served for a directory. `""` for none. |
| `listing`       | `false`        | List a directory that has no index. |
| `dotfiles`      | `false`        | Serve names that start with a dot. |
| `cache_control` | `"no-cache"`   | The `Cache-Control` header; `""` to send none. |
| `types`         | `{}`           | Content types by extension, added to the built-in table: `{"wasm": "application/wasm"}`. |

What a file gets:

- A `Content-Type` from its extension (the usual ones for the web are
  known; anything else is `application/octet-stream`).
- `ETag` and `Last-Modified`. A browser that already has the file asks
  with `If-None-Match` or `If-Modified-Since` and gets a **304** with no
  body.
- Byte ranges: `Range: bytes=0-99`, `bytes=100-` and `bytes=-50` get a
  **206** with a `Content-Range`, which is what lets a browser seek in a
  video or resume a download. A range past the end is a **416**.
  `If-Range` is honoured.
- A directory asked for without its trailing slash is redirected to
  the path with one, so relative links in its index work.
- A large file is never read into memory whole: it goes out 64 KiB at a
  time, as fast as the client takes it, while other connections are
  served in between.

A path cannot leave the folder. `..` is resolved before the path is
looked at, and a path that would climb above the root is a 400, as is
one with a NUL in it. A backslash or a colon in a name is a 404, so a
path cannot mean something else on Windows than it does elsewhere.

To send one file from a route, with all of the above:

```omg
proc download(req) {
    return http.send_file(req, "reports/" + req.params.name + ".pdf")
}
```

## Answering later, and in pieces

A handler that has nothing to say yet returns `http.pending()` and
keeps the request. Whoever has the answer later calls
`app.respond(req, response)`. In between, the server carries on with
everyone else.

```omg
proc slow(req) {
    proc answer() {
        app.respond(req, http.text(200, "worth the wait\n"))
    }
    app.timer(2000, answer)
    return http.pending()
}
```

| Timers                 | |
| ---------------------- | --- |
| `app.timer(ms, proc)`  | Call `proc()` once, `ms` from now. Returns an id. |
| `app.every(ms, proc)`  | Call it every `ms`. Returns an id. |
| `app.cancel(id)`       | Stop a timer. |

An answer can also go out in pieces, which is how a page shows progress
or a client follows events:

```omg
proc countdown(req) {
    alloc n := 3
    alloc id := 0
    proc tick() {
        app.send(req, n + "\n")
        n := n - 1
        if n == 0 {
            app.cancel(id)
            app.finish(req)
        }
    }
    id := app.every(1000, tick)
    return http.stream(200, {"Content-Type": "text/plain"})
}
```

| Streams                       | |
| ----------------------------- | --- |
| `http.stream(status, headers)`| Returned from a handler (or given to `app.respond`): the head goes out now and the body is whatever follows. |
| `app.send(req, data)`         | One more piece: a string, a byte list or a list of bytes. `false` if the client has gone. |
| `app.finish(req)`             | The end of the answer. |
| `app.backlog(req)`            | Bytes sent but not yet taken by the client. A producer faster than its reader should wait for this to fall. |

`app.respond`, `app.send` and `app.finish` return `false`, and do
nothing, when the request is no longer waiting: the client went away,
or it was answered already, or it waited longer than
`handler_timeout_ms` and was sent a 503.

To run something of your own between requests, drive the loop yourself
instead of calling `listen()`:

```omg
app.start()
loop app.poll(100) {       # serve for up to 100 ms, then come back
    check_the_queue()
}
```

## Hooks and errors

```omg
proc require_key(req) {
    if http.header(req, "X-Api-Key") != "secret" { return http.text(401, "no\n") }
    return false                       # carry on to the routes
}
proc add_header(req, res) {
    res.headers["X-Frame-Options"] := "DENY"
    return res
}
app.before(require_key)
app.after(add_header)
```

- `app.before(proc(req))` runs before the routes. Returning a response
  answers the request there; returning `false` lets it through.
- `app.after(proc(req, res))` sees every response on its way out,
  the server's own error pages included, and returns the one to send.
- A handler that raises an error is a **500**. The error goes to the
  log, never to the client. `app.on_error(proc(req, err))` can return a
  response of its own instead.
- A handler that returns nothing is a 500 too, and the log says which
  route it was.

Nothing a handler does can bring the server down: the next request is
served as usual.

## Options

`http.server({...})` takes any of these. An option it does not know is
an error, so a misspelling is caught at once.

| Option                        | Default       | |
| ----------------------------- | ------------- | --- |
| `host`                        | `"127.0.0.1"` | `"0.0.0.0"` to be reachable from other machines. |
| `port`                        | `8080`        | |
| `max_connections`             | `512`         | Open at once. Further ones wait their turn to be accepted. |
| `max_header_bytes`            | `16384`       | Request line and headers together. Over it: 431. |
| `max_body_bytes`              | `1048576`     | Over it: 413. |
| `max_requests_per_connection` | `1000`        | Then the connection is closed and the client opens another. |
| `idle_timeout_ms`             | `15000`       | A kept-alive connection with no request on it. |
| `header_timeout_ms`           | `10000`       | From a request's first byte to the end of its headers. Over it: 408. |
| `body_timeout_ms`             | `30000`       | To the end of its body. Over it: 408. |
| `write_timeout_ms`            | `30000`       | A client that stops taking the answer. |
| `handler_timeout_ms`          | `60000`       | An `http.pending()` that is never answered. Over it: 503. |
| `shutdown_timeout_ms`         | `10000`       | How long a stop waits for requests in flight. |
| `handle_signals`              | `true`        | Stop cleanly on Ctrl-C and SIGTERM. |
| `server_name`                 | `"omg-http"`  | The `Server` header; `""` to send none. |
| `log`                         | `true`        | `true`: access log on standard output. `false`: none. Or a `proc(line)` of your own. |

The timeouts are what keep a server standing on the open internet: a
client that connects and says nothing, or sends a header a byte a
minute, is dropped instead of holding a place for ever.

The access log is one line a request, in the Common Log Format with the
time taken added:

```
127.0.0.1 - - [13/Nov/2026:02:46:38 +0000] "GET /hello/world HTTP/1.1" 200 13 0ms
```

`app.stats()` gives `{connections, accepted, requests, draining}`.

## Stopping

Ctrl-C, or SIGTERM from a process manager, starts a clean stop: the
server stops accepting, closes idle connections, finishes the requests
in flight (for up to `shutdown_timeout_ms`) and then `app.listen()`
returns, so the program can save what it needs to and end. A second
Ctrl-C stops at once.

`app.stop()` does the same from inside the program. Called from a
handler, that handler's own answer is still sent.

This works the same on Windows, where it is Ctrl-C and Ctrl-Break in
the console.

## What it does with the protocol

- **Keep-alive** by default for HTTP/1.1, on request for HTTP/1.0.
- **Pipelining**: requests sent without waiting are answered in order.
- **Request bodies** by `Content-Length` or chunked, with trailers
  skipped; `Expect: 100-continue` is answered before the body is read.
- **Refusals**: a request it cannot parse is a 400, no `Host` in
  HTTP/1.1 is a 400, two different `Content-Length`s are a 400, a
  transfer coding other than chunked is a 501, an HTTP version other
  than 1.0 and 1.1 is a 505. After a refusal the connection is closed.
- **Leniency** where the standard asks for it: bare line feeds for line
  ends, an absolute URL as the target, empty lines before a request.
- **HTTP/1.0 clients** get a streamed answer without chunking, ended by
  closing the connection.

## How fast it is

`tools/dev/http-bench.omg` is a load generator, also written in OMG and
also one loop around `tcp_poll`:

```sh
omg --build tools/dev/http-bench.omg bench
./bench -c 50 -n 20000 http://127.0.0.1:8080/hello/world
```

| Option       | |
| ------------ | --- |
| `-c N`       | Connections kept busy at once (default 10). |
| `-n N`       | Requests in all (default 1000). |
| `-t SECONDS` | Run for this long instead. |
| `--close`    | A new connection for every request. |
| `--quiet`    | Print one number: requests per second. |

Measured on one core of a cloud machine, server and load generator
sharing it, the server built with `omg --build`:

| What                                              | Requests a second |
| ------------------------------------------------- | ----------------: |
| A route, 50 connections kept alive                | 46,500 |
| A route, 1,000 connections kept alive             | 45,000 |
| A small file, 50 connections kept alive           | 22,500 |
| A route, a new connection for every request       | 26,300 |
| The same, built through C (`--target c`)          | 13,000 |
| The same, interpreted (`omg app.omg`)             | 800 |

A 300 KB file goes out about 10,000 times a second, which is 3 GB a
second: the pieces of a file are byte lists from `file_read` to
`tcp_write`, so sending one is a copy. (With ordinary lists it was
68 MB a second.) Build a server before putting load on it: interpreted
is for trying things.

## The builtins underneath

The server is ordinary OMG. What the language gives it is a small set
of socket builtins, available on every system OMG runs on:

| Builtin                              | |
| ------------------------------------ | --- |
| `tcp_listen(host, port)`             | A listening socket. |
| `tcp_accept(listener)`               | The next connection. |
| `tcp_connect(host, port)`            | A connection to somewhere else. |
| `tcp_read(h, max)`                   | Up to `max` bytes, as a byte list; empty when the other side has closed. |
| `tcp_write(h, data)`                 | Send a string, a byte list or a list of bytes. Returns the number of bytes sent. |
| `tcp_write(h, data, offset)`         | The same, starting `offset` bytes in: for carrying on after a partial write. |
| `tcp_close(h)`                       | |
| `tcp_nonblocking(h, on)`             | With it on, `tcp_accept` and `tcp_read` return `false` instead of waiting when there is nothing, and `tcp_write` sends what fits and says how much that was. |
| `tcp_poll(reads, writes, timeout_ms)`| Wait until a socket in `reads` has something to read (or accept), or one in `writes` has room, or the time runs out. Returns `[readable, writable]`. A negative timeout waits for ever; `0` only looks. |
| `tcp_peer(h)`                        | The other end's address: `"203.0.113.9:53012"`. |
| `interrupted()`                      | How many times the program has been asked to stop (Ctrl-C, SIGTERM) since it first called this. The first call takes those requests over: they no longer end the program, and a `tcp_poll` in progress returns early so the program can act. |
| `file_read(h, max)`                  | The next `max` bytes of an open file, so a large file can be sent a piece at a time. `file_read(h)` still reads all that is left. |
| `file_size(path)`, `file_mtime(path)`| A file's length in bytes, and when it was last changed (milliseconds since 1970). |

A non-blocking client is the same loop pointed the other way;
`tools/dev/http-bench.omg` is one in 400 lines.

These work interpreted and in built executables (machine code and
through C) on Linux, macOS and Windows. In JavaScript output the file
builtins work under Node, and the `tcp_*` ones raise an error when
called.

## What it does not do

- **TLS.** Put it behind a reverse proxy (nginx, Caddy) for HTTPS, as
  is usual for application servers.
- **HTTP/2, WebSockets, compression.** A proxy in front gives the first
  and the last.
- **More than one core.** It is one loop on one thread. To use more
  cores, run several on different ports behind a proxy.
- **JSON.** `http.json` labels text as JSON; building and parsing it is
  up to the program (`tools/json.omg` has a tokenizer to start from).
