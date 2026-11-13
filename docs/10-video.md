# 10. A window, and a video player

`tools/video/play.omg` plays the video of an MP4 file in a window:

```sh
omg --build tools/video/play.omg play
./play clip.mp4
```

Everything between the file and the screen is OMG. The MP4 reader and
the H.264 decoder are OMG programs, and the window is opened by the
runtime, itself OMG, speaking to the system directly. No library is
linked in and no other program is started: `play` is one executable of
about 600 KB, and `--target windows-x64` gives the same thing for
Windows from Linux.

This page covers the four builtins that give a program a window, then
the video tools built on them.

- [A window](#a-window)
- [How the window gets on the screen](#how-the-window-gets-on-the-screen)
- [The player](#the-player)
- [Reading an MP4 file](#reading-an-mp4-file)
- [Decoding H.264](#decoding-h264)
- [How fast it is](#how-fast-it-is)
- [How it is tested](#how-it-is-tested)
- [What is not there](#what-is-not-there)

## A window

| Builtin | What it does |
| ------- | ------------ |
| `window_open(title, width, height)` | Open a window whose inside is `width` x `height` pixels. Returns a handle. |
| `window_draw(win, pixels, width)` | Show a picture: a list with one int per pixel. |
| `window_poll(win)` | The next thing that happened to the window, or `false`. Never waits. |
| `window_close(win)` | Close it. |

A complete program:

```omg
alloc w := 320
alloc h := 240
alloc win := window_open("hello", w, h)
alloc pixels := list_repeat(0, w * h)

alloc t := 0
alloc open := true
loop open {
    # a picture: one number per pixel, red * 65536 + green * 256 + blue
    alloc y := 0
    loop y < h {
        alloc x := 0
        loop x < w {
            pixels[y * w + x] := (((x + t) & 255) << 16) | ((y & 255) << 8) | 128
            x := x + 1
        }
        y := y + 1
    }
    window_draw(win, pixels, w)

    # what the user did since last time
    alloc ev := window_poll(win)
    loop ev != false {
        if ev[0] == "close" or (ev[0] == "key" and ev[1] == "q") {
            open := false
        }
        ev := window_poll(win)
    }
    sleep_ms(20)
    t := t + 4
}
window_close(win)
```

**The picture.** `pixels` holds the rows from the top, one after
another, and `width` says how long a row is; the height follows from
the length of the list. A pixel is `red * 65536 + green * 256 + blue`,
each from 0 to 255. The picture does not have to be the size of the
window. It is scaled to fit, keeping its shape, with black bars where
the shapes differ, so a program draws at the size it likes and the
user resizes the window as they like. The window keeps the last
picture and repaints it by itself when it has been covered.

**What happened.** `window_poll` returns `false` when there is nothing,
and otherwise one of:

| Value | Meaning |
| ----- | ------- |
| `["key", name]` | A key went down. |
| `["resize", width, height]` | The window has a new size. The picture has already been fitted to it. |
| `["close"]` | The user closed the window. It is gone: stop drawing. |

Keys that print are named by their character: `"a"`, `"A"` with Shift,
`"7"`, `"."`. The others are `"space"`, `"enter"`, `"escape"`, `"tab"`,
`"backspace"`, `"delete"`, `"left"`, `"right"`, `"up"`, `"down"`,
`"home"`, `"end"`, `"pageup"`, `"pagedown"` and `"f1"` to `"f12"`.

Call `window_poll` often, in a loop until it returns `false`. It is
also what keeps the window alive: a program that stops calling it has
a window the system reports as not responding.

**Limits.** One window at a time. There is no mouse, no text drawing
and no sound: it is a surface for pictures, and what a video player or
a simple game needs. A program that has a window is still a console
program: `emit` goes to the terminal it was started from.

**Where it works.** In a program run by `omg` or built with
`omg --build` for Linux or Windows. Built through C
(`--target c`) or as JavaScript, the four builtins are there and raise
a `ValueError` that says so, which a program can catch. `window_open`
raises a `ValueError` too when there is nowhere to show a window (on
Linux: `DISPLAY` is not set, or no X server answers there).

## How the window gets on the screen

Neither system's window code is linked into an OMG program; there is
no linker. The runtime does what a C program's libraries would do.

**Linux** (`src/runtime/x64/linux.omg`). An X server is a program that
listens on a socket; drawing is a conversation with it. The runtime
connects to the socket `DISPLAY` names (`/tmp/.X11-unix/X0` for `:0`,
TCP for `host:0`), introduces itself, with the cookie from
`~/.Xauthority` if there is one, and then sends the requests of the
X11 core protocol as bytes it lays out itself: create a window, name
it, map it, and for every picture `PutImage` with the pixels. Events
(key presses, exposures, a new size, the close button) arrive on the
same socket and `window_poll` reads them. Wayland desktops run an X
server for programs like this one (XWayland), so it works there too,
as long as `DISPLAY` is set.

**Windows** (`src/runtime/x64/windows.omg`). The executable imports
what it needs from `user32.dll` and `gdi32.dll` the same way it
imports file and console functions from `kernel32.dll`: the names go in
the import table that OMG's PE writer builds, and Windows fills in the
addresses when the program starts. A window class, `CreateWindowExW`,
a message loop run from `window_poll`, and `SetDIBitsToDevice` to put
the pixels up. Only a program that uses a window imports these two
libraries.

The part that is the same on both, scaling the picture into a block of
pixels in the form both systems take, is in
`src/runtime/x64/builtins.omg`. It runs as machine code whether the
program is built or interpreted, so `window_draw` costs the same either
way.

## The player

```sh
omg --build tools/video/play.omg play        # or: play.exe --target windows-x64
./play clip.mp4 [--loop] [--scale N] [--fast]
```

| Key | Does |
| --- | ---- |
| space | pause, and go on |
| `.` | while paused: show the next picture |
| left, right | jump five seconds back or forward |
| home | back to the start |
| `q`, escape | quit (so does closing the window) |

`--loop` starts again at the end. `--scale N` opens the window N times
the size of the video (small videos get 2 or 3 by default). `--fast`
does not wait for each picture's time, which measures how fast the
machine can go.

Pictures are shown at the times the file gives them. When the machine
cannot decode that fast, the player first stops drawing some pictures
(decoding cannot be skipped: each picture is built from earlier ones),
and if that is not enough it lets the video run slow rather than
freeze. At the end it prints what happened:

```
mandel.mp4: 320x240, 50 pictures, 25.0 a second
50 pictures shown in 1.9 s: 25.3 a second
decoding took 15.5 ms a picture, drawing 11.9 ms
```

Run interpreted (`omg tools/video/play.omg clip.mp4`) it works, but
far too slowly for anything larger than a postage stamp: build it.

The player is 300 lines. It is three modules put together:

```omg
import "mp4.omg" as mp4
import "h264.omg" as h264

alloc m := mp4.open("clip.mp4")
h264.set_config(m.config)
alloc info := h264.info()
alloc win := window_open("clip", info.width, info.height)
alloc rgb := list_repeat(0, info.width * info.height)
alloc i := 0
loop i < m.count {
    alloc pic := h264.decode(mp4.read_sample(m, i))
    if pic != false {
        h264.to_rgb(pic, rgb)
        window_draw(win, rgb, info.width)
        h264.release(pic)
    }
    i := i + 1
}
```

(which shows the pictures as fast as they come, in the order they are
stored; `play.omg` adds the timing, the order of showing and the keys.)

## Reading an MP4 file

`tools/video/mp4.omg`. An MP4 file is a tree of boxes: four bytes of
length, four of name, then the contents, which may be more boxes. The
pictures are all in one box, `mdat`, as a run of bytes with nothing to
say where one ends. The index is in another, `moov`: for each track a
handful of tables that give every sample's size, where it starts, when
it is decoded and shown, and whether decoding can start there.

`mp4.open(path)` reads only the index and returns a dict:

| Key | What it holds |
| --- | ------------- |
| `codec` | `"avc1"` for H.264; whatever the file says otherwise |
| `config` | the decoder's configuration (for H.264 the `avcC` box), a list of bytes |
| `width`, `height` | of the video |
| `timescale`, `duration` | ticks in a second; length of the track in ticks |
| `count` | number of samples (pictures) |
| `sizes`, `offsets` | per sample: its length, and where it starts in the file |
| `dts`, `pts` | per sample: when to decode it, when to show it, in ticks |
| `sync` | per sample: 1 when decoding can start there |
| `order` | sample numbers in the order they are shown |
| `audio` | codec of the sound track, `""` when there is none |

`mp4.read_sample(m, i)` seeks and reads one sample as a list of bytes;
`mp4.close(m)` closes the file. The file is never read whole, so its
size does not matter.

`tools/video/mp4info.omg` prints what a file holds, and with `--frames`
decodes it and prints a checksum for every picture:

```
$ omg tools/video/mp4info.omg tests/video/clip.mp4
# video: avc1 82x66, 20 frames, 1.333 s, 15.0 frames a second
```

## Decoding H.264

`tools/video/h264.omg`, 3,500 lines, is a decoder for the Baseline
profile of H.264: the subset meant for simple devices, and what
`ffmpeg -profile:v baseline` writes.

| Proc | Does |
| ---- | ---- |
| `reset()` | Forget everything, to start on another stream. |
| `set_config(bytes)` | Take the `avcC` configuration of an MP4 file. |
| `decode(sample)` | Decode one sample of an MP4 file. Returns a picture, or `false`. |
| `decode_stream(bytes)` | Decode a raw `.h264` stream. Returns a list of pictures. |
| `info()` | `{width, height, coded_width, coded_height, crop_left, crop_top, profile, level}` |
| `to_rgb(pic, out)` | Fill `out` with the picture as `window_draw` takes it. |
| `checksum(pic)` | The number `ffmpeg -f framecrc` prints for the same picture. |
| `release(pic)` | Hand a picture back, so its memory is used again. |

What happens to a picture on its way through, in the order of the file's
sections:

1. **Bits.** The stream is cut into units, the escape bytes are taken
   out, and values are read a bit at a time, most of them in a
   variable-length code where small numbers are short.
2. **Parameters.** Two kinds of unit describe the stream (its size, how
   pictures are numbered) and are remembered. Every picture then starts
   with a header that refers to them.
3. **Macroblocks.** A picture is a grid of 16x16 blocks. Each is
   *predicted*, either from its neighbours above and to the left in the
   same picture (17 ways of continuing edges into the block), or from
   an earlier picture, displaced by a motion vector with quarter-pixel
   precision (the in-between positions come from a six-tap filter).
   Blocks can be split down to 4x4, each part with its own vector.
4. **What prediction got wrong.** The difference between the prediction
   and the real picture is stored as the coefficients of a 4x4
   transform, scaled down so that most are zero, and coded with tables
   that adapt to the blocks around (CAVLC). The decoder reads them,
   scales them back up, runs the inverse transform and adds the result
   to the prediction.
5. **Deblocking.** Working in blocks leaves visible edges. A filter runs
   over every block edge of the finished picture and smooths it by an
   amount that depends on how the two sides were coded.
6. **Remembering.** Finished pictures are kept for later ones to predict
   from; the stream says how many, and can order them about.

All of it is integer arithmetic, and the standard pins down every
step, so there is one right answer for every pixel.

## How fast it is

Built with `omg --build`, on one core of a 2.3 GHz Xeon server, in
milliseconds a picture (`play --fast` prints these):

| Video | Decoding | Converting and drawing | Pictures a second |
| ----- | -------- | ---------------------- | ----------------- |
| 176x144 | 1.0 | 1.1 | 450 |
| 320x240, a test pattern | 3.4 | 4.0 | 130 |
| 320x240, a Mandelbrot zoom (hard to compress) | 4.7 | 3.7 | 120 |
| 640x480 | 11 | 7 | 55 |
| 1280x720 | 31 | 20 | 19 |

Video is usually 25 or 30 pictures a second, so up to 640x480 plays at
its proper speed with time to spare. At 1280x720 decoding alone just
fits, and decoding and drawing together do not: the player keeps time
by drawing about one picture in three. Full HD (1920x1080) is beyond
it.

For scale: ffmpeg decodes the same files 15 to 30 times faster, with
inner loops written by hand in assembly. The OMG decoder is ordinary
OMG; nothing in it is special to the compiler. Two things in the back
end made it this fast, and they help every OMG program that loops over
lists of numbers: reading and writing list elements, and shifts, are
compiled in line instead of as calls into the runtime
([04-pipeline.md](04-pipeline.md) shows the code). Before that the
figures above were 2.4 times higher for decoding and 5 times for
converting.

Run by the interpreter instead of built, decoding is about 250 times
slower.

## How it is tested

H.264 decoding is exact, so the referee is another decoder. For every
test clip, `ffmpeg -f framecrc` gives a checksum of each decoded
picture, and `mp4info --frames` must print the same numbers.
`tests/video.sh` does this for a clip and a raw stream that are checked
in with their expected checksums (ffmpeg is not needed to run the
tests), both interpreted and built. While the decoder was written the
same comparison ran over a dozen clips made to exercise one feature
each: several reference pictures, several slices, every block size,
cropped sizes, very low and very high quality, the deblocking filter
off and with its settings changed, intra refresh. All matched ffmpeg
on every picture.

The window is tested on Linux with Xvfb, an X server that draws into
memory, when it is installed: a program opens a window, draws, polls
and closes, and the player plays a clip to its end. On Windows the
same checks run against the real desktop.

## What is not there

- **Other profiles of H.264.** Most MP4 files from cameras, phones and
  the web use the Main or High profile, which add a second way of
  coding the bits (CABAC), pictures predicted from two others (B
  pictures) and an 8x8 transform. The decoder says which of these it
  met and stops: `h264: CABAC entropy coding (Main and High profile) is
  not supported`. To try the player on such a file, convert it first:
  `ffmpeg -i in.mp4 -c:v libx264 -profile:v baseline -an out.mp4`.
- **Other codecs** (H.265, AV1, VP9). `mp4.open` reads the file and
  names the codec; nothing decodes it.
- **Sound.** The player says when a file has a sound track and does
  not play it.
- **Interlaced video, and fragmented MP4 files** (the kind streaming
  sites send in pieces).
- **macOS.** `omg` goes through C there, and the window builtins exist
  only in the machine-code runtime.
- On Windows the video stops while the window is being dragged or
  resized, and goes on when the mouse is let go.
