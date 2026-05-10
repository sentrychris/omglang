# omgschem — Scheme/Lisp interpreter in OMG

Status: To Do
Owner: sentrychris + claude

## Goal

A small but real Scheme interpreter, written in pure OMG. Single file,
no new builtins, runs as a REPL or executes a `.scm` script.

## Why this is a showcase

The classic "host another language in your language" demonstration.
Scheme is the canonical pick because:

- The reader is tiny (S-expressions are just nested lists).
- The evaluator is short (eval / apply with closures and tail calls is
  the original Scheme paper's whole point).
- It exercises every interesting OMG feature: dicts (environments),
  closures (Scheme procedures), recursion (eval/apply mutual), strings
  + symbols (interned via dict lookup), error handling (try/except for
  unbound variables, type errors).

It's also the lowest-risk plan: no new builtins, no platform concerns,
no binary I/O. If something breaks, it breaks in pure OMG and is
debuggable through existing tooling.

## Subset (R5RS-flavoured minimum)

Atoms:
- Integers (OMG int64) and floats
- Strings, symbols, booleans
- Lists (proper and improper)
- Procedures (built-in and lambda-defined)

Special forms:
- `define`, `set!`
- `lambda`, `if`, `cond`, `when`, `unless`
- `let`, `let*`, `letrec`
- `and`, `or`, `not`, `begin`
- `quote` and the `'` reader shorthand

Built-in procedures:
- Arithmetic: `+ - * / mod < > <= >= =`
- Predicates: `null? pair? number? string? symbol? procedure? eq? equal?`
- Lists: `car cdr cons list length reverse append map filter`
- Strings: `string-append string-length substring string->symbol symbol->string`
- I/O: `display newline read read-line` (reads from stdin via `stdin_readline`)
- Control: `error`

What's deliberately out of scope: `call/cc`, full numeric tower,
macros (`define-syntax`), tail-call optimisation (or: implement only
trivially via a trampolined eval — see Risks).

## Architecture

Single file: `tools/lisp/omgschem.omg`. Three layers stacked top-to-
bottom: reader, evaluator, REPL.

### Reader (~300 lines)

```
tokenise(src) → list of tokens
  Tokens: "(", ")", "'", numbers, strings, symbols, booleans

parse(tokens) → ast
  Recursive descent. AST is OMG-native: nested lists.

  Scheme       OMG-side AST
  ───────────────────────────────────────
  42           42
  "hi"         "hi"
  foo          {tag: "sym", name: "foo"}
  ()           []
  (a b c)      [{sym a}, {sym b}, {sym c}]
  'x           [{sym quote}, {sym x}]
```

### Evaluator (~500 lines)

Environment is a list of dicts (innermost-first). `lookup`,
`define_in`, `set_in` walk the chain. New scopes prepend a fresh
dict.

```
eval(expr, env) → value
  numbers/strings/booleans/lists  → self-evaluating
  symbol                          → lookup(env, sym)
  ('quote, x)                     → x
  ('if, c, t, e)                  → eval branch
  ('lambda, params, body)         → ["closure", params, body, env]
  ('define, name, value)          → define_in(env, name, eval(value, env))
  (proc, args...)                 → apply(eval(proc, env), [eval(a, env) for a in args])

apply(proc, args) → value
  built-in   → call OMG-side handler
  closure    → eval(body, extend(closure_env, params, args))
```

Closures hold their definition-time env, exactly as the rest of OMG
does — symmetry that makes the implementation feel natural to write.

### Built-ins (~300 lines)

Each Scheme built-in is one OMG `proc` taking a list of evaluated
args, returning a value (or panicking on type error). A dict at the
top of `omgschem.omg` maps Scheme names to these procs. Fallback
unknown lookup → `error: unbound variable: <name>`.

### REPL + script mode (~200 lines)

```
omgschem                    # interactive REPL
omgschem foo.scm            # run a script
echo '(display 42)' | omgschem -    # run from stdin
```

REPL: read full S-expression (track paren balance like
[bootstrap/src/omg.omg's REPL](../bootstrap/src/omg.omg) does);
eval; print; loop. Errors caught by try/except and printed without
exiting.

## Scope

| Piece | Lines |
|---|---|
| Reader (tokeniser + parser) | ~300 |
| Evaluator | ~500 |
| Built-in procedures | ~300 |
| REPL + script mode | ~200 |
| Tests (Scheme programs) | ~150 |
| **Total** | **~1450** |

~1–2 days.

## Test programs

A `tools/lisp/examples/` directory:

- `factorial.scm`: classic recursion; tests numbers, recursion,
  closures.
- `fib.scm`: same, plus naive vs memoised version (tests dict-based
  memo).
- `mergesort.scm`: tests `cons`/`car`/`cdr`/`map`/`length`.
- `church.scm`: Church numerals — purely functional, no built-ins
  beyond `lambda` and procedure application. Showcase: closures all
  the way down.
- `metacirc.scm`: a 50-line eval/apply written *in* Scheme, evaluated
  by `omgschem`. The "interpreter inside an interpreter inside an
  interpreter" trick.

Each example is a `.scm` file checked in alongside its expected
output (`.scm.expected`). A test in `tests/run.sh` (or a local
`tools/lisp/test.sh`) runs each through `omgschem` and diffs output.

## Risks

- **Stack depth on recursion.** OMG's call stack is bounded by the
  host VM. Naive eval-as-recursion blows up on Scheme recursion that
  isn't tail-call optimised. Mitigation: eval works in a manual loop
  for tail positions (`begin` last-expr, `if` arm, `let` body), so
  Scheme tail-recursive programs become OMG iterative loops in eval.
  ~50 extra lines, big practical win, lets `(define (loop n) (if
  (zero? n) 'done (loop (- n 1))))` actually terminate.
- **No `call/cc`.** Out of scope; document. Scheme without call/cc is
  still recognisably Scheme.
- **Symbol equality.** OMG strings compared with `==` work fine for
  `eq?` if symbols are stored as plain strings inside the
  `{tag: "sym", name: "..."}` shape. No interning needed; equality
  on the wrapping dict's `name` field is enough.
- **Floating-point parse edge cases.** Reuse the lexer pattern from
  [bootstrap/src/compiler.omg](../bootstrap/src/compiler.omg)'s number
  scanner. Don't reinvent.

## Where to start

1. **Hour 1**: tokeniser, with a dozen unit-style tests embedded in a
   `proc test_tokenise()` at the bottom of the file.
2. **Hour 2**: parser. Numbers, strings, booleans, lists, quote
   shorthand. Test by round-tripping to a printer.
3. **Hour 3–4**: bare-minimum evaluator: numbers, `+`, `-`, `if`,
   `define`, `lambda`. `factorial.scm` should run.
4. **Hour 5–6**: rest of the special forms + the built-in procedure
   table. `mergesort.scm` should run.
5. **Hour 7**: REPL with paren-balance multi-line input.
6. **Hour 8**: tail-position hoisting in eval for `begin`/`if`/`let`.
   Validate by running `loop` from `(define (loop n) ...)` for n=10000.
7. **Hour 9–10**: examples, expected outputs, a test runner.

## Done means

- `tools/lisp/omgschem.omg` runs every example in
  `tools/lisp/examples/` to its expected output.
- The metacircular evaluator example runs end-to-end (Scheme inside
  Scheme inside OMG inside the OMG VM inside the Rust runtime — five
  layers of interpretation).
- README gets a one-liner: "OMG can host other languages — see
  `tools/lisp/`."

## Open questions

- Worth bothering with `define-syntax` (hygenic macros)? No — that's a
  whole other project. Skip.
- Should `omgschem` itself be AOT-compileable to a standalone binary
  via `omg --build`? Yes; nothing about it should block AOT. Confirm
  it actually works at the end and add to the parity corpus.
- Should error messages include source line numbers? The parser would
  need to thread span info through the AST. Cute but adds ~100 lines;
  cut from v1.
