# OMG games

Interactive terminal games written in OMG. They lean on the runtime's
real-time I/O primitives:

| Builtin | Purpose |
|---|---|
| `time_ms()` | monotonic-ish millisecond clock for tick pacing |
| `sleep_ms(n)` | pause between frames |
| `stdin_set_raw(on)` | cbreak / no-echo mode for non-blocking input |
| `stdin_read_key()` | one-byte non-blocking read |

These work on Linux ttys. The browser playground can compile them but
not run them (no controlling terminal).

## What's here

| Game            | Size           | Highlights |
|-----------------|----------------|------------|
| [`snake.omg`](snake.omg)   | ~200 lines | Walls, self-collision, fruit spawning via LCG, speed-up per apple. Hjkl / wasd / arrows. |
| [`tetris.omg`](tetris.omg) | ~400 lines | 7 tetrominoes with 4 rotation states each, line clearing, level/score, hard + soft drop, pause. |

## How to play

Both games AOT-compile cleanly. Build once, run from a terminal:

```sh
omg --build games/snake.omg snake && ./snake
omg --build games/tetris.omg tetris && ./tetris
```

The interpreter path (`omg games/snake.omg`) works too — the inner
loop is cheap, so the tick rate is fine either way. AOT gives slightly
crisper input feedback at high tick rates.

Quit with `q` in both games. If something goes wrong and your shell
prompt looks weird afterwards, run `reset` to restore cooked mode.

## Why a separate folder?

The games are bigger and more opinionated than the snippets in
[`../examples/`](../examples/) (each example is a focused 10-50 line
demo of one language feature). They also depend on Linux TTY
behaviour, which the parity test deliberately doesn't exercise — so
keeping them out of `examples/` keeps the parity matrix clean.

## Notes

- The pseudo-random number generator in both games is a textbook LCG
  seeded from `time_ms()`. Reproducible if you fix the seed; not
  cryptographic.
- Both games wrap `stdin_set_raw(true)` in a try/except so a piped
  invocation (no TTY) prints a friendly error instead of a traceback.
  Useful for CI smoke tests.
- Tetris's `render()` packs its piece + HUD args into 3 parameters.
  Originally that was a workaround for `OMG_MAX_ARITY = 8` in the
  C-AOT path; the cap is now 32, but the packed signature is still
  cleaner to read so we kept it.
