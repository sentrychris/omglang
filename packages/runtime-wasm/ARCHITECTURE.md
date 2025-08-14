# Architecture

The runtime is compiled from Rust to WebAssembly using `wasm-bindgen`. A Web Worker hosts the
module and exposes a small message based protocol for the browser based REPL.
