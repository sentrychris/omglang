## What is `bootstrap/bin/omg`?

It's a regular Linux executable like `cat` or `ls`. The OS knows how to run it. It contains x86-64 machine code and is dynamically linked against libc.

## What's inside `bootstrap/bin/omg`?

1. A small C runtime (`bootstrap/src/omg_rt.h`) which provides things like:
  - A `Value` type to hold int / string / list / dict / closure etc.
  - Refcounted lists, dicts and closures
  - `omg_add`, `omg_emit`, `omg_subprocess` etc.
  - setjmp/longjmp-based exception handling

  Thought of as "what OMG needs from the metal", like libc for C programs.

2. Translated OMG bytecode, compiled to straight-line C. Each OMG instruction became a few lines of C that push/pop a stack and call runtime helpers.





