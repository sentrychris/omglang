# omgtetris — TUI apps in OMG (2048 → vim-clone → Tetris)

Status: To Do

Owner: sentrychris + claude

## Goal

Three terminal apps in escalating complexity, each doable as a
standalone project. The progression is the plan: ship 2048 quickly to
prove ANSI rendering works, then a small modal text editor, then real-
time Tetris (which requires one new builtin).

## Why this is a showcase

These are visceral. A README screenshot or a short screencast carries
the demo more than any code listing would. They're also the natural
test of OMG's interactive story:

- Can OMG render to a terminal in real time? (yes, via ANSI escapes)
- Can it read keystrokes? (today: only with Enter; for Tetris: no.)
- Is the language ergonomic enough that a 600-line app stays readable?

## The three tiers

### Tier 1: 2048 (~200 lines, no new builtins)

```
+-----+-----+-----+-----+
|     |  2  |     |  4  |
+-----+-----+-----+-----+
|  4  |     |  8  |     |
+-----+-----+-----+-----+
|     | 16  |     |  2  |
+-----+-----+-----+-----+
|  2  |     |     |     |
+-----+-----+-----+-----+
Score: 84    Move (w/a/s/d, q to quit):
```

Input via `stdin_readline()`. User types `w` + Enter; we read the
line, take the first character, dispatch. Each turn redraws the
board with ANSI cursor-home + clear.

Single file. Pure OMG. Done in an afternoon.

### Tier 2: omgvi — minimal modal editor (~600 lines)

A tiny line-oriented editor with `normal` and `insert` modes. The
catch: each command is one line of input, terminated by Enter.

Commands (normal mode): `i` insert, `a` append, `dd` delete line,
`x` delete char, `o` open line, `:w` write, `:q` quit, `:wq`,
`/pattern` search forward, `n` next match, `gg`/`G` jump.

Insert mode: each Enter-terminated line is appended to the buffer.
Press Enter on an empty line to leave insert mode (substitute for
ESC).

```
~ omgvi notes.txt
─────────────────────────────────
  1 the first line
  2 the second line
> 3 the cursor is here
─────────────────────────────────
NORMAL  notes.txt  3:1
:
```

Demonstrates: file I/O, list-based buffer, text-search, ANSI
rendering with explicit cursor position. About a day's work.

### Tier 3: real-time Tetris (~800 lines + 1 new builtin)

What 2048 looks like *with gravity*. Pieces fall on a clock; user
input must be detected without blocking on Enter.

```
┌────────────────────┐
│        ██          │
│      ████          │
│        ██          │
│                    │
│ ██                 │
│ ████   ██          │
│ ████ ██████  ██████│
└────────────────────┘
Score: 1240   Lines: 14   Level: 3
[a/d move  s drop  w rotate  q quit]
```

This needs raw-mode terminal input — a single keystroke at a time,
no Enter required, non-blocking poll.

## The new builtin (Tetris only)

`stdin_read_char(timeout_ms) → str | bool`

- Returns one character (a 1-char string) if available within the
  timeout.
- Returns `false` on timeout, EOF, or "no input ready."
- Implies the program has put the terminal into raw mode (no line
  buffering, no echo). Tetris invokes a `terminal_raw_mode(true)` /
  `terminal_raw_mode(false)` pair around its main loop.

That's two new builtins really:

- `stdin_read_char(timeout_ms)` — non-blocking single-byte read
- `terminal_raw_mode(on)` — wraps `tcsetattr`

Cost in `omg_rt.h`: ~80 lines of `termios` and `select()` plumbing.
Cost in the Rust runtime (`runtime/src/vm/builtins.rs`): similar,
using `crossterm` or raw libc. Both backends mirror each other.

## Architecture (Tetris)

```
main loop:
    raw_mode(on)
    last_tick = monotonic_ms()
    loop !game_over {
        ch = stdin_read_char(50)        // 50ms input poll
        if ch != false { handle_input(ch) }

        now = monotonic_ms()
        if now - last_tick >= drop_interval(level) {
            advance_gravity()
            last_tick = now
        }

        if board_changed { redraw() }
    }
    raw_mode(off)
```

Implies a third tiny builtin: `monotonic_ms()` returning a
millisecond-precision integer. (Or computed from `time(NULL) * 1000 +
ms_part` — depends on whether we want sub-second precision; for
Tetris, yes.)

## Scope (cumulative)

| Tier | New OMG | New builtins | Days |
|---|---|---|---|
| 2048 | ~200 lines | 0 | 0.5 |
| omgvi | ~600 lines | 0 | 1.0 |
| Tetris | ~800 lines | 3 (`stdin_read_char`, `terminal_raw_mode`, `monotonic_ms`) | 1.5 |

Recommended path: ship 2048 first, take a week off, ship omgvi, then
do Tetris when there's appetite for adding builtins. None of the tiers
depends on the others, but they're a natural progression.

## Files

```
tools/games/
├── 2048.omg
├── tetris.omg            ← tier 3
└── README.md             ← controls, screenshots

tools/editors/
└── omgvi.omg             ← tier 2
```

## Risks

- **ANSI portability.** Linux + macOS terminals are fine. Windows
  consoles vary; matches existing project policy ("not Windows-tested"
  per [README.md](../README.md)). Document as a Linux/macOS demo.
- **Raw-mode cleanup on crash.** If an OMG runtime error fires inside
  Tetris's main loop, the terminal is left in raw mode. Need a crash-
  hook to restore. Either: a `defer raw_mode(off)`-style mechanism (no
  such language feature today), or wrap the main loop in `try /
  except` and call `raw_mode(off)` in both arms. The latter is
  enough.
- **Refresh flicker.** Naive full-screen redraws each frame look bad.
  Tetris should use ANSI cursor positioning to redraw only changed
  cells. Adds ~50 lines but is the difference between charming and
  embarrassing.
- **stdin_read_char with timeout 0** vs **fully blocking** — pick
  semantics carefully. Recommend: `0` = non-blocking poll (return
  `false` immediately if nothing ready); `>0` = wait up to N ms;
  `<0` = block forever. Document explicitly.

## Where to start

1. **Day 1, morning**: 2048. Single file, no new builtins. One sitting.
2. **Day 1, afternoon**: 2048 polish — colour cells by value (ANSI 256
   colour codes), nice animations on merge. Not strictly necessary but
   the difference is noticeable.
3. **Pause** at this point and ship. The rest can wait.
4. **Later**: omgvi. Same shape (input-loop → render), but mutating a
   buffer and round-tripping a file.
5. **Later still**: builtins for Tetris. Land them in their own commit
   with `tests/builtins.sh` coverage. *Then* write Tetris.

## Done means

- 2048 has a checked-in screenshot in [tools/games/README.md](../tools/games/README.md)
  and a ~1-line invocation example in the top-level README's tools
  table.
- omgvi can edit a real file end-to-end and is dogfoodable for editing
  short scripts.
- Tetris runs at a steady ~30 FPS with smooth input on a Linux
  terminal.

## Open questions

- For omgvi, do we want syntax highlighting for `.omg` files? Cute but
  not essential; cut from v1.
- Should the new Tetris builtins go behind a feature flag in
  `omg_rt.h` so embedded targets without `termios` still build? Yes —
  use `#ifdef OMG_HAVE_TERMIOS` and stub the builtins to panic on
  platforms where they're not compiled in.
