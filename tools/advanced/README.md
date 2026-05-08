# tools/advanced

Demo-quality projects that show OMG handling something larger than a
single Unix-style utility. Each tool here imports from `tools/` and
`tools/advanced/` to compose features, rather than re-implementing
everything inline.

## ssg.omg — static site generator

A tiny markdown-based static site generator written in OMG.

```sh
omg tools/advanced/ssg.omg <site-dir>
```

It expects this layout under `<site-dir>/`:

```
<site-dir>/
├── content/                      input markdown
│   ├── index.md                  → out/index.html (uses templates/index.html)
│   ├── about.md                  → out/about.html
│   └── posts/
│       ├── hello-world.md        → out/posts/hello-world.html
│       └── another-post.md       → out/posts/another-post.html
└── templates/
    ├── default.html              required: layout for every page
    └── index.html                optional: layout for content/index.md
```

After running, the generated site appears under `<site-dir>/out/`,
mirroring the directory structure of `content/`.

A worked example lives in [`example-site/`](./example-site/) — try it:

```sh
omg tools/advanced/ssg.omg tools/advanced/example-site
open tools/advanced/example-site/out/index.html
```

### Front-matter

Every markdown file may begin with a small block of `key: value` lines
between two `---` separators:

```markdown
---
title: Hello, world
date: 2026-05-08
---

# Body starts here.
```

The values become template variables (`{{title}}`, `{{date}}`). Pages
without front-matter render fine; the corresponding placeholders are
left as-is.

### Templates

Templates are plain HTML with `{{name}}` placeholders. The SSG provides:

| Variable    | Where it comes from                                     |
| ----------- | ------------------------------------------------------- |
| `{{title}}` | Front-matter `title:` (if set)                          |
| `{{date}}`  | Front-matter `date:` (if set)                           |
| `{{body}}`  | Markdown body, converted via `tools/md2html.omg`        |
| `{{posts}}` | Index template only: `<li>` list of posts under `posts/` |

Unknown placeholders are preserved verbatim, so typos are visible in the
output rather than silently dropped.

### How it works

In rough order:

1. **Walk** `content/` recursively (`read_dir` + `is_dir`), gather every
   `.md` file in lexicographic order.
2. **Pass 1: load.** For each file: read raw text, peel off the
   front-matter, convert the markdown body to HTML via the `convert()`
   function exported by [`tools/md2html.omg`](../md2html.omg). Collect
   one record per post (`[src_rel, dst_rel, meta, body_html]`). Two
   passes are needed because the index template needs the full post list
   *before* it can render.
3. **Pass 2: render.** Pick `index.html` for `content/index.md`,
   otherwise `default.html`. Substitute placeholders. Write to the
   mirror path under `out/`, creating intermediate directories with
   `make_dir` (mkdir -p semantics).

### Limitations

These are demo-friendly cuts, not bugs:

- No drafts, no tags, no incremental rebuilds.
- Templates are flat: no `{{#if}}` / `{{#each}}` blocks. The post-list
  rendering is hard-coded.
- Front-matter is line-oriented `key: value`; no quoting, no nested
  structures, no YAML lists.
- Markdown subset is whatever
  [`tools/md2html.omg`](../md2html.omg) supports — ATX headers,
  paragraphs, fenced code blocks, inline `**bold**` / `*italic*` /
  `` `code` ``, `[text](url)` links, and ordered/unordered lists. No
  tables, no nested lists, no footnotes.

### Runtime requirements

The SSG depends on three filesystem builtins added alongside it:
`read_dir`, `is_dir`, `make_dir`. They live in the runtime so this
demo could exist; see
[`runtime/src/vm/builtins.rs`](../../runtime/src/vm/builtins.rs).
