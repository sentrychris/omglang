mod bytecode;
mod error;
mod repl;
mod value;
mod vm;

use bytecode::{parse_bytecode, Instr};
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

/// Execute an OMG source string using the embedded interpreter.
#[wasm_bindgen]
pub fn run_source(source: &str) -> Result<(), JsValue> {
    let (orig_code, mut funcs) = parse_bytecode(INTERP_OMGBC);
    let mut code = vec![
        Instr::PushStr(source.to_string()),
        Instr::Call("run".to_string()),
        Instr::Halt,
    ];
    let offset = code.len();
    for func in funcs.values_mut() {
        func.address += offset;
    }
    code.extend_from_slice(&orig_code);
    run(&code, &funcs, &[]).map_err(|e| JsValue::from_str(&format!("{}", e)))
}
