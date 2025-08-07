# CHANGELOG.md

Ordered from most recent at the top to oldest at the bottom.

## [Unreleased]

### Added
- Expanded `bytecode.py` to compile the full OMG language including imports,
  dictionary operations, assertions, and break handling.
- Added a bootstrap interpreter in `bootstrap/` written in OMG capable of
  emitting bytecode, demonstrating self-hosting of the compiler.
- Bytecode compiler and native VM now recognize built-in functions like
  `length` and `chr`, emitting `BUILTIN` instructions and executing them
  directly.
- Command line compiler writes bytecode using UTF-8 encoding and accepts an
  optional output path to avoid shell re-encoding on Windows.

### Fixed
- Refactored self-hosting interpreter example to declare loop variables
  outside loops in both parser and executor, preventing "already declared"
  errors.
- Native VM now concatenates lists when using `ADD`, preventing `length()`
  from receiving integers instead of lists.
- Built-in calls in the native VM can access and modify global variables,
  so `length()` inside functions operates on lists rather than defaulting
  to integers.
- The native VM parses and executes the `MOD` instruction so modulo
  operations in bytecode (e.g., in `rot_13.omg`) run without crashing.

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
