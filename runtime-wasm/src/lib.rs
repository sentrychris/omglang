use serde::{Deserialize, Serialize};
use serde_json::Value;
use wasm_bindgen::prelude::*;

/// Initialize panic hook when the module starts.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Interpreter state for a WebAssembly session.
#[derive(Default)]
pub struct Session;

/// Options controlling evaluation behaviour.
#[derive(Serialize, Deserialize, Default)]
pub struct EvalOptions {
    pub timeout_ms: Option<u32>,
    pub fuel: Option<u64>,
}

/// Diagnostic information produced during evaluation.
#[derive(Serialize, Default)]
pub struct Diagnostic {
    pub message: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub kind: String,
}

/// Result returned from evaluating a piece of OMG code.
#[derive(Serialize, Default)]
pub struct EvalResult {
    pub stdout: Vec<String>,
    pub return_value: Value,
    pub diagnostics: Vec<Diagnostic>,
    pub elapsed_ms: u32,
    pub fuel_used: Option<u64>,
}

/// Public wrapper around a session which is exposed to JavaScript.
#[wasm_bindgen]
pub struct WasmSession {
    inner: Session,
}

#[wasm_bindgen]
impl WasmSession {
    /// Create a new WebAssembly-backed session.
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmSession {
        WasmSession {
            inner: Session::default(),
        }
    }

    /// Reset the session to an empty state.
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.inner = Session::default();
    }

    /// Evaluate a snippet of OMG code and return a serialised result.
    #[wasm_bindgen]
    pub fn eval(&mut self, _code: &str, opts_js: JsValue) -> Result<JsValue, JsValue> {
        let _opts: EvalOptions = serde_wasm_bindgen::from_value(opts_js).unwrap_or_default();
        let result = EvalResult::default();
        serde_wasm_bindgen::to_value(&result).map_err(|e| e.into())
    }
}

/// Version string of the runtime.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
