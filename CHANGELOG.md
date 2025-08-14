# CHANGELOG.md

Ordered from most recent at the top to oldest at the bottom.

## [0.1.2] - 2025-08-10

### Added
- Generated a WebAssembly package for the runtime with the embedded OMG interpreter bytecode, output under `wasm/`.
- Exposed a `run_source` API in the runtime for executing OMG code from a string, enabling browser-based REPLs.
- `index.html` demonstrating an in-browser REPL powered by the WebAssembly runtime.
- Implemented control-breaking mechanics for exceptions in the VM layer. VM's eval loop tracks an `error_flag` and after each instruction, unwinds the block stack or returns the error if no handler exists.
- Introduced a centralized `call_builtin` helper to dispatch built-ins through a single code path.
- Registered `call_builtin` as a recognized built-in in the compiler for proper lowering during bytecode generation.
- Narrowed error types, adding opcode instructions, handlers and compiler instrutions for specific errors i.e. `SyntaxError`.
- Prefix for calls from the interpreter into the VM layer. Such calls are now prefixed with `_omg_vm` for clarity.
- Backward decoding support for legacy raise opcodes (47–51) for one release.
- Tests covering all raise kinds and stack underflow behaviour.
- Added a `raise` builtin in the VM that delegates to `ops_control::handle_raise`,
  enabling generic errors via `call_builtin` without special-case interpreter logic.

### Changed
- Updated the bytecode interpreter to invoke `call_builtin` for `Instr::CallBuiltin` to streamline execution flow.
- Simplified the interpreter to delegate built-in calls to `call_builtin`.
- Consolidated multiple raise opcodes into a single `RAISE <kind>` instruction driven by a new compact `ErrorKind` enum.
- Updated compiler, VM, and disassembler to encode error kinds as a byte operand and construct `RuntimeError` variants centrally.
- Added VM invariant error on stack underflow for `RAISE`.
- VM stack operations now return `RuntimeError::VmInvariant` on underflow instead of panicking.
- Function call handling in the VM now returns `RuntimeError` on undefined or invalid calls instead of panicking.
- Moved builtin dispatch into a dedicated `vm::builtins` module exposing `call_builtin`.
- Refactored VM opcode dispatch into dedicated handler modules for arithmetic, structural, and control operations.
- `Value::as_int` now returns `Result<i64, RuntimeError>` and emits a `TypeError` when string parsing fails.
- Removed generated WebAssembly artifacts from version control; `wasm/` is now gitignored and rebuilt locally with `wasm-pack`.
- Documented separate build steps for the native binary and WebAssembly package in `README.MD`.

### Fixed
- Renamed CLI binary to `omg` to avoid build output filename collisions with the `omg_runtime` library.
- Centralized `call_builtin` helper eliminates scattered implementations across the runtime.
- Narrower error handling eliminates relying on generic string-based `raise` which was resulting in prefixed errors e.g. `RuntimeError: SyntaxError: Unxepected <symbol>...`, errors are now correctly defined according to their type. Generic `raise()` has been retained for special cases.
- Refactored basename extraction in bootstrap interpreter's `import_module` to avoid negative string indexing when module paths lack directory separators.
- Guarded `dirname` and `run_file_with_args` against negative string indexing so modules in the current directory import and execute without errors.
- Validated slice indices in the VM, returning `IndexError` for out-of-range or invalid ranges instead of panicking.
- VM `LOAD` instruction now raises `UndefinedIdentError` when a name is missing instead of defaulting to zero.
- `run_source` now runs the interpreter's global initialization before invoking `run`, preventing stack underflow in the WebAssembly REPL.

## [0.1.1] - 2025-08-08

### Added
- `verify_binary.py` now performs a two-pass decode/validate of the
  interpreter bytecode, checking jump targets, function addresses and
  `CALL`/`TCALL` references for validity.
- Native VM now embeds the OMG interpreter bytecode and can execute `.omg`
  scripts directly; the bytecode is generated at build time via a Rust build
  script.
- Bytecode compiler emits binary `.omgb` files and the native VM loads these
  binary bytecode programs directly instead of parsing textual mnemonics.
- OMG interpreter is written in OMG, compiled ahead of time to `.omgb` and embedded into the runtime
- `FrozenDict` value type in the VM to expose read-only module exports.
- Native VM forwards command-line arguments to bytecode programs via a global
  `args` list, allowing compiled interpreters to execute scripts.

### Changed
- Split native VM runtime into separate modules for easier development.
- Removed VM-level `IMPORT` instruction; all `.omg` module loading is handled by
  the interpreter.
- Bytecode compiler now rejects `import` statements, deferring module resolution
  to the interpreter.

### Fixed
- Bytecode compiler wrote boolean literals as integers without an operand
  byte, causing the native runtime to panic when parsing `.omgb` files.
  Boolean literals are now encoded correctly.
- `read_file` no longer panics on missing files; the interpreter reports a
  clear error message instead.
- Interpreter normalizes script paths before resolving imports, preventing
  missing module errors on Windows.
- Guarded native VM value formatting against cyclic structures to prevent
  stack overflows when importing modules.
- Refactored self-hosting interpreter example to declare loop variables
  outside loops in both parser and executor, preventing "already declared"
  errors.
- Bytecode compiler emits built-in calls in tail positions as `BUILTIN`
  instructions instead of `TCALL`, preventing "Unknown function" errors
  when running compiled interpreters under the native VM.
- Increased Python recursion limit so the self-hosted interpreter can run
  deeply recursive programs without hitting `RecursionError`.
- Renamed internal argument variables so the native VM no longer conflates
  function call arguments with the global `args` list, fixing incorrect
  output in scripts like `hexrgb.omg`.

## [0.1.0] - 2025-08-06

### Added
- Support for multiline `/** ... */` docblock comments in the lexer and VSCode syntax highlighting.
- Introduced a full-featured VSCode extension for OMG:
  - Syntax highlighting via `omg.tmLanguage.json`
  - Language configuration (brackets, comments, etc.)
  - Build output via `.vsix` included for local installation
  - Scaffolded language server entry point at `vscode/server/main.py` for future LSP support

### Changed
- Restructured project layout for better modularity and maintainability:
  - All core components moved under `omglang/` (parser, lexer, interpreter, etc.)
  - Test suite relocated to `omglang/tests/`
  - New `scripts/` folder for build, packaging, and automation utilities
  - Output artifacts from PyInstaller now live under `output/`

### Build System
- Switched to using `pyproject.toml` for unified build configuration and packaging metadata.
  - Using fresh virtualenv for development.
  - `setup.cfg` retained for backward compatibility and linting options

### Notes
- Existing CLI and interpreter functionality remains unchanged.

## [0.0.0] - 2025-08-06

### Added

* **Module import system**:
  * New `import "<file>" as <alias>` syntax for loading other OMG scripts.
  * Top-level `alloc` and `proc` bindings are exported automatically.
  * Imported namespaces are read-only via `FrozenNamespace`.
  * Interpreter tracks loaded modules to prevent recursive imports.
* **Examples** demonstrating module usage in `examples/modules`.

### Changed

* Lexer recognizes `import` and `as` keywords.
* Parser supports `'import'` statements through a dedicated `parse_import`.
* `FunctionValue` retains defining global scope so imported functions can recurse or call other module-level bindings.

### Fixed

* Circular imports now raise a `RuntimeError`.
* Imported modules reject attempts to reassign exported values.

### Tests

* Added `test_modules.py` covering:
  * Basic import and usage of exported bindings.
  * Enforcement of read-only modules.
  * Circular import detection.
  * Recursive functions across modules.


## [0.0.0] - 2025-08-05

### Added

* **Dictionary support**:

  * New dictionary literal syntax using `{ key: value, ... }`
    * Example: `alloc person := { name: "Chris", age: 32 }`
  * Support for both:
    * **Dot notation** (`person.name`)
    * **Index notation** (`person["name"]`)
  * New AST node types and interpreter evaluation logic for:
    * Dictionary literals
    * Field access and assignment (`x.key := val`)
    * Key-based access and mutation (`x["key"] := val`)
  * Parser now distinguishes between simple identifiers and l-values with chained accessors.
  * Dot access is desugared into a `'dot'` AST node, and assignments to keys or attributes are translated to `'index_assign'` or `'attr_assign'`.

### Changed

* `interpreter.py`:
  * Indexing logic generalized to evaluate dynamic expressions and support dictionary lookup.
  * Slice and dot access logic now evaluates base expressions (not just identifiers).
  * Assignment handling extended to support nested field/index assignments.
* `expressions.py`:
  * Added dictionary literal parsing with support for both string and identifier keys.
  * Extended factor parsing to support postfix dot/index/call chains.
* `statements.py`:
  * Introduced `_parse_lvalue()` for recursive l-value parsing.
  * Reassignment parsing now recognizes and routes attribute and index assignments.
* `lexer.py`:
  * Added support for the dot token `.` as `DOT`.

### Fixed

* AST formatting (`_format_expr`) now renders lists, strings, dictionaries, dot/index expressions more clearly in debug and error messages.

### Tests

* Added `test_dictionaries.py` to validate full range of dictionary functionality:

  * Declaration, mutation, nested structure, function parameter passing, and `facts` assertions.

## [0.0.0] - 2025-08-04

### Added
- Support for first-class functions and lexical closures
- Example demonstrating higher-order function usage
- Tests for assigning, passing, and returning functions

### Changed
- Parser and interpreter updated to treat procedures as values

## 2025-08-02

### Added
- Initial `OMG_SPEC.md` defining core syntax, semantics, and runtime rules
- `AGENTS.md` documenting expected behavior for intelligent assistants
- `DEVELOPMENT.md` outlining development policy and roadmap

### Changed
- Formalized language design
- Stabilized core specification

### Notes
- Marks the formalization of the OMG language structure, design goals, and contributor policy
