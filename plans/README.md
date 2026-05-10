# Showcase plans

Four projects that would each, in different ways, demonstrate what OMG
can do now that the native pipeline, self-hosted compiler, OMG-on-OMG
VM, and stdin builtins are all in place. None of these are scheduled —
they're sketches detailed enough to pick up and start from.

## The four

| # | Plan | What it shows | Approximate scope |
|---|---|---|---|
| 1 | [native-js — OMG to JavaScript + web playground](01-native-js.md) | Multi-target: same compiler, two backends; OMG runs in any browser | ~2500 lines (transpiler + JS runtime + playground), ~1–2 days focused |
| 2 | [omgdb — SQL-subset embedded database](02-omgdb.md) | Real systems work: B-trees, on-disk format, query parser, executor | ~3000 lines OMG, ~3–5 days |
| 3 | [omgtetris — TUI app (2048 → vim-clone → Tetris)](03-omgtetris.md) | Interactive, visual; stresses ANSI rendering and (eventually) real-time input | 200 → 600 → 800 lines; needs one new builtin for the Tetris tier |
| 4 | [omgschem — Scheme/Lisp interpreter in OMG](04-omgschem.md) | Hosting another language; classic showcase; pure OMG, no new builtins | ~1300 lines, ~1–2 days |

## Suggested ordering

If picking one:

- **Most impressive externally**: 1 (native-js). It's the most linkable
  and the most "wait, what?" — paste OMG in a textarea, hit run, see
  output. The story writes itself.
- **Easiest to start**: 4 (omgschem). Pure OMG, no new builtins, no
  external dependencies. A good warmup that proves the language scales
  to a real interpreter.
- **Most "real software"**: 2 (omgdb). Forces the language through
  binary I/O, complex algorithms, and persistence.
- **Most fun and visible**: 3 (omgtetris). Demos well in a video.

If doing all four, I'd order: **4 → 1 → 2 → 3**.

- **4 first** — fastest win, builds confidence that OMG handles a
  large single-file project cleanly.
- **1 second** — biggest payoff per hour, and it benefits from anything
  4 reveals about edge cases.
- **2 third** — bigger investment, and it'll exercise file I/O paths in
  ways that may surface bugs worth fixing before tackling…
- **3 last** — needs a new `stdin_read_char` builtin (raw mode + single
  byte) for the Tetris tier; saving it for last means the rest of the
  toolchain has been hammered first.

## What's already in place these plans assume

- Self-hosted compiler with byte-identical fixed point ([bootstrap/src/compiler.omg](../bootstrap/src/compiler.omg))
- OMG-in-OMG VM ([bootstrap/src/vm.omg](../bootstrap/src/vm.omg))
- OMG-to-C transpiler + native ELF AOT path ([bootstrap/src/native-c.omg](../bootstrap/src/native-c.omg), [bootstrap/src/omg_rt.h](../bootstrap/src/omg_rt.h))
- Process control: `subprocess`, `exit`, `getpid`
- Standard input: `stdin_readline`, `stdin_read`, `stdin_read_bytes`
- File I/O: `read_file`, `file_open`/`file_read`/`file_write`/`file_close` (text + binary modes)
- Closures, dicts, lists, imports, try/except
