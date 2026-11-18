# Fixtures of the browser's tests

`tests/browser.sh` runs these. `check.omg` asks one part of the
browser one question and prints the answer; `server.omg` is a web
server that answers awkwardly on purpose.

Where the expected answers come from:

| File | What it is | Made with |
|------|------------|-----------|
| `vectors.txt` | 101 questions with answers: SHA-2, HMAC, HKDF, ChaCha20-Poly1305, AES-GCM, X25519, ECDH and ECDSA on P-256 and P-384, RSA signatures (PKCS #1 v1.5 and PSS) | Python's `hashlib` and the `cryptography` package; the first two X25519 lines are the examples of RFC 7748 |
| `pki/*.pem` | certificates only, no keys: a root (RSA 2048), an issuer under it (P-384, may sign servers only), server certificates under that (P-256), one signed by the root with RSA-PSS, one that ran out after a day, one signed by a certificate that may not sign, one with a bit flipped, one self-signed | `openssl req` and `openssl x509`, valid for a hundred years from November 2026 |
| `pages/*.png`, `*.gif` | 44x30 pictures: colour, colour with alpha, 16 colours, grey; a plain GIF and an interlaced one with a transparent colour | Pillow; the checksums in `browser.sh` are of Pillow's own pixels (with transparent pixels black) |
| `pages/*.jpg` | baseline 4:2:0, progressive 4:4:4, grey | Pillow; JPEG decoders may differ in the last bits, so the checksums are this decoder's, recorded when its pixels were within 2 of Pillow's |
| `pages/twenty.gz` | `dom.html` twenty times over, gzip -9 | Python's `gzip` |
| `pages/dom.html`, `want/dom.txt` | awkward HTML and the tree it should become | by hand, the tree read through once |
| `pages/boxes.html`, `want/boxes.txt` | boxes without text, and where each must be | by hand: every line of `boxes.txt` was worked out from the CSS rules before it was kept |
| `pages/paint.html` | borders, a gradient, round corners, a see-through disc, pictures | its checksum is the same from the Linux and the Windows build |
| `urls.txt`, `want/urls.txt` | addresses to take apart and links to resolve | by hand |

The private keys of the certificates were thrown away: nothing here
can sign anything. The handshake tests make a fresh key each run.

The layout and painting pages hold no text, because which fonts a
machine has decides how wide text is.
