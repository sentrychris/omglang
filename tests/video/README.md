# Test clips for tools/video

Two small H.264 videos, and for each the checksum of every decoded
picture. `tests/video.sh` decodes them with `tools/video/h264.omg` and
compares.

The clips and the checksums both come from ffmpeg, so the referee is a
decoder other than the one being tested. H.264 decoding is exact:
every correct decoder gives the same pictures.

| File | What it is |
| ---- | ---------- |
| `clip.mp4` | 82x66 (so the pictures are cropped), 20 pictures, an I picture every 10, up to 3 reference pictures, 2 slices a picture |
| `stream.h264` | a raw stream, 64x48, 8 pictures, intra refresh instead of I pictures, constrained intra prediction, deblocking offsets |
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

# the checksums: the sixth column of framecrc, as a decimal number
ffmpeg -i clip.mp4 -f framecrc - | grep -v '^#' |
    awk -F, '{ printf "%d %d\n", NR - 1, strtonum($6) }' > clip.mp4.frames
```

To check a clip of your own the same way:

```sh
omg tools/video/mp4info.omg your.mp4 --frames
```
