# 01 · Quickstart

You'll be running and compiling OMG programs in five minutes.

## Prerequisites

- A C compiler (`cc` — basically any Linux/macOS box has one; on
  Windows, GCC or Clang from MinGW-w64)
- Rust + Cargo (only for the **first-time** bootstrap; never again after that)

On Windows the commands on this page are `bin\omg.exe ...`
and the build script is `scripts\build.ps1`.
[08-windows.md](08-windows.md) has the Windows specifics.

## First-time bootstrap

```sh
# Build the Rust runtime (used once to seed the native toolchain)
cd rust && cargo build --release && cd ..

# Build the OMG-native toolchain (one binary, omg) into bin/
scripts/build.sh
```

You'll see three steps print: pick a compiler, install the runtime
files, build `omg` (OMG source → C → ELF). Takes a couple of minutes,
most of it the C compiler. After this, `bin/` has everything
you need. (On Linux x86-64 and Windows x86-64 a working native binary
is already checked in, `bin/omg` and `bin\omg.exe`,
so you can skip both commands.)

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
omg --build foo.omg foo   # AOT-compile to a standalone ELF binary
./foo                     # Run the ELF directly
```

(`omg` here is `bin/omg`. See [README.md](README.md#conventions-in-these-docs).)

| Mode      | What happens                              | Output         | When to use            |
| --------- | ----------------------------------------- | -------------- | ---------------------- |
| `omg foo.omg`  | Compile + run in-process (no temp files)      | runs   | dev loop               |
| `omg --compile foo.omg foo.omgb` | Save bytecode             | `.omgb` file   | distribute portable bc |
| `omg --build foo.omg foo`        | Full AOT                  | native ELF     | ship a binary          |

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

The resulting `calc` is ~30 KB, statically self-contained (apart from libc),
and runs in any directory with no Rust anywhere in sight.

## Cheat sheet

```sh
# Run without building
omg foo.omg [args...]

# Build to ELF
omg --build foo.omg [out_name]

# Just compile to bytecode (skip execution)
omg --compile foo.omg foo.omgb

# Disassemble bytecode or .omg source (in-process; matches Rust output)
omg --disasm foo.omgb
omg --disasm foo.omg

# Rebuild the native toolchain (after you've changed compiler.omg etc.)
scripts/build.sh
```

## Where to go next

- New to OMG as a **language**? → [03-language-tour.md](03-language-tour.md)
- Want to know **how this all works**? → [02-architecture.md](02-architecture.md)
- Something **broke**? → [07-debugging.md](07-debugging.md)
