### **Task Introduction – Import/Export System for OMG**

We are implementing a module import system for the OMG language. The goal is to allow OMG scripts to explicitly import functions and constants from other `.omg` files under a namespace. Only top-level `proc` and `alloc` declarations are exported; the rest of the file executes normally but cannot be accessed externally.

#### Goals and Design Constraints

**Primary goal**: Allow one .omg script to import proc functions and alloced constants from another .omg file.

**Scope:**

- No cyclical imports.
- Imports must be explicit (no wildcard import *).
- Only top-level alloc and proc definitions can be exported.
- Variables/functions imported must be immutable from the importing script’s perspective.

The syntax is:

```omg
import "utils.omg" as utils
emit utils.greet("world")
```

This imports `greet` and any other exported symbols from `utils.omg` and binds them to the `utils` namespace.

### Key Rules:

* Imports execute the full script and extract top-level `proc` and `alloc` symbols.
* Symbols are namespaced via `as utils`, and accessed via dot notation (e.g. `utils.name`).
* Nested imports are allowed, but recursive imports must be detected and disallowed.
* Imported values are read-only.
* Relative paths are used for module resolution.

---


To implement the OMG import/export system cleanly and incrementally, the most effective approach is to **begin with runtime behavior**.

* Imports must execute the other script and extract top-level symbols before parsing or emitting can proceed.
* Runtime handling informs the parser design—e.g., what the parser must emit as AST and what the interpreter expects.
* Errors like "module not found" or "recursive import" are runtime conditions.


## **Task Breakdown for Codex**

### **Phase 1: Runtime Integration (Import Execution and Symbol Binding)**

#### ✅ Task 1: Add `Interpreter.import_module(path)` method

* Accepts a relative `.omg` file path and returns a dict of exported bindings.
* Internally:

  * Opens and runs the target script in a new interpreter instance.
  * Collects and returns only top-level `decl` and `func_def` statements.
  * Filters out top-level `emit`, `loop`, etc. (still executes them).
* Must preserve global scope and closure capture from the module's context.

#### ✅ Task 2: Add circular import protection

* Maintain a global `loaded_modules` set in the main interpreter instance.
* Raise a descriptive error if `import_module()` tries to load the same file twice in the same call stack.

---

### **Phase 2: Syntax and Parsing**

#### ✅ Task 3: Extend lexer with `import` keyword

* Add `IMPORT` token type and include `import` as a keyword in `lexer.py`.

#### ✅ Task 4: Extend parser to recognize import statements

* Parse:

  ```omg
  import "utils.omg" as utils
  ```

  into:

  ```python
  ('import', "utils.omg", "utils", line)
  ```
* Validate: `as` must be present, path must be a string literal, alias must be a valid identifier.

---

### **Phase 3: Interpreter Support**

#### ✅ Task 5: Extend interpreter `execute()` to handle `import` statements

* On AST node `('import', path, alias, line)`:

  * Call `import_module(path)`
  * Bind result to `vars[alias]` as a dictionary (keys = proc/const names, values = values/FunctionValues)

---

### **Phase 4: Access and Safety**

#### ✅ Task 6: Support dotted access into imported namespaces

* Already supported in the interpreter (`dot` node) for dictionaries.
* Validate that imported modules behave correctly under `utils.FUNC()` or `utils.CONST`.

#### ✅ Task 7: Make imported symbols read-only

* Disallow:

  ```omg
  utils.VERSION := "2.0"
  ```
* May involve marking namespace dicts as frozen or raising on `dot` assignment.

---

### **Phase 5: UX and Debugging**

#### ✅ Task 8: Improve error messages for module loading

* Add messages for:

  * File not found
  * Module already loaded
  * Syntax errors in imported script
  * Missing `;;;omg` header in imported script

#### ✅ Task 9: Add REPL and script support for relative paths

* Ensure REPL-based `import` looks for files in working directory.
* Normalize and canonicalize paths to avoid duplicate loads under different names.

---

### **Optional Future Work**

(Not part of MVP)

* Task: `exposing` syntax to limit exports
* Task: Caching parsed modules
* Task: Import relative to calling module’s directory (not just cwd)
