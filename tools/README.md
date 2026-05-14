# OMG tools

A bunch of command-line utilities **written in OMG itself**.

Each tool is a standalone `.omg` script in this directory; together they
demonstrate that OMG handles real tasks. They use the language's strings, 
lists, dicts, file I/O (text and binary), imports, closures and more.

## Running

```sh
omg tools/<dir>/<tool>.omg <args...>
```

For example:

```sh
omg tools/omg/unix/wc.omg examples/hello_world.omg examples/prime_sieve.omg
omg tools/omg/unix/grep.omg "proc " examples/higher_order.omg
omg tools/omg/unix/hex.omg bootstrap/src/compiler.omgb | head
```

## Path handling

Relative paths in command-line arguments resolve against your **shell's
current working directory**, the same way `wc`, `cat`, `python`, etc.
behave. The script's location on disk is irrelevant for runtime file I/O.

```sh
$ cd ~
$ omg ~/workspace/omglang/tools/unix/wc.omg notes.txt
   ←     # reads ~/notes.txt
```

(`import` paths still resolve relative to the importing source file —
that's a *compile-time* path concern, not a runtime one.)

## The tools

### Text utilities

| Tool        | Approx. Unix equivalent | What it does |
|-------------|-------------------------|--------------|
| `unix/wc.omg`    | `wc -l -w -m`           | Lines, words, characters per file plus a total. |
| `unix/grep.omg`  | `grep -F -n`            | Print lines (with line numbers, prefixed with path when more than one input) containing a literal substring. |
| `unix/hex.omg`   | `xxd`                   | 16-bytes-per-row hex + ASCII-gutter dump of any binary file. |
| `unix/sort.omg`  | `sort`                  | Concatenate inputs and print all lines sorted lexicographically (insertion sort, in place). |
| `unix/uniq.omg`  | `uniq`                  | Drop adjacent duplicate lines. Pair with `sort` for full dedup. |
| `unix/head.omg`  | `head -n N`             | First `N` lines (default 10) of each input; multi-file output uses `==> path <==` banners. |
| `unix/tail.omg`  | `tail -n N`             | Last `N` lines (default 10) of each input; same multi-file banners. |

### Format and conversion

| Tool          | Approx. Unix equivalent | What it does |
|---------------|-------------------------|--------------|
| `tmpl.omg`    | `envsubst` / `m4 -P`    | Render a `{{name}}` template using values from a `key=value` data file. |
| `base64.omg`  | `base64`                | Encode binary files to base64 text, or decode base64 text to binary. Streams output to a file in decode mode (no in-memory byte list). |
| `json.omg`    | `jq -c` / `jq .`        | Pretty-print or `--minify` a JSON file. Operates on the token stream, so it preserves number formats (including floats and exponents) without needing OMG-native floats. |
| `md2html.omg` | `pandoc -f md -t html`  | Convert a useful subset of Markdown to HTML: ATX headers, paragraphs, ` ``` ` fenced code blocks, `**bold**` / `*italic*` / `` `code` ``, `[text](url)` links, ordered/unordered lists, horizontal rules. HTML-escapes `< > & "`. |

### OMG dev tools (dogfooding)

| Tool             | What it does |
|------------------|--------------|
| `omg/omg-fmt.omg`    | Minimal formatter: re-indents every line using 4-space units sized by accumulated `{ } ( ) [ ]` depth, strips trailing whitespace, preserves blank lines and the contents of strings/docblocks. Idempotent. |
| `omg/omg-bundle.omg` | Inlines `import` statements into a single self-contained `.omg` file. Top-level names from each imported module are mangled to `__alias__name` and exposed via a `freeze({ ... })` namespace dict; calls like `math.is_prime(97)` continue to work because the dict holds first-class closures. |
| `omg/omg-deps.omg`   | Prints the import graph of an OMG program as an indented tree. Detects cycles, marks unreadable files. |

### Advanced

Larger, demo-quality programs that compose the smaller tools.

| Tool                  | What it does |
|-----------------------|--------------|
| `web/ssg.omg`    | Static site generator. Walks `<site>/content/`, parses front-matter, converts each markdown file via `md2html.omg`, applies `<site>/templates/*.html`, and writes a parallel directory tree to `<site>/out/`. See [`examples/omg-ssg-site/README.md`](../examples/omg-ssg-site/README.md) for the layout, template variables, and an example site. |
| `db/omgdb.omg`   | A small SQL database written in OMG — a 4-KB paged on-disk format, a recursive-descent SQL parser (`CREATE` / `INSERT` / `SELECT` / `DELETE` / `DROP` with `WHERE` and `ORDER BY`), and a SQLite-style REPL with one-shot `-e` and stdin pipeline modes. See [`db/README.md`](db/README.md). |
| `edit.omg`       | **OMG edit** — a nano-shaped terminal editor with a sidebar file browser. Open a file directly or hand it a directory and it drops you into the sidebar to pick one. Editor side: scrollable buffer, save / quit, cut / paste lines (kill-chain semantics), forward search, go-to-line, syntax highlighting for `.omg`. Sidebar side: arrow / vi-style navigation, Enter to descend or open, Backspace for parent dir. `^B` toggles sidebar visibility, `^T` swaps focus between panes. Terminal size detected via `stty size`. AOT-compile for crispest input. |

### Shared helpers

`modules/lib.omg` collects helpers shared between the tools:

- `split_lines(text)`, `contains(haystack, needle)`
- `sort_strings(xs)` — in-place insertion sort
- `trim(s)`, `starts_with(s, prefix)`
- `hex_pad(n, width)`, `printable(byte)`, `parse_int(s)`
- `is_ident_start(c)`, `is_ident_char(c)` — used by `omg-fmt`/`omg-bundle`
- `path_split(p)` — split into `[dir, base]`
- `list_contains(xs, word)` — exact membership

Each tool imports it as `lib`.

## Verifying parity

These tools should match their Unix counterparts on simple inputs:

```sh
$ omg tools/unix/wc.omg examples/maze_solver.omg | awk '{print $1, $2, $3}'
105 378 2348
$ wc -l -w -m examples/maze_solver.omg | awk '{print $1, $2, $3}'
105 378 2348
```

```sh
$ printf 'ccc\naaa\nbbb\naaa\n' > /tmp/x && omg tools/unix/sort.omg /tmp/x
aaa
aaa
bbb
ccc
$ sort /tmp/x
aaa
aaa
bbb
ccc
```

`base64.omg` round-trips byte-for-byte:

```sh
$ omg tools/base64.omg encode bootstrap/src/compiler.omgb /tmp/c.b64
$ omg tools/base64.omg decode /tmp/c.b64 /tmp/c.bin
$ cmp bootstrap/src/compiler.omgb /tmp/c.bin && echo OK
OK
```

`omg-bundle.omg` produces a single-file program with identical behaviour:

```sh
$ omg tools/omg/omg-bundle.omg examples/import_modules.omg /tmp/bundled.omg
$ diff <(omg /tmp/bundled.omg) <(omg examples/import_modules.omg) && echo OK
OK
```

## Performance notes

These are demos, not optimised utilities. OMG strings are immutable and
list `+` allocates a new list, so anything that builds output one
character at a time is `O(n²)`:

- All tools handle text files up to a few thousand lines comfortably.
- `sort.omg` uses in-place insertion sort: `O(N²)` swaps with `O(L)`
  comparisons. Quick for hundreds of lines, slow for tens of thousands.
- `hex.omg` allocates a `string + char` per byte. Fine for files up to a
  few hundred KB; large binaries will grind.
- `base64.omg` decode mode streams bytes straight to the output file
  (3 bytes per `file_write` call), so it scales to MB-sized files; encode
  mode builds the full base64 string in memory and is `O(n²)` in input
  size — good up to a few hundred KB.
- `md2html.omg` and `json.omg` keep the rendered output in a single
  growing string. Fine for typical README/config-sized files; will get
  slow over a few hundred KB.
- `omg-fmt.omg` and `omg-bundle.omg` keep their output in a single growing
  string (same caveat as above) and process source files line-by-line, so
  performance scales linearly with line count for typical programs.

If/when the runtime grows mutable string builders or persistent
collections, these tools should pick up the speedup automatically — the
algorithms are right, it's the data structures that are pricey.
