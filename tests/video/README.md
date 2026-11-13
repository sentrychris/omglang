# Test clips for tools/video

Small H.264 videos, and for each the checksum of every decoded
picture; and two clips with AAC sound ([below](#sound)). `tests/video.sh` decodes them with `tools/video/h264.omg` and
compares.

The clips and the checksums both come from ffmpeg, so the referee is a
decoder other than the one being tested. H.264 decoding is exact:
every correct decoder gives the same pictures.

| File | What it is |
| ---- | ---------- |
| `clip.mp4` | 82x66 (so the pictures are cropped), 20 pictures, an I picture every 10, up to 3 reference pictures, 2 slices a picture |
| `stream.h264` | a raw stream, 64x48, 8 pictures, intra refresh instead of I pictures, constrained intra prediction, deblocking offsets |
| `main.mp4` | Main profile: 80x64, 18 pictures, CABAC, B pictures, a fade-in coded with weighted prediction |
| `high.mp4` | High profile: 80x64, 18 pictures, CAVLC, 8x8 transforms, the JVT quantisation matrices, B pictures used as references, motion derived in time, 2 slices a picture |
| `bframes.h264` | a raw High profile stream, 64x48, 12 pictures, CABAC, B pictures: the decoder has to put the pictures in the order they are shown |
| `*.frames` | per picture: its number, and the Adler-32 checksum of its Y, U and V planes |

How they were made (ffmpeg 6.1 with libx264):

```sh
ffmpeg -f lavfi -i "testsrc2=size=82x66:rate=15" -frames:v 20 \
    -c:v libx264 -profile:v baseline -pix_fmt yuv420p \
    -g 10 -refs 3 -slices 2 -crf 30 -movflags +faststart clip.mp4

ffmpeg -f lavfi -i "mandelbrot=size=64x48:rate=10" -frames:v 8 \
    -c:v libx264 -profile:v baseline -pix_fmt yuv420p \
    -x264-params "intra-refresh=1:constrained-intra=1:deblock=2,-1" \
    -crf 26 -bsf:v h264_mp4toannexb -f h264 stream.h264

ffmpeg -f lavfi -i "testsrc2=size=80x64:rate=15" -frames:v 18 -vf "fade=in:0:12" \
    -c:v libx264 -profile:v main -pix_fmt yuv420p -g 12 -bf 2 -refs 3 \
    -x264-params "weightp=2:weightb=1:direct=auto:b-pyramid=none" \
    -crf 28 -movflags +faststart main.mp4

ffmpeg -f lavfi -i "mandelbrot=size=80x64:rate=15" -frames:v 18 \
    -c:v libx264 -profile:v high -coder 0 -pix_fmt yuv420p -bf 3 -refs 3 -slices 2 \
    -x264-params "direct=temporal:b-pyramid=normal:cqm=jvt:weightp=0" \
    -crf 24 -movflags +faststart high.mp4

ffmpeg -f lavfi -i "mandelbrot=size=64x48:rate=10" -frames:v 12 \
    -c:v libx264 -profile:v high -pix_fmt yuv420p -bf 2 -g 6 \
    -x264-params "b-pyramid=none:open-gop=0" \
    -crf 22 -bsf:v h264_mp4toannexb -f h264 bframes.h264

# the checksums: the sixth column of framecrc, as a decimal number
ffmpeg -i clip.mp4 -an -fps_mode passthrough -f framecrc - | grep -v '^#' |
    gawk -F, '{ printf "%d %d\n", NR - 1, strtonum($6) }' > clip.mp4.frames
```

To check a clip of your own the same way:

```sh
omg tools/video/mp4info.omg your.mp4 --frames
```

## Sound

| File | What it is |
| ---- | ---------- |
| `sound.mp4` | 82x66, 12 pictures, with a sound track: AAC-LC, two channels at 48000 Hz, 39 blocks. Left a rising tone with bursts of noise (so the encoder switches to short blocks), right a tone that starts and stops |
| `mono.m4a` | sound only: AAC-LC, one channel at 22050 Hz, 14 blocks, decaying notes over noise, coded with noise substitution |
| `*.sound` | per block of sound: its number, and the Adler-32 checksum of its samples as 16-bit little-endian bytes |

AAC decoding is not exact as H.264 decoding is: two correct decoders
may differ in the last bit of a sample. So the `.sound` files hold what
`tools/video/aac.omg` itself gives (`tools/video/aacinfo.omg` prints
them), and `tests/video.sh` checks that every back end reproduces
them. That they are right was checked once, against ffmpeg, allowing
for that last bit:

```sh
ffmpeg -f lavfi -i "testsrc2=size=82x66:rate=15" \
    -f lavfi -i "aevalsrc=0.4*sin(2*PI*(300+900*t)*t)+0.3*random(0)*lt(mod(t\,0.2)\,0.02)|0.4*sin(2*PI*440*t)*gt(mod(t\,0.25)\,0.12):s=48000" \
    -t 0.8 -c:v libx264 -profile:v baseline -pix_fmt yuv420p -g 10 -crf 32 \
    -c:a aac -b:a 96k -movflags +faststart sound.mp4

ffmpeg -f lavfi -i "aevalsrc=0.5*sin(2*PI*220*t)*exp(-3*mod(t\,0.3))+0.1*random(1):s=22050" \
    -t 0.6 -c:a aac -b:a 32k -aac_pns 1 mono.m4a

# the comparison: both decoders' samples, which may differ by 1 at most
omg tools/video/aacinfo.omg sound.mp4 --pcm ours.raw
ffmpeg -ignore_editlist 1 -i sound.mp4 -vn -f s16le theirs.raw
python3 -c "
import numpy as np
a = np.fromfile('ours.raw', '<i2').astype(int)
b = np.fromfile('theirs.raw', '<i2').astype(int)
print(len(a), len(b), abs(a - b).max())"     # 79872 79872 1
```
