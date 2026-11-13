#!/bin/bash
# The video tools in tools/video (an MP4 reader, an H.264 decoder, an
# AAC decoder, a player) and the window and sound builtins the player
# shows and plays with.
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

# === The sound track ==========================================================
section "Video: AAC decoder (tools/video/aac.omg)"

# AAC decoding is not exact the way H.264 is: decoders may differ in the
# last bit of a sample. The checksums beside the clips are the ones this
# decoder gives, kept after its samples had been compared with ffmpeg's
# (none differed by more than 1; tests/video/README.md says how). What
# is checked here is that every way of running OMG gives those same
# samples: the decoder is all floating point, and uses nothing but
# + - * / and sqrt, which every machine rounds the same way.
"$OMG_NATIVE" --build tools/video/aacinfo.omg "$TMPDIR_TEST/aacinfo" >/dev/null 2>&1
if is_executable "$TMPDIR_TEST/aacinfo"; then
    pass "aacinfo builds"
else
    fail "aacinfo builds"
fi

assert_eq "mp4info describes the sound track too" \
    "# video: avc1 82x66, 12 frames, 0.800 s, 15.0 frames a second
# audio: mp4a 48000 Hz, 2 channels, 39 blocks, 0.821 s" \
    "$("$TMPDIR_TEST/mp4info" tests/video/sound.mp4 2>&1 | tr -d '\r')"
want=$(tr -d '\r' < tests/video/sound.mp4.frames)
assert_eq "the pictures of a file with sound decode as ffmpeg's do" "$want" \
    "$(frames_of "$TMPDIR_TEST/mp4info" tests/video/sound.mp4 --frames)"

# Two channels at 48000 Hz: a rising tone with bursts of noise (which
# make the encoder switch to short blocks) on the left, a tone that
# starts and stops on the right.
want=$(tr -d '\r' < tests/video/sound.mp4.sound)
assert_eq "stereo AAC decodes to the expected samples (built)" "$want" \
    "$(frames_of "$TMPDIR_TEST/aacinfo" tests/video/sound.mp4)"

# One channel at 22050 Hz, in a file with no video: decaying notes over
# noise, coded with noise substitution.
want=$(tr -d '\r' < tests/video/mono.m4a.sound)
assert_eq "mono AAC decodes to the expected samples (built)" "$want" \
    "$(frames_of "$TMPDIR_TEST/aacinfo" tests/video/mono.m4a)"
assert_eq "mono AAC decodes to the expected samples (interpreted)" "$want" \
    "$(frames_of "$OMG_NATIVE" tools/video/aacinfo.omg tests/video/mono.m4a)"
if [ -n "$(omg_c_compiler)" ]; then
    "$OMG_NATIVE" --build tools/video/aacinfo.omg "$TMPDIR_TEST/aacinfo-c" --target c >/dev/null 2>&1
    assert_eq "mono AAC decodes to the expected samples (through C)" "$want" \
        "$(frames_of "$TMPDIR_TEST/aacinfo-c" tests/video/mono.m4a)"
fi
if command -v node >/dev/null 2>&1; then
    "$OMG_NATIVE" --emit-js tools/video/aacinfo.omg "$TMPDIR_TEST/aacinfo.js" >/dev/null 2>&1
    assert_eq "mono AAC decodes to the expected samples (as JavaScript)" "$want" \
        "$(frames_of node "$TMPDIR_TEST/aacinfo.js" tests/video/mono.m4a)"
fi

out=$("$TMPDIR_TEST/aacinfo" tests/video/clip.mp4 2>&1); code=$?
assert_contains "a file without sound is refused by name" "there is no AAC sound track to decode" "$out"
assert_eq "and the exit code says it failed" "1" "$code"

echo "this is not a video" > "$TMPDIR_TEST/not-a-video.mp4"
out=$("$TMPDIR_TEST/mp4info" "$TMPDIR_TEST/not-a-video.mp4" 2>&1); code=$?
assert_contains "a file that is not MP4 is refused by name" "does not look like an MP4 file" "$out"
assert_eq "and the exit code says it failed" "1" "$code"

# === The sound builtins ======================================================
section "Video: audio_open / audio_write / audio_played / audio_pause / audio_close"

cat > "$TMPDIR_TEST/audio-errors.omg" <<'EOF'
try { audio_write(1, [1, 2], 0) } except e { emit e }
try { audio_played(1) } except e { emit e }
try { audio_pause(1, true) } except e { emit e }
try { audio_close(1) } except e { emit e }
try { audio_open(100, 2) } except e { emit e }
try { audio_open(48000, 3) } except e { emit e }
try { audio_open("x", 2) } except e { emit e }
EOF
audio_errors_want="ValueError: audio_write(): no such sound stream
ValueError: audio_played(): no such sound stream
ValueError: audio_pause(): no such sound stream
ValueError: audio_close(): no such sound stream
ValueError: audio_open: the rate must be between 4000 and 192000 a second
ValueError: audio_open: the channels must be 1 or 2
TypeError: audio_open() expects (rate: int, channels: int)"

cat > "$TMPDIR_TEST/audio-none.omg" <<'EOF'
try {
    audio_open(8000, 1)
    emit "opened"
} except e {
    emit e
}
EOF

# Two seconds of quiet sound, start to finish. Where there is nothing to
# play sound on, the first line says so and the program stops there.
cat > "$TMPDIR_TEST/audio-play.omg" <<'EOF'
alloc a := 0
try {
    a := audio_open(8000, 1)
} except e {
    emit "no device: " + e
    exit(0)
}
emit "open " + a
try { audio_open(8000, 1) } except e { emit e }
try { audio_write(a, [1, "x"], 0) } except e { emit e }
try { audio_write(a, [1, 2], 5) } except e { emit e }
try { audio_pause(a, 1) } except e { emit e }
emit audio_write(a, [], 0)

# a second of sound: a slow ramp, quiet
alloc n := 8000
alloc pcm := list_repeat(0, n)
alloc i := 0
loop i < n {
    pcm[i] := i % 200 - 100
    i := i + 1
}
# The count of what was heard never runs ahead of what was written,
# never goes back, and arrives at all of it.
alloc at := 0
alloc bad := 0
alloc last := 0
alloc t0 := time_ms()
loop audio_played(a) < n and time_ms() - t0 < 20000 {
    at := at + audio_write(a, pcm, at)
    alloc p := audio_played(a)
    if p < last or p > at {
        bad := bad + 1
    }
    last := p
    sleep_ms(5)
}
emit "written " + at + ", heard " + audio_played(a) + ", wrong counts " + bad

# another second; part of the way through it is held, and let go again
at := 0
t0 := time_ms()
loop audio_played(a) < n + 2000 and time_ms() - t0 < 20000 {
    at := at + audio_write(a, pcm, at)
    sleep_ms(5)
}
audio_pause(a, true)
sleep_ms(150)
alloc p1 := audio_played(a)
sleep_ms(300)
alloc p2 := audio_played(a)
emit "held: moved on by " + (p2 - p1) + ", heard all " + (p2 >= 2 * n)
audio_pause(a, false)
t0 := time_ms()
loop audio_played(a) < 2 * n and time_ms() - t0 < 20000 {
    at := at + audio_write(a, pcm, at)
    sleep_ms(5)
}
emit "let go: written " + at + ", heard " + (audio_played(a) - n)
emit audio_close(a)
try { audio_played(a) } except e { emit e }
a := audio_open(44100, 2)
emit "open " + a
emit audio_close(a)
EOF
audio_play_want="open 1
ValueError: audio_open: a sound stream is already open (one at a time)
TypeError: audio_write() expects every sample to be an integer
ValueError: audio_write: \`from\` is outside the samples
TypeError: audio_pause() expects (audio, on: bool)
0
written 8000, heard 8000, wrong counts 0
held: moved on by 0, heard all false
let go: written 8000, heard 8000

ValueError: audio_played(): no such sound stream
open 1
"

target=$(omg_default_target)
sound_pid=""
sound_server=""
if [ "$target" = "linux-x64" ] || [ "$target" = "windows-x64" ]; then
    assert_eq "the sound builtins refuse what makes no sense (interpreted)" "$audio_errors_want" \
        "$("$OMG_NATIVE" "$TMPDIR_TEST/audio-errors.omg" 2>&1 | tr -d '\r')"
    "$OMG_NATIVE" --build "$TMPDIR_TEST/audio-errors.omg" "$TMPDIR_TEST/audio-errors-built" >/dev/null 2>&1
    assert_eq "the sound builtins refuse what makes no sense (built)" "$audio_errors_want" \
        "$("$TMPDIR_TEST/audio-errors-built" 2>&1 | tr -d '\r')"
    "$OMG_NATIVE" --build "$TMPDIR_TEST/audio-play.omg" "$TMPDIR_TEST/audio-play-built" >/dev/null 2>&1
fi
if [ "$target" = "windows-x64" ]; then
    # A machine without a sound card (the ones tests run on, often) has
    # nothing to open; one with a card plays the two seconds.
    out=$("$TMPDIR_TEST/audio-play-built" 2>&1 | tr -d '\r')
    case "$out" in
        "no device: ValueError: audio_open: Windows has no device to play sound on")
            pass "without a sound device, audio_open says so" ;;
        *)
            assert_eq "sound is written, counted, held and let go (built)" "$audio_play_want" "$out"$'\n' ;;
    esac
elif [ "$target" = "linux-x64" ]; then
    # From here on the tests keep away from the sound server of whoever
    # runs them: nobody wants test tones from their loudspeakers.
    unset PULSE_SERVER PULSE_RUNTIME_PATH PULSE_COOKIE
    export XDG_RUNTIME_DIR="$TMPDIR_TEST/no-sound-here"
    assert_eq "without a sound server, audio_open says what is missing" \
        'ValueError: audio_open: no sound server answers (PulseAudio or PipeWire, at $XDG_RUNTIME_DIR/pulse/native or $PULSE_SERVER)' \
        "$("$OMG_NATIVE" "$TMPDIR_TEST/audio-none.omg" 2>&1)"
    # With PulseAudio installed, a server of our own is started that
    # plays into nothing (a "null sink"), and the builtins are tried for
    # real against it. Without, those checks are skipped.
    if command -v pulseaudio >/dev/null 2>&1; then
        mkdir -p "$TMPDIR_TEST/sound/home"
        chmod 700 "$TMPDIR_TEST/sound"
        HOME="$TMPDIR_TEST/sound/home" XDG_RUNTIME_DIR="$TMPDIR_TEST/sound" \
            pulseaudio --daemonize=no --exit-idle-time=-1 --system=false -n \
            --load="module-native-protocol-unix auth-anonymous=1" \
            --load="module-null-sink sink_name=omgtest rate=48000" >/dev/null 2>&1 &
        sound_pid=$!
        for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30; do
            [ -S "$TMPDIR_TEST/sound/pulse/native" ] && break
            sleep 0.1
        done
        if [ -S "$TMPDIR_TEST/sound/pulse/native" ] && kill -0 "$sound_pid" 2>/dev/null; then
            sound_server="$TMPDIR_TEST/sound"
            export XDG_RUNTIME_DIR="$sound_server"
            assert_eq "sound is written, counted, held and let go (interpreted)" "$audio_play_want" \
                "$("$OMG_NATIVE" "$TMPDIR_TEST/audio-play.omg" 2>&1)"$'\n'
            assert_eq "sound is written, counted, held and let go (built)" "$audio_play_want" \
                "$("$TMPDIR_TEST/audio-play-built" 2>&1)"$'\n'
        else
            kill "$sound_pid" 2>/dev/null
            sound_pid=""
            echo "  (sound checks skipped: PulseAudio did not start)"
        fi
    else
        echo "  (sound checks skipped: they need PulseAudio, to play into nothing)"
    fi
else
    echo "  (sound checks skipped: this omg builds through C, which has no sound)"
fi

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
    # A file with sound: it is played where sound can be, and where it
    # cannot the player says so and shows the pictures all the same.
    out=$("$TMPDIR_TEST/play" tests/video/sound.mp4 2>&1 | tr -d '\r')
    assert_contains "the player plays a file with sound to its end" "pictures shown" "$out"
    if [ -n "$sound_server" ]; then
        assert_contains "and plays its sound" "sound: AAC, 48000 Hz, two channels" "$out"
        case "$out" in
            *"the sound was turned off"*) fail "and keeps the sound to the end" ;;
            *) pass "and keeps the sound to the end" ;;
        esac
        assert_contains "a file with sound only is played without a window" "s of sound played" \
            "$("$TMPDIR_TEST/play" tests/video/mono.m4a 2>&1)"
    else
        case "$out" in
            *"sound: AAC, 48000 Hz, two channels"*|*"(no sound: "*) pass "and says what became of its sound" ;;
            *) fail "and says what became of its sound" ;;
        esac
    fi
    assert_contains "--mute plays without sound" "pictures shown" \
        "$("$TMPDIR_TEST/play" tests/video/sound.mp4 --mute 2>&1)"
}

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

# The same goes for sound.
if [ -n "$(omg_c_compiler)" ]; then
    "$OMG_NATIVE" --build "$TMPDIR_TEST/audio-none.omg" "$TMPDIR_TEST/audio-none-c" --target c >/dev/null 2>&1
    assert_contains "through C, audio_open raises an error that can be caught" \
        "ValueError: audio_open is not available in a program built through C" \
        "$("$TMPDIR_TEST/audio-none-c" 2>&1)"
fi
if command -v node >/dev/null 2>&1; then
    "$OMG_NATIVE" --emit-js "$TMPDIR_TEST/audio-none.omg" "$TMPDIR_TEST/audio-none.js" >/dev/null 2>&1
    assert_contains "as JavaScript, audio_open raises an error that can be caught" \
        "ValueError: audio_open is not supported in the JS runtime" \
        "$(node "$TMPDIR_TEST/audio-none.js" 2>&1)"
fi

if [ -n "$sound_pid" ]; then
    kill "$sound_pid" 2>/dev/null
    wait "$sound_pid" 2>/dev/null
fi
