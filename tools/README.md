# OMG tools

Small Unix-style command-line utilities **written in OMG itself**.

Each tool is a standalone `.omg` script in this directory; together they
demonstrate that OMG handles real text processing — not just toy examples.
They use the language's strings, lists, dicts, file I/O (text and binary),
imports, and closures.

## Running

```sh
omg tools/<tool>.omg <args...>
```

For example:

```sh
omg tools/wc.omg examples/hello_world.omg examples/prime_sieve.omg
omg tools/grep.omg "proc " examples/higher_order.omg
omg tools/hex.omg bootstrap/compiler.omgb | head
```

## Path handling

Relative paths in command-line arguments are resolved against the **tool's
directory** (`tools/`), not your shell's current working directory. The
runtime sets `current_dir` based on the script being run, and `read_file`
/ `file_open` honour it. In practice that means:

- Use absolute paths for files outside `tools/`:
  `omg tools/wc.omg /etc/hosts`
- Or paths relative to the tool's location:
  `omg tools/wc.omg ../examples/maze_solver.omg`

## The tools

### Text utilities

| Tool        | Approx. Unix equivalent | What it does |
|-------------|-------------------------|--------------|
| `wc.omg`    | `wc -l -w -m`           | Lines, words, characters per file plus a total. |
| `grep.omg`  | `grep -F -n`            | Print lines (with line numbers, prefixed with path when more than one input) containing a literal substring. |
| `hex.omg`   | `xxd`                   | 16-bytes-per-row hex + ASCII-gutter dump of any binary file. |
| `sort.omg`  | `sort`                  | Concatenate inputs and print all lines sorted lexicographically (insertion sort, in place). |
| `uniq.omg`  | `uniq`                  | Drop adjacent duplicate lines. Pair with `sort` for full dedup. |
| `head.omg`  | `head -n N`             | First `N` lines (default 10) of each input; multi-file output uses `==> path <==` banners. |
| `tail.omg`  | `tail -n N`             | Last `N` lines (default 10) of each input; same multi-file banners. |

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
| `omg-fmt.omg`    | Minimal formatter: re-indents every line using 4-space units sized by accumulated `{ } ( ) [ ]` depth, strips trailing whitespace, preserves blank lines and the contents of strings/docblocks. Idempotent. |
| `omg-bundle.omg` | Inlines `import` statements into a single self-contained `.omg` file. Top-level names from each imported module are mangled to `__alias__name` and exposed via a `freeze({ ... })` namespace dict; calls like `math.is_prime(97)` continue to work because the dict holds first-class closures. |
| `omg-deps.omg`   | Prints the import graph of an OMG program as an indented tree. Detects cycles, marks unreadable files. |
| `omg-test.omg`   | Runs each test file through the OMG-in-OMG tree-walk interpreter at `reference/interpreter.omg` and reports pass/fail. A test passes if it executes to completion without raising; `facts`/`panic`/runtime errors count as failures. Exits non-zero on any failure. |

### Shared helpers

`lib.omg` collects helpers shared between the tools:

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
$ omg tools/wc.omg examples/maze_solver.omg | awk '{print $1, $2, $3}'
105 378 2348
$ wc -l -w -m examples/maze_solver.omg | awk '{print $1, $2, $3}'
105 378 2348
```

```sh
$ printf 'ccc\naaa\nbbb\naaa\n' > /tmp/x && omg tools/sort.omg /tmp/x
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
$ omg tools/base64.omg encode bootstrap/compiler.omgb /tmp/c.b64
$ omg tools/base64.omg decode /tmp/c.b64 /tmp/c.bin
$ cmp bootstrap/compiler.omgb /tmp/c.bin && echo OK
OK
```

`omg-bundle.omg` produces a single-file program with identical behaviour:

```sh
$ omg tools/omg-bundle.omg examples/import_modules.omg /tmp/bundled.omg
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
- `omg-test.omg` runs each test through the **tree-walk** interpreter at
  `reference/interpreter.omg`, which is dramatically slower than the
  bytecode VM (the entire point of the Rust runtime is to avoid it). A
  test that compiles and runs in 5 ms via `omg <file>` may take several
  seconds via `omg-test`. Use it for small unit-style tests; for anything
  with a real workload, use `omg <file>` directly and check the exit code.

If/when the runtime grows mutable string builders or persistent
collections, these tools should pick up the speedup automatically — the
algorithms are right, it's the data structures that are pricey.
