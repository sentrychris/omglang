# OMG games

Interactive terminal games written in OMG.

These work on Linux ttys. The browser playground can compile them but
not run them (no controlling terminal).

## What's here

| Game            | Size           | Highlights |
|-----------------|----------------|------------|
| [`snake.omg`](snake.omg)   | ~200 lines | Walls, self-collision, fruit spawning via LCG, speed-up per apple. Hjkl / wasd / arrows. |
| [`pong.omg`](pong.omg)     | ~250 lines | Single-player vs CPU, edge-of-paddle deflection, first to 7. CPU's max speed is tuned to be beatable. |
| [`tetris.omg`](tetris.omg) | ~400 lines | 7 tetrominoes with 4 rotation states each, line clearing, level/score, hard + soft drop, pause. |

## How to play

Both games AOT-compile cleanly. Build once, run from a terminal:

```sh
omg --build games/snake.omg snake && ./snake
omg --build games/tetris.omg tetris && ./tetris
```

The interpreter path (`omg games/snake.omg`) works too, the inner
loop is cheap, so the tick rate is fine either way. AOT gives slightly
crisper input feedback at high tick rates.

Quit with `q` in both games. If something goes wrong and your shell
prompt looks weird afterwards, run `reset` to restore.