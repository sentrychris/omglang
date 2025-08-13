mod bytecode;
mod error;
mod repl;
mod value;
mod vm;

use bytecode::parse_bytecode;
use vm::run;
use wasm_bindgen::prelude::*;

/// Embedded interpreter.omgb generated at build time.
const INTERP_OMGBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/interpreter.omgb"));

/// Execute an OMG source file using the embedded interpreter.
#[wasm_bindgen]
pub fn run_file(prog_path: &str) -> Result<(), JsValue> {
    let args = vec![prog_path.to_string()];
    let (code, funcs) = parse_bytecode(INTERP_OMGBC);
    run(&code, &funcs, &args).map_err(|e| JsValue::from_str(&format!("{}", e)))
}
