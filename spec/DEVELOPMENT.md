## Overview

This document outlines the development structure, source layout, workflow conventions, and future plans for the OMG programming language project. It serves as a reference for contributors and tools (such as Codex, AGENTS.md) to ensure consistent structure, clear organization, and forward-compatible decisions.

## Development Workflow

### 1. Branch Strategy

* Use the `main` branch for stable, documented features.
* Use the `development` branch for staging work in progress.
* Use feature branches (`feature/xyz`) for experimental syntax or runtime behavior.

### 2. Testing

* Example-based tests are written in OMG and live in the `examples/` directory.
* Output is verified manually after tests are run with `pytest`
* Edge cases and failure modes should be documented via `facts` assertions.

### 3. Feature Introduction

New features must:

* Be consistent with the educational scope of OMG
* Be documented in `spec/OMG_SPEC.md`
* Include at least one usage example
* Avoid introducing unnecessary complexity

## Style Conventions

* Use `snake_case` for functions and variables
* Limit functions to a single conceptual responsibility
* Write module-level docstrings and inline comments where appropriate
* Use descriptive names over abbreviations
* Keep modules small and cohesive

## Contributing

Contributors are expected to:

* Follow guidelines in `AGENTS.md`
* Use `OMG_SPEC.md` as the source of truth
* Communicate clearly when proposing feature changes
* Maintain simplicity and clarity in all code

## Final Note

OMG is a learning-focused language project. The development process should remain flexible, collaborative, and transparent—prioritizing understanding over perfection.
