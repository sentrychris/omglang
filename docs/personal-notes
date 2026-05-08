Two-step pipeline. The first step compiles, the second runs the bytecode:

```sh
# 1. Compile <prog>.omg with the OMG-written compiler.
omg --self-hosted-compile myprog.omg /tmp/myprog.omgb

# 2. Execute the .omgb through the OMG-written VM.
omg bootstrap/vm.omg /tmp/myprog.omgb
```

That's the most-OMG path: an OMG-written compiler produced the bytecode, an OMG-written VM is interpreting it. Both happen to be running on the Rust substrate VM, but everything language-level is OMG.

A few variants worth knowing:

```sh
# Same as above, but use --rust for the outer step. The OMG VM source
# (bootstrap/vm.omg) is then compiled by the Rust frontend
# instead of the OMG compiler — much faster startup, identical output.
omg --rust bootstrap/vm.omg /tmp/myprog.omgb

# If you also want the inner program to be compiled by the Rust
# frontend (fastest path while still using the OMG VM to execute):
omg --rust --compile myprog.omg /tmp/myprog.omgb
omg --rust bootstrap/vm.omg /tmp/myprog.omgb
```

Concrete one-liner using the prime sieve as a smoke test:

```sh
omg --self-hosted-compile examples/prime_sieve.omg /tmp/sieve.omgb \
  && omg --rust bootstrap/vm.omg /tmp/sieve.omgb
```

That should print the primes up to 100, byte-identical to running omg `examples/prime_sieve.omg` through the regular Rust VM.