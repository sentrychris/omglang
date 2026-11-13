# OMG games

Interactive terminal games written in OMG.

They need a real terminal: a Linux or macOS tty, or a Windows console
(Windows Terminal or the classic one). The browser playground can
compile them but not run them (no controlling terminal).

## What's here

| Game            | Size           | Highlights |
|-----------------|----------------|------------|
| [`snake.omg`](snake.omg)   | ~250 lines | Walls, self-collision, fruit spawning via LCG, speed-up per apple. Hjkl / wasd / arrows. |
| [`pong.omg`](pong.omg)     | ~300 lines | Single-player vs CPU, edge-of-paddle deflection, first to 7. CPU's max speed is tuned to be beatable. |
| [`rogue.omg`](rogue.omg)   | ~850 lines | Procgen rooms + corridors, Bresenham FOV (visible/explored/dark), 6 monster types with depth-scaled spawns, bump combat, potions + gold, infinite descent. First game in the repo to use ANSI colour. |
| [`tetris.omg`](tetris.omg) | ~450 lines | 7 tetrominoes with 4 rotation states each, line clearing, level/score, hard + soft drop, pause. |

## How to play

All four build to small standalone executables. Build once, run from
a terminal:

```sh
omg --build examples/games/snake.omg snake && ./snake
omg --build examples/games/tetris.omg tetris && ./tetris
```

```powershell
# Windows
bin\omg.exe --build examples\games\snake.omg snake
.\snake.exe
```

The interpreter path (`omg examples/games/snake.omg`) works too, the inner
loop is cheap, so the tick rate is fine either way. A built game gives
slightly crisper input feedback at high tick rates.

Quit with `q` in every game. If something goes wrong and your shell
prompt looks weird afterwards, run `reset` to restore.