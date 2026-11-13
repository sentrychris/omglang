# 01 · Quickstart

You'll be running and compiling OMG programs in five minutes.

## Prerequisites

On Linux and Windows (x86-64): none. A native `omg` is checked into
`bin/` (`bin/omg` and `bin\omg.exe`), and it is the whole toolchain. It
runs programs, builds them into executables and rebuilds itself, with
no C compiler, no linker and no other language installed.

On Windows the commands on this page are `bin\omg.exe ...`.
[08-windows.md](08-windows.md) has the Windows specifics.

On macOS, or Linux on ARM, see [Other systems](#other-systems) below:
there OMG goes through C, so you need a C compiler (`cc`).

## Rebuilding the toolchain

You only need this after changing something under `src/`:

```sh
scripts/build.sh          # Windows PowerShell: scripts\build.ps1
```

It prints three steps: pick the `omg` that does the building (the one
in `bin/`), install the runtime files next to it, and build. The build
is `omg --build src/omg.omg`, run twice so that what ends up in `bin/`
is the new source built by itself. It takes a few seconds. On Linux it
writes `bin/omg.exe` as well, because `omg` can write a Windows
executable from anywhere.

## Hello world

```sh
cat > hello.omg <<'EOF'
;;;omg
emit "hello, world"
EOF

bin/omg hello.omg
# → hello, world
```

The `;;;omg` line at the top is the source-type marker. It's optional but
conventional — the lexer strips it if present.

## Three ways to run a program

```sh
omg foo.omg               # Compile and run (one-shot)
omg foo.omgb              # Run precompiled bytecode
omg --build foo.omg foo   # Build a standalone executable
./foo                     # Run it directly
```

(`omg` here is `bin/omg`. See [README.md](README.md#conventions-in-these-docs).)

| Mode      | What happens                              | Output         | When to use            |
| --------- | ----------------------------------------- | -------------- | ---------------------- |
| `omg foo.omg`  | Compile + run in-process (no temp files)      | runs   | dev loop               |
| `omg --compile foo.omg foo.omgb` | Save bytecode             | `.omgb` file   | distribute portable bc |
| `omg --build foo.omg foo`        | Full AOT                  | native executable | ship a binary       |

## A bigger example

```omg
;;;omg

# A small calculator. Usage: foo <a> <op> <b>
proc calc(a, op, b) {
    if op == "+"  { return a + b }
    if op == "-"  { return a - b }
    if op == "*"  { return a * b }
    if op == "/"  { return a / b }
    if op == "**" { return pow(a, b) }
    panic("unknown operator: " + op)
}

if length(args) < 4 {
    emit "usage: " + args[0] + " <a> <op> <b>"
} else {
    alloc r := calc(int(args[1]), args[2], int(args[3]))
    emit "" + args[1] + " " + args[2] + " " + args[3] + " = " + r
}
```

```sh
omg --build calc.omg calc
./calc 7 + 5
# → 7 + 5 = 12
./calc 2 '**' 10        # quote ** so the shell doesn't glob it
# → 2 ** 10 = 1024
```

The resulting `calc` is about 32 KB and depends on nothing: on Linux
it is a static ELF file that talks to the kernel directly, on Windows
a `.exe` that needs only `kernel32.dll`. Copy it to another machine
and it runs.

## Building for the other system

`--target` picks what `--build` writes. Any `omg` can write for either
system:

```sh
omg --build calc.omg calc --target windows-x64   # writes calc.exe, even on Linux
omg --build calc.omg calc --target linux-x64     # writes an ELF, even on Windows
omg --build calc.omg calc --target c             # through a C compiler instead
```

Without `--target` you get a program for the system you are on.
`--target c` generates C and hands it to `cc` (GCC or Clang from
MinGW-w64 on Windows). It is how OMG reaches systems the machine-code
back end does not cover, and a second opinion on it where it does.

## Cheat sheet

```sh
# Run without building
omg foo.omg [args...]

# Build an executable (for this system, or name another)
omg --build foo.omg [out_name]
omg --build foo.omg [out_name] --target windows-x64

# Just compile to bytecode (skip execution)
omg --compile foo.omg foo.omgb

# Disassemble bytecode or .omg source
omg --disasm foo.omgb
omg --disasm foo.omg

# Rebuild the native toolchain (after you've changed compiler.omg etc.)
scripts/build.sh
```

## Other systems

The machine-code back end covers x86-64 on Linux and Windows. Anywhere
else (macOS, Linux on ARM) `omg` is built through C, and `--build`
there goes through the C compiler too. You need a first `omg` to start
from, and there are two ways to get one.

**From generated C.** On any machine where `omg` runs, write the
toolchain out as one C file, copy it over, and compile it:

```sh
bin/omg --emit-c src/omg.omg omg.c       # on Linux or Windows
cc -O3 -w omg.c -o bin/omg -lm           # on the other system
scripts/build.sh                         # from now on it rebuilds itself
```

The file has no dependencies beyond the C library. CI does exactly
this for its macOS job, and keeps the `omg.c` it wrote as an artifact
(`omg-c`) of every run.

**From the Rust seed.** `rust/` holds the runtime OMG started from. It
is frozen (see [rust/README.md](../rust/README.md)), but for as long
as the toolchain's own source stays within the language it knew, it
still builds it:

```sh
cargo build --release --manifest-path rust/Cargo.toml
scripts/build.sh
```

A binary built on one of these systems is for that machine. Don't
commit it back over the checked-in Linux one.

## Where to go next

- New to OMG as a **language**? → [03-language-tour.md](03-language-tour.md)
- Want to know **how this all works**? → [02-architecture.md](02-architecture.md)
- Something **broke**? → [07-debugging.md](07-debugging.md)
