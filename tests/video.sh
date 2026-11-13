#!/bin/bash
# The video tools in tools/video (an MP4 reader, an H.264 decoder, a
# player) and the window builtins the player draws with.
#
# H.264 decoding is exact: every decoder must produce the same pictures,
# bit for bit. So the referee for the decoder is ffmpeg. The clips in
# tests/video were made with it, and beside each is the checksum
# ffmpeg computes for every picture (see tests/video/README.md). ffmpeg
# is not needed to run the tests.

set -u
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
require_native_toolchain

cd "$REPO_ROOT"

# What mp4info prints, without the lines that give times.
frames_of() {
    "$@" 2>&1 | tr -d '\r' | grep -v '^#'
}

section "Video: MP4 reader and H.264 decoder (tools/video)"

"$OMG_NATIVE" --build tools/video/mp4info.omg "$TMPDIR_TEST/mp4info" >/dev/null 2>&1
if is_executable "$TMPDIR_TEST/mp4info"; then
    pass "mp4info builds"
else
    fail "mp4info builds"
fi

assert_eq "mp4info describes a file" \
    "# video: avc1 82x66, 20 frames, 1.333 s, 15.0 frames a second" \
    "$("$OMG_NATIVE" tools/video/mp4info.omg tests/video/clip.mp4 2>&1 | tr -d '\r')"

# 20 pictures, two of them complete in themselves, the rest built from
# up to three earlier ones; two slices each; 82x66 is not a whole number
# of 16x16 blocks, so the pictures are cropped.
want=$(tr -d '\r' < tests/video/clip.mp4.frames)
assert_eq "an MP4 file decodes to the pictures ffmpeg gets (built)" "$want" \
    "$(frames_of "$TMPDIR_TEST/mp4info" tests/video/clip.mp4 --frames)"
assert_eq "an MP4 file decodes to the pictures ffmpeg gets (interpreted)" "$want" \
    "$(frames_of "$OMG_NATIVE" tools/video/mp4info.omg tests/video/clip.mp4 --frames)"

# A raw stream: no intra pictures after the first (the encoder refreshes
# a column at a time), and its own deblocking settings.
want=$(tr -d '\r' < tests/video/stream.h264.frames)
assert_eq "a raw H.264 stream decodes to the pictures ffmpeg gets (built)" "$want" \
    "$(frames_of "$TMPDIR_TEST/mp4info" tests/video/stream.h264 --frames)"
assert_eq "a raw H.264 stream decodes to the pictures ffmpeg gets (interpreted)" "$want" \
    "$(frames_of "$OMG_NATIVE" tools/video/mp4info.omg tests/video/stream.h264 --frames)"

# Main profile: arithmetic coding (CABAC), B pictures, which are built
# from a picture before and a picture after and so are stored out of
# order, and weighted prediction (the clip fades in).
want=$(tr -d '\r' < tests/video/main.mp4.frames)
assert_eq "a Main profile file decodes to the pictures ffmpeg gets" "$want" \
    "$(frames_of "$TMPDIR_TEST/mp4info" tests/video/main.mp4 --frames)"

# High profile: 8x8 transforms and 8x8 intra prediction, here with the
# other entropy coder (CAVLC), the encoder's own quantisation weights,
# and B pictures that other B pictures are built from.
want=$(tr -d '\r' < tests/video/high.mp4.frames)
assert_eq "a High profile file decodes to the pictures ffmpeg gets" "$want" \
    "$(frames_of "$TMPDIR_TEST/mp4info" tests/video/high.mp4 --frames)"

# A raw High profile stream with B pictures: there is no file around it
# to say when each picture is shown, so the decoder must put them in
# order itself.
want=$(tr -d '\r' < tests/video/bframes.h264.frames)
assert_eq "a raw stream with B pictures comes out in the order it is shown" "$want" \
    "$(frames_of "$TMPDIR_TEST/mp4info" tests/video/bframes.h264 --frames)"

echo "this is not a video" > "$TMPDIR_TEST/not-a-video.mp4"
out=$("$TMPDIR_TEST/mp4info" "$TMPDIR_TEST/not-a-video.mp4" 2>&1); code=$?
assert_contains "a file that is not MP4 is refused by name" "does not look like an MP4 file" "$out"
assert_eq "and the exit code says it failed" "1" "$code"

# === The window builtins ====================================================
section "Video: window_open / window_draw / window_poll / window_close"

cat > "$TMPDIR_TEST/window.omg" <<'EOF'
alloc w := 64
alloc h := 48
alloc win := window_open("omg test window", w, h)
emit win
alloc px := list_repeat(0, w * h)
alloc i := 0
loop i < w * h {
    px[i] := ((i % w) << 18) | ((i // w) << 10) | 255
    i := i + 1
}
emit window_draw(win, px, w)
# nobody is at the keyboard: whatever the window system reports on its
# own (a repaint, a first size) must not come out as a key or a close
alloc n := 0
alloc bad := 0
loop n < 20 {
    alloc ev := window_poll(win)
    if ev != false {
        if ev[0] != "resize" {
            bad := bad + 1
        }
    }
    sleep_ms(10)
    n := n + 1
}
emit bad
try {
    window_draw(win, [1, 2, 3], 2)
} except e {
    emit e
}
try {
    window_draw(win, [1, "two", 3, 4], 2)
} except e {
    emit e
}
emit window_close(win)
try {
    window_poll(win)
} except e {
    emit e
}
EOF
window_want="1

0
ValueError: window_draw: the pixels do not make whole rows of the given width
TypeError: window_draw() expects every pixel to be an integer

ValueError: window_poll(): no such window"

cat > "$TMPDIR_TEST/window-none.omg" <<'EOF'
try {
    window_open("nowhere", 10, 10)
    emit "opened"
} except e {
    emit e
}
EOF

window_checks() {
    assert_eq "a window opens, takes a picture and closes (interpreted)" "$window_want" \
        "$("$OMG_NATIVE" "$TMPDIR_TEST/window.omg" 2>&1 | tr -d '\r')"
    "$OMG_NATIVE" --build "$TMPDIR_TEST/window.omg" "$TMPDIR_TEST/window-built" >/dev/null 2>&1
    assert_eq "a window opens, takes a picture and closes (built)" "$window_want" \
        "$("$TMPDIR_TEST/window-built" 2>&1 | tr -d '\r')"
    # The player, start to finish: --fast shows every picture without
    # waiting for its time.
    "$OMG_NATIVE" --build tools/video/play.omg "$TMPDIR_TEST/play" >/dev/null 2>&1
    assert_contains "the player plays a file to its end (built)" "20 pictures shown" \
        "$("$TMPDIR_TEST/play" tests/video/clip.mp4 --fast 2>&1)"
    assert_contains "the player plays a file to its end (interpreted)" "20 pictures shown" \
        "$("$OMG_NATIVE" tools/video/play.omg tests/video/clip.mp4 --fast 2>&1)"
    # B pictures are stored out of order: all 18 must still be shown. And
    # without --fast the player waits for each picture's time.
    assert_contains "the player plays a file with B pictures" "18 pictures shown" \
        "$("$TMPDIR_TEST/play" tests/video/high.mp4 --fast 2>&1)"
    # (a busy machine may leave pictures out, so only the time is checked:
    # the clip lasts 1.2 seconds)
    secs=$("$TMPDIR_TEST/play" tests/video/main.mp4 2>&1 | sed -n 's/.* pictures shown in \([0-9]*\)\..*/\1/p')
    if [ -n "$secs" ] && [ "$secs" -ge 1 ]; then
        pass "the player keeps the file's time"
    else
        fail "the player keeps the file's time"
    fi
}

target=$(omg_default_target)
if [ "$target" = "windows-x64" ]; then
    window_checks
elif [ "$target" = "linux-x64" ]; then
    assert_eq "without DISPLAY, window_open says what is missing" \
        "ValueError: window_open: DISPLAY is not set, so there is no X server to show a window on" \
        "$(env -u DISPLAY "$OMG_NATIVE" "$TMPDIR_TEST/window-none.omg" 2>&1)"
    # A window needs a display. Xvfb is an X server that draws into
    # memory; with it the checks run anywhere, and without it they are
    # skipped (the display of whoever runs the tests is left alone).
    if command -v Xvfb >/dev/null 2>&1; then
        xvfb_pid=""
        for n in 91 92 93 94 95 96 97 98; do
            [ -e "/tmp/.X11-unix/X$n" ] && continue
            Xvfb ":$n" -screen 0 640x480x24 -nolisten tcp >/dev/null 2>&1 &
            xvfb_pid=$!
            for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
                [ -e "/tmp/.X11-unix/X$n" ] && break
                sleep 0.1
            done
            if [ -e "/tmp/.X11-unix/X$n" ] && kill -0 "$xvfb_pid" 2>/dev/null; then
                break
            fi
            kill "$xvfb_pid" 2>/dev/null
            xvfb_pid=""
        done
        if [ -n "$xvfb_pid" ]; then
            saved_display="${DISPLAY-}"
            saved_xauthority="${XAUTHORITY-}"
            export DISPLAY=":$n"
            unset XAUTHORITY
            window_checks
            kill "$xvfb_pid" 2>/dev/null
            wait "$xvfb_pid" 2>/dev/null
            if [ -n "$saved_display" ]; then export DISPLAY="$saved_display"; else unset DISPLAY; fi
            [ -n "$saved_xauthority" ] && export XAUTHORITY="$saved_xauthority"
        else
            echo "  (window checks skipped: Xvfb did not start)"
        fi
    else
        echo "  (window checks skipped: they need Xvfb, an X server without a screen)"
    fi
else
    echo "  (window checks skipped: this omg builds through C, which has no window)"
fi

# A program built through C, or run as JavaScript, has the four names
# and an error that says where a window can be had.
if [ -n "$(omg_c_compiler)" ]; then
    "$OMG_NATIVE" --build "$TMPDIR_TEST/window-none.omg" "$TMPDIR_TEST/window-none-c" --target c >/dev/null 2>&1
    assert_contains "through C, window_open raises an error that can be caught" \
        "ValueError: window_open is not available in a program built through C" \
        "$("$TMPDIR_TEST/window-none-c" 2>&1)"
fi
if command -v node >/dev/null 2>&1; then
    "$OMG_NATIVE" --emit-js "$TMPDIR_TEST/window-none.omg" "$TMPDIR_TEST/window-none.js" >/dev/null 2>&1
    assert_contains "as JavaScript, window_open raises an error that can be caught" \
        "ValueError: window_open is not supported in the JS runtime" \
        "$(node "$TMPDIR_TEST/window-none.js" 2>&1)"
fi
