# Changelog

## 0.3.0

A rewrite.

**Language server**

- Built on a port of the compiler's lexer and parser, so syntax errors
  appear as you type with the compiler's message, on the compiler's line.
- Hover documentation for every builtin (signature, parameters, result,
  errors, a tested example), every keyword and operator, and your own
  procs, variables, modules and dict entries, including the comment
  written above a declaration.
- Completion that fits the cursor: module and dict members after a dot,
  import paths, keywords only where they are allowed.
- Signature help for builtins and for your own procs.
- Go to definition, find references and rename across files.
- Checks for what the compiler leaves to run time: wrong argument counts,
  undefined names, assignment without `alloc`, procs hidden by builtins,
  broken imports, and more, several with quick fixes.
- Semantic highlighting, outline, workspace symbols, folding, clickable
  import paths, and a formatter for indentation.

**Editor**

- New grammar. Fixes: floats, exponents and `//` were not recognised; the
  first name of any `a.b` was coloured as a module; slices such as
  `xs[a:b]` could not be told from dict keys; `\'` was not an escape;
  builtins added since 0.2 were missing, and a builtin's name was coloured
  as one even as a variable or after a dot; block comments were toggled
  with `/** */`.
- OMG Dark and OMG Light themes.
- OMG highlighting inside Markdown code fences.
- Commands: Run File (Ctrl+F5), Build Native Executable, Show Bytecode,
  Open REPL, Restart Language Server.
- Snippets.
- New file icons. They are now the language's own icons, which every file
  icon theme falls back to; the separate "OMG Icons" icon theme is gone,
  since choosing it hid the icons of every other file type.

**Packaging**

- One `package.json`; the client and server are bundled with esbuild, so
  the `.vsix` carries two JavaScript files and no `node_modules`.
- A test suite that holds the extension to the compiler (`npm test`).

## 0.2.0

- Language server in TypeScript (completion, hover, go to definition for
  the open file), replacing the Python one.
