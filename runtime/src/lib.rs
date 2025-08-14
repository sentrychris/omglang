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

fn execute_with<F>(inject: F, args: &[String]) -> Result<String, JsValue>
where
    F: FnOnce(&mut Vec<Instr>),
{
    let (mut code, funcs) = parse_bytecode(INTERP_OMGBC);
    if matches!(code.last(), Some(Instr::Halt)) {
        code.pop();
    }
    inject(&mut code);
    code.push(Instr::Halt);

    let mut output = String::new();
    {
        let mut emit = |s: String| {
            output.push_str(&s);
            output.push('\n');
        };
        run(&code, &funcs, args, &mut emit)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    }
    Ok(output)
}

/// Execute an OMG source file using the embedded interpreter.
#[wasm_bindgen]
pub fn run_file(prog_path: &str) -> Result<String, JsValue> {
    let args = vec![prog_path.to_string()];
    execute_with(
        |code| {
            code.push(Instr::PushStr(prog_path.to_string()));
            code.push(Instr::Call("run_file".to_string()));
        },
        &args,
    )
}

/// Execute an OMG source string using the embedded interpreter.
#[wasm_bindgen]
pub fn run_source(source: &str) -> Result<String, JsValue> {
    execute_with(
        |code| {
            code.push(Instr::PushStr(source.to_string()));
            code.push(Instr::Call("run".to_string()));
        },
        &[],
    )
}
