# Packaging the native distribution

`bootstrap/package.sh` produces a slim, Rust-free copy of the OMG
native toolchain at `dist/omglang-native/`. The output mirrors the
layout of the standalone
[omglang-native](https://github.com/sentrychris/omglang-native)
companion repo: `src/` for the OMG sources + C runtime header, `bin/`
for the pre-built ELFs, plus `examples/`, `tools/`, `tests/`, and
`docs/` for offline use.

## When to use it

The parent `omglang/` repo is the active development workspace — Rust
runtime, OMG sources, build scripts, tests for both backends, and
docs all live together. The native distribution is what you ship to
someone who:

- doesn't want to install Rust,
- wants a small standalone tarball they can drop on a server,
- is mirroring the `omglang-native` companion repo from this one
  (the script's output is byte-for-byte the structure that repo expects).

## Prerequisites

Build the Rust runtime once (only needed to seed `bootstrap/bin/` for
first-time use; on subsequent runs the script reuses whatever is
already in `bootstrap/bin/`):

```sh
cargo build --release --manifest-path runtime/Cargo.toml
bootstrap/build.sh
```

After that, `bootstrap/bin/` contains the four native ELFs (omg, omgc,
omgcc, omgjs) plus the inlined runtime headers (omg_rt.h, omg_rt.js).
You can package them at any time without touching Rust again.

## Running it

```sh
bootstrap/package.sh                   # populate dist/omglang-native/
bootstrap/package.sh --clean           # wipe dist/omglang-native/ first
bootstrap/package.sh --tarball         # also produce dist/omglang-native.tar.gz
bootstrap/package.sh --help            # show flags
```

The script is idempotent: re-running it overwrites the previous
contents of `dist/omglang-native/`. Use `--clean` if you've removed
files from the source tree and want a guaranteed-fresh output.

`dist/` is gitignored.

## What the script does

```
bootstrap/src/  ──┬─►  dist/omglang-native/src/    (OMG source + omg_rt.h)
bootstrap/bin/  ──┴─►  dist/omglang-native/bin/    (pre-built ELFs)
examples/       ────►  dist/omglang-native/examples/
tools/          ────►  dist/omglang-native/tools/
tests/          ──[1]►  dist/omglang-native/tests/
docs/native/    ──[2]►  dist/omglang-native/docs/
                ──[3]►  dist/omglang-native/build.sh   (slim self-rebuild)
                ──[4]►  dist/omglang-native/README.md  (slim, native edition)
```

[1] **tests/** — paths are rewritten so `bootstrap/bin/` becomes `bin/`
and `bootstrap/src/` becomes `src/`. The Rust-runtime checks in
`lib.sh`, `parity.sh`, and `regression.sh` are stripped, since the
distribution has no Rust binary to compare against. What remains is a
native-only test suite: AOT vs interpreted parity, builtins, REPL,
driver modes, and regression cases that don't need Rust.

[2] **docs/** — paths are rewritten the same way.
`runtime/target/release/omg` becomes `bin/omg`; the `cargo build`
prerequisite step becomes `./build.sh`.

[3] **build.sh** is generated from a template inside `package.sh`. It's
the self-rebuild path of `bootstrap/build.sh` with the Rust bootstrap
branch removed: the dist always has `bin/omgc` + `bin/omgcc`
pre-built, so it never needs the Rust runtime to rebuild itself.

[4] **README.md** is also generated from a template. It's the
native-edition README pointing at the parent repo for the full
project.

## What's deliberately left out

- `examples/self_hosted.omg` — references the parent repo's
  `bootstrap/src/compiler.omg` path, so it wouldn't run as-is in the
  dist.
- `runtime/`, `reference/`, `vscode/` — Rust, Python, and VS Code
  extension respectively. Not part of the native pipeline.

## Verifying the output

After packaging, sanity-check the dist runs end-to-end:

```sh
dist/omglang-native/bin/omg dist/omglang-native/examples/hello_world.omg
dist/omglang-native/bin/omg --build dist/omglang-native/examples/hello_world.omg /tmp/h
/tmp/h
dist/omglang-native/tests/run.sh
```

A passing test suite means the dist is shippable.

## Updating the omglang-native companion repo

If you maintain the standalone
[omglang-native](https://github.com/sentrychris/omglang-native) repo,
the typical workflow is:

```sh
# In omglang/
bootstrap/build.sh                     # ensure bin/ is fresh
bootstrap/package.sh --clean           # regenerate dist/

# Sync the dist into the companion repo (rsync preserves permissions)
rsync -a --delete dist/omglang-native/ ../omglang-native/

# Then in ../omglang-native/
git status                             # review what changed
git add -A && git commit -m "sync from omglang"
```

The `bootstrap/package.sh` script is the single source of truth for
that repo's layout — anything that diverges between the two should be
fixed by updating the script, not by hand-editing the companion repo.
