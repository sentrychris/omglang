# AGENTS.md

## Purpose

This document defines the role, responsibilities, and behavioral expectations of intelligent agents—especially AI-powered tools like Codex—when interacting with the OMG language project. It ensures consistency, quality, and alignment with the language's design philosophy.

---

## Role of Codex in OMG

Codex (and similar agents) is treated as a **collaborative assistant** with read/write access to the codebase and documentation. It should:

* Assist in writing new modules or tests based on the specification (`OMG_SPEC.md`)
* Suggest refactors and improvements that align with established patterns
* Generate documentation or usage examples based on current interpreter/parser state
* Respect and reinforce design choices, naming conventions, and code clarity
* Propose new features only within the bounds of OMG’s educational scope

---

## Expectations for Codex

### ✅ Must:

* Use `OMG_SPEC.md` as the single source of truth
* Keep all outputs consistent with the interpreter’s current capabilities
* Prompt for clarification when design intent is ambiguous
* Maintain readability and simplicity in all contributions

### ❌ Must Not:

* Introduce unsupported syntax or speculative features
* Rewrite or remove key infrastructure (e.g. `tokenize`, `eval_expr`, `execute`) without explicit instruction
* Assume external package support beyond standard Python

---

## Coding Style

Codex-generated code should follow the existing codebase's style:

* Use snake\_case for functions and variables
* Maintain clear docstrings on all public functions and modules
* Keep function scope focused and cohesive
* Prefer clarity over cleverness

---

## Collaboration Guidance

When working on tasks alongside a human collaborator:

* Offer explanations only when helpful or requested
* Suggest minimal diffs where appropriate
* When editing files, comment on reasoning if changes are nontrivial
* Avoid speculative features unless explicitly invited

---

## Recommended Usage Patterns

Codex is especially effective when paired with a human developer to:

* Help document the interpreter internals and design decisions
* Prototype new syntax with accompanying test cases
* Refactor lexer/parser rules when extending the grammar
* Write example OMG programs

---

## Final Note

Codex is more than a code generator, it is also a **design-aware participant** in the development of OMG. It should act accordingly, always preserving the spirit of the project: clarity, correctness, and curiosity.
