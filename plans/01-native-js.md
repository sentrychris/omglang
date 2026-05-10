# native-js — OMG → JavaScript + web playground

Status: To Do
Owner: sentrychris + claude

## Goal

Add a JavaScript backend that mirrors the existing OMG-to-C path, then
ship a single static HTML page that lets anyone run OMG in their
browser. End-state demo: paste OMG source into a textarea, click Run,
see output — same compiler, just a different backend.

## Why this is a showcase

OMG already compiles to native ELFs via [bootstrap/src/native-c.omg](../bootstrap/src/native-c.omg).
A second backend at the *same* point in the pipeline proves the
toolchain is genuinely retargetable. The web playground is the
linkable artefact: drop-in URL, no install, immediate "this language
exists and does things."

## Architecture

Three new pieces, all parallel to existing files:

```
bootstrap/src/native-js.omg     ← parallel to native-c.omg
bootstrap/src/omg_rt.js         ← parallel to omg_rt.h
web/                            ← static playground
├── index.html
├── app.js
├── omg_rt.js                   ← copied from bootstrap/src/
├── compiler.omgb               ← prebuilt; embedded compiler
└── vm.js                       ← vm.omg compiled with native-js
```

### Pipeline (browser case)

```
user types source in textarea
        │
        ▼
vm.js (← vm.omg compiled to JS) runs compiler.omgb on source
        │
        ▼
produces user's bytecode
        │
        ▼
vm.js runs that bytecode
        │
        ▼
output to <pre> in the page
```

This is the same triple-meta dance the Rust runtime does
(`runtime/src/main.rs` running embedded `compiler.omgb` on user input),
just executed in the browser.

Alternative shape (simpler but less impressive): native-js transpiles
*directly* to JS at build time, no VM-in-browser. Skip this unless the
VM-in-browser route hits a wall.

### What native-js.omg emits

For each bytecode op, emit a JS expression manipulating a `stack`
array. Same shape as `emit_c_for_instr` in native-c.omg. Key difference
from the C backend: JS already has GC, so refcount calls drop entirely
(no `omg_retain` / `omg_release`). That makes the JS output ~30%
shorter than the C output for the same input.

### What omg_rt.js provides

- `Value` representation. JS objects with a `tag` field, or take
  advantage of native types: numbers stay JS numbers, strings stay JS
  strings, lists are JS arrays. Use BigInt for OMG int64 to avoid
  precision drift in bitwise ops; downcast for arithmetic if needed.
- All the builtins: `omg_emit`, `omg_length`, `omg_chr`, `omg_ascii`,
  arithmetic, etc. ~30 functions.
- Stubs for builtins that don't make sense in-browser: `read_file`
  reads from a virtual filesystem, `subprocess` panics, `getpid`
  returns 1, `stdin_read` reads from a hidden buffer the page
  populates.

## Scope

| Piece | Lines | Notes |
|---|---|---|
| native-js.omg | ~800 | Same shape as native-c.omg's 1500-ish; less because no refcount or C struct boilerplate |
| omg_rt.js | ~1000 | Smaller than omg_rt.h; JS gives us strings/arrays/dicts for free |
| web/index.html + app.js | ~300 | Textarea, Run button, output pane, basic styling |
| Tests (parity vs C-AOT) | ~150 lines test harness + corpus reuse | |

Total: ~2250 lines of new code. ~1–2 days focused.

## Testing strategy

Existing parity tests already compare Rust runtime, omgc, native-c AOT,
and native interpreted on the [examples/ corpus](../examples/). Add a
fifth path: native-js → run with node → diff stdout. Same 18 example
programs.

Add a row to [tests/parity.sh](../tests/parity.sh):

```bash
section "Parity: native-js (node) vs Rust runtime"
for src in "${EXAMPLES[@]}"; do
    out_path="$TMPDIR_TEST/$(basename "$src" .omg).js"
    "$OMG_RUST" --compile "$src" "$TMPDIR_TEST/in.omgb"
    "$OMG_RUST" bootstrap/src/native-js.omg "$TMPDIR_TEST/in.omgb" "$out_path"
    rust_out=$("$OMG_RUST" "$src" 2>&1)
    js_out=$(node "$out_path" 2>&1)
    if [ "$rust_out" = "$js_out" ]; then
        pass "JS == Rust: $name"
    else
        fail "JS == Rust: $name"
    fi
done
```

## Risks

- **i64 semantics in JS.** JS numbers are f64 — losses of precision on
  bitwise ops over 2^53. Use BigInt for OMG ints throughout. Cost: ~20%
  perf vs Number, but parity matters more.
- **`current_dir` / `args` / `module_file` globals.** Browser has no
  meaningful values. Page provides synthetic ones (`/`, `[]`, `<browser>`).
- **File I/O surface.** Most tools/ programs read files; in-browser
  this means a virtual FS. Out of scope for v1 — playground starts with
  hello-world / fibonacci / closures examples that don't touch files.
- **Compile time in browser.** vm.js compiling compiler.omgb on a
  10KB user program: probably 200–500ms. Tolerable; spinner if needed.

## Where to start

1. **Day 1 morning**: stub `omg_rt.js` with just enough for
   `examples/hello_world.omg` (Value type, `omg_emit`, `omg_str`,
   `omg_int`, the OP_PUSH_INT / OP_PUSH_STR / OP_EMIT / OP_HALT cases).
2. **Day 1 afternoon**: copy native-c.omg → native-js.omg, gut all the
   C-specific bits, get hello_world to round-trip:
   `omg compile → native-js → node → "Hello, world!"`.
3. **Day 2**: extend op coverage until the parity suite passes for the
   18 example programs. Reuse the corpus, don't invent new tests.
4. **Day 2 evening**: write the playground page. ~50 lines HTML + JS.

## Done means

- `tests/run.sh parity` adds a "JS == Rust" section that passes 18/18
- `web/index.html` opens in any modern browser, runs the bundled
  hello-world / closures / prime-sieve examples
- README gets a "Try OMG in your browser" link

## Open questions

- Should `web/` be checked in or built by a `bootstrap/package.sh`-style
  script? Probably the latter — the artefacts are large.
- Host the playground on GitHub Pages or just bundle as a tarball? Pages
  is essentially free; do it.
- Should `native-js.omg` itself ever run *in* the browser? (i.e. could
  someone edit native-js.omg in the browser and recompile vm.js without
  leaving the page?) Cute but not v1.
