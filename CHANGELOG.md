# CHANGELOG.md

Ordered from most recent at the top to oldest at the bottom.

## 2025-08-14

### Fixed
- Tokenizer in `omg_interpreter_boot.omg` now skips comments and header markers so self-hosted sources no longer emit stray tokens or trigger missing `read_number` errors.

### Changed
- Predeclared temporaries in the bootstrap interpreter's executor to avoid repeated `alloc` declarations during meta-circular evaluation.

## 2025-08-13

### Added
- Minimal `omg_interpreter_boot.omg` bundling tokenizer, parser, and evaluator without dictionaries for bootstrap.

### Changed
- Self-hosting driver now uses the low-level interpreter for Stage 3 meta-circular evaluation.

## 2025-08-12

### Added
- Tail-call optimization to the bytecode compiler and native VM via a new `TCALL` instruction.
- Example OMG program demonstrating tail-recursive factorial compilation.

### Changed
- `return` statements now emit `TCALL` when the returned expression is a direct function call, avoiding extra stack frames.

## 2025-08-11

### Changed
- Cached source and token lengths in the self-hosted interpreter to reduce Stage 3 parsing overhead.

### Notes
- Meta-circular driver runtime is improved but remains lengthy during full self-interpretation.

## 2025-08-10

### Added
- Comprehensive Python bytecode compiler that lowers any OMG program to stack-based instructions.
- Expanded Rust VM with booleans, lists, control flow, and function calls to execute compiled bytecode.

### Changed
- Replaced minimal OMG bytecode example with full compiler module and richer native runtime.

## 2025-08-09

### Added
- Minimal OMG bytecode compiler emitting stack-based instructions.
- Native Rust stack VM executing OMG bytecode without Python.
- Arithmetic, variables, and load/store support in the bytecode compiler and native VM.

### Changed
- N/A

## 2025-08-08

### Added
- Initial native Rust host runtime (`/native`) embedding the Python driver to launch the self-hosted OMG interpreter.
- `.gitignore` entry to exclude Rust build artifacts under `native/target`.

### Changed
- N/A

## 2025-08-07

### Added
- Bundled tokenizer, parser, and interpreter into `omg_interpreter.omg` exposing `run` and `run_file`.
- File I/O primitive `read_file` and command-line argument forwarding in the Python runtime.
- Example drivers for string execution, file execution, and self-hosting meta-interpretation.
- Expanded OMG self-hosting interpreter with boolean logic, comparison, slice/index syntax and `ascii` builtin for meta-circular runs.
- Comment-stripped `omg_interpreter_fast.omg` for leaner tokenization in Stage 3 driver.

### Changed
- CLI now accepts additional arguments passed to scripts as `args`.
- Self-hosting interpreter now skips headers/comments in-tokenizer and uses dictionary-based token storage for faster meta interpretation.
- Removed line-number bookkeeping from self-hosted tokens and AST to streamline Stage 3 execution.

## [0.1.0] - 2025-08-06

### Added
- Introduced a full-featured VSCode extension for OMG:
  - Syntax highlighting via `omg.tmLanguage.json`
  - Language configuration (brackets, comments, etc.)
  - Build output via `.vsix` included for local installation
  - Scaffolded language server entry point at `vscode/server/main.py` for future LSP support

### Changed
- Restructured project layout for better modularity and maintainability:
  - All core components moved under `omglang/` (parser, lexer, interpreter, etc.)
  - Test suite relocated to `omglang/tests/`
  - OMG examples organized into subfolders: `examples/modules/`, `examples/self-hosting/`, etc.
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
