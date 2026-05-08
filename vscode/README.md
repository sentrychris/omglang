# OMG Language Support for VS Code

Syntax highlighting and a Language Server Protocol (LSP) server for the OMG
programming language. The whole extension is **TypeScript / Node** — no
Python dependency.

## Features

- **Syntax highlighting** for `.omg` files, including:
  - The required `;;;omg` header
  - Keywords (`alloc`, `proc`, `if/elif/else`, `loop`, `break`, `try`,
    `except`, `import`, `as`, `emit`, `facts`, `return`, `and`, `or`)
  - Built-in functions (`length`, `chr`, `ascii`, `hex`, `binary`,
    `freeze`, `panic`, `raise`, `read_file`, file I/O, etc.) highlighted
    distinctly from user functions
  - Numeric literals including binary (`0b1010`) and decimal
  - String escapes
  - Single-line `#` comments and `/** ... */` doc-blocks
  - Reserved globals (`args`, `module_file`, `current_dir`)

- **Completion**:
  - Keywords and built-ins
  - Top-level `proc`s and `alloc`s from the current file
  - Parameters of the enclosing `proc`
  - Member completion after `.` for imported namespaces
    (e.g. typing `math.` lists `math`'s exported procs/allocs)

- **Hover**: signature, inline detail, and `/** ... */` doc-block (if any).

- **Go-to-definition**: jumps to the `proc`, `alloc`, or `import as ...`
  line for the symbol under the cursor — including across imports.

- **Document outline**: top-level procs and allocs in the file.

- **File icon**: a violet "OMG" badge on `.omg` files. Visible on tabs
  unconditionally, and in the file explorer either:
  - automatically, if your active file-icon theme delegates unknown
    extensions to the language-defined icon (most don't); or
  - by selecting **OMG Icons** under
    *File → Preferences → Theme → File Icon Theme…*. This bundled icon
    theme just covers `.omg` (source) and `.omgb` (compiled bytecode);
    everything else falls back to your previous theme's defaults.

## Layout

```
vscode/
├── package.json                 # extension manifest
├── tsconfig.json                # project references → client + server
├── language-configuration.json  # comments, brackets, indentation
├── syntaxes/omg.tmLanguage.json # TextMate grammar
├── icons/
│   ├── omg.svg                  # `.omg` source-file icon
│   ├── omgb.svg                 # `.omgb` bytecode-file icon
│   └── omg-icon-theme.json      # opt-in "OMG Icons" file-icon theme
├── client/                      # extension entry point (loads the server)
│   ├── package.json
│   ├── tsconfig.json
│   └── src/extension.ts
├── server/                      # LSP server (Node, vscode-languageserver)
│   ├── package.json
│   ├── tsconfig.json
│   └── src/
│       ├── server.ts            # LSP wiring (completion, hover, def, …)
│       ├── analyzer.ts          # OMG-aware document analysis
│       └── builtins.ts          # keyword / built-in tables
└── README.md
```

## Build

```bash
cd vscode
npm install              # installs root + client + server (postinstall)
npm run compile          # tsc -b: builds client/out and server/out
```

For development, `npm run watch` keeps both projects compiling on save.

## Install locally

```bash
cd vscode
npm install -g @vscode/vsce      # one-time
vsce package                     # produces omg-language-support-*.vsix
code --install-extension omg-language-support-*.vsix
```

Or run the **Extensions: Install from VSIX...** command in VS Code.

For interactive development, open this `vscode/` folder in VS Code and hit
F5 — the launch profile starts the **Extension Development Host**, where
the extension is loaded and you can edit/test against any `.omg` file.

## Notes

- Only files that begin with the required `;;;omg` header are treated as
  OMG; the language server doesn't try to compile other files.
- The server uses a line-based scanner (regex over each line) rather than
  hooking into the actual Rust compiler. That keeps the LSP fast and
  decouples it from the runtime — at the cost of less precise diagnostics.
  Real syntax errors still surface when you run the file with `omg`.
- The previous Python-based server (using `pygls`) has been removed; the
  extension is now pure Node so it works without Python on PATH.
