# 02 · Architecture

OMG is one language implemented in itself. Knowing which piece does
what prevents most confusion. (For a one-page list of every part and
what it is for, see [toolchain.md](toolchain.md).)

## The picture

```
       ┌──────────────────────────────────────────────────┐
       │  OMG: the language. Source files are .omg,       │
       │  bytecode files are .omgb.                       │
       │  The compiler, the VM, the back ends and the     │
       │  runtime of built programs are all OMG.          │
       └──────────────────────────────────────────────────┘
                            │
                            │ compiled to bytecode, which is then
                            ▼
       ┌────────────────────────────────────┐
       │  interpreted: src/vm.omg           │     Two ways to
       │  runs it straight away             │ ◄── execute OMG
       │  (`omg foo.omg`, the REPL)         │     bytecode.
       └────────────────────────────────────┘     Pick one.
                            or
       ┌────────────────────────────────────┐
       │  built: a back end turns it into   │     The other way:
       │  x86-64 machine code and writes    │ ◄── an executable
       │  the executable (`omg --build`)    │     with no VM in it.
       └────────────────────────────────────┘
```

Both live in the same program, `omg`. And `omg` is itself an OMG
program that went down the second path: `omg --build src/omg.omg`.

## Who does what

| Phase                    | What's running                |
| ------------------------ | ----------------------------- |
| Editing source           | A `.omg` text file            |
| `.omg` → `.omgb`         | `compiler.omg` (inside `omg`, as machine code) |
| Run: `omg foo.omg`       | `vm.omg` (inside `omg`, as machine code) interpreting the bytecode |
| Build: `omg --build`     | `native-x64.omg` (inside `omg`) turning the bytecode, plus the runtime, into an executable |
| Running what was built   | Your program as machine code, with the OMG runtime. No VM, no `omg`. |

On systems the machine-code back end does not cover (macOS, ARM) the
build row reads: `native-c.omg` writes C, and `cc` compiles it. There
`omg` itself was built the same way.

## The three back ends

| Back end | File | Output | Used for |
| -------- | ---- | ------ | -------- |
| x86-64   | `src/backends/native-x64.omg` | a Linux ELF or Windows PE executable, written directly | `omg --build` on Linux and Windows; the toolchain itself |
| C        | `src/backends/native-c.omg`   | one C file, for `cc` | `--target c`, `--emit-c`; every other system |
| JavaScript | `src/backends/native-js.omg` | one JavaScript file | `--emit-js`; the browser playground and explorer |

Each has a runtime: the code a built program needs for values, memory,
strings, dictionaries, builtins and errors.

| Runtime | Written in | Where |
| ------- | ---------- | ----- |
| for machine code | OMG | `src/runtime/x64/` (`core`, `float`, `builtins`, and `linux` or `windows`) |
| for C            | C   | `src/runtime/omg_rt.h` |
| for JavaScript   | JavaScript | `src/runtime/omg_rt.js` |

The machine-code runtime is the unusual one. It is OMG source, but in
a restricted dialect ("raw mode") in which every value is one machine
word and memory and the operating system are reached through a few
reserved names (`__peek64`, `__poke64`, `__syscall`, ...). The back
end compiles it with a simpler code generator than the one it uses for
your program, and links the two together. Nothing in an OMG executable
comes from anywhere else: no libc, no startup files, no linker.
[06-runtime.md](06-runtime.md) describes it.

## Native toolchain inventory

```
bin/
├── omg         the toolchain for Linux x86-64: run / REPL / --compile /
│               --build / --emit-c / --emit-js / --disasm, all in-process
├── omg.exe     the same, for Windows x86-64
├── x64/        the machine-code runtime, as OMG source
├── omg_rt.h    C runtime header  (pasted into every .c omg generates)
└── omg_rt.js   JS runtime        (pasted into every .js omg generates)
```

`omg` reads the runtime files from the folder it lives in. For a
machine-code build it compiles `x64/*.omg` to bytecode in memory
(about a tenth of a second) and hands the back end two programs: yours
and the runtime.

It is produced by `scripts/build.sh`, which:

1. Copies the runtime files from `src/runtime/` into `bin/`, so that
   what is generated next is built on the current runtime.
2. Runs `omg --build src/omg.omg` into a temporary folder: the driver
   compiles its own source (and everything it imports) to bytecode and
   puts that through the x86-64 back end. This is "stage 1": the new
   source, built by the old `omg`.
3. Runs stage 1 on the same source. This is the new source built by
   itself, and it is what goes into `bin/`.
4. On Linux, has stage 1 write `bin/omg.exe` as well, with
   `--target windows-x64`.

On Windows the script (or `scripts/build.ps1`, its PowerShell twin)
does steps 1 to 3 for `omg.exe`. On other systems steps 2 and 3 are
`omg --emit-c src/omg.omg omg.c` and `cc -O3 omg.c`.

The `omg` that starts step 2 is `bin/omg` itself when one that runs on
this machine is already there. Otherwise it is the frozen Rust runtime
running `omg.omg` from source, if you have built it.

## The fixed point

Run the script again and `omg` rebuilds itself to byte-for-byte the
same binary: `omg` building `omg.omg` reproduces `omg`. (In the same
checkout, that is. The binary records the full paths of its sources
for tracebacks, so a build in another directory differs in those
strings and nothing else.)

That equality is the strongest single check the project has. For the
output to come out the same, the compiler has to compile itself
correctly, the back end has to translate the compiler and itself
correctly, and the runtime has to behave while they do it. A mistake
in any of them almost always shows up as a different file, or as no
file at all. `tests/parity.sh` and `tests/standalone.sh` both rebuild
twice and compare.

A smaller version of the same idea covers the compiler alone:
`omg --compile src/compiler.omg` (the compiler as machine code) and
`omg src/omg.omg --compile src/compiler.omg` (the compiler interpreted
by `vm.omg`) must write the same bytecode.

## What's *not* OMG

- `src/runtime/omg_rt.h` — C. The runtime for `--target c`: value
  representation, refcounting, builtins, setjmp-based exception
  handling, tracebacks.
- `src/runtime/omg_rt.js` — JavaScript. The same job for programs
  generated by `omg --emit-js`, and for the browser playground.
- `rust/` — Rust. The runtime OMG was first built with: a lexer,
  parser, compiler and VM. Frozen; nothing above depends on it.

That's the whole non-OMG surface, and on Linux and Windows none of it
takes part in running, building or rebuilding anything.

## Where it came from

- **Rust was the seed.** You need *something* to start from when there
  is no compiler yet. The Rust runtime ran `compiler.omg` until
  `compiler.omg` could compile itself, and then refereed: for a long
  time every change had to produce the same bytes from both.
- **C was the first exit.** The C back end made `omg` an ordinary
  executable with no Rust in it, at the price of needing a C compiler
  for every build.
- **x86-64 is home.** With its own back end and a runtime written in
  OMG, the toolchain needs nothing but itself.

The Rust runtime is kept, frozen, as the documented way to rebuild
everything from source alone, and the C back end as the way onto
systems the machine-code one does not reach.

## Read next

- [03-language-tour.md](03-language-tour.md) — write OMG programs
- [04-pipeline.md](04-pipeline.md) — what each compilation stage does
- [05-extending.md](05-extending.md) — add features everywhere they have to go
