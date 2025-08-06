use std::env;
use pyo3::prelude::*;
use pyo3::types::PyList;

/// Native host runtime that forwards execution to the Python-based OMG interpreter
/// and bootstraps the self-hosted interpreter written in OMG.
fn main() -> PyResult<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: omg_native <interpreter.omg> <script.omg> [args...]");
        std::process::exit(1);
    }

    let interpreter_path = &args[1];
    let script_path = &args[2];
    let script_args = &args[3..];

    Python::with_gil(|py| -> PyResult<()> {
        // Ensure repository root is on sys.path so Python modules can be imported.
        let sys = PyModule::import(py, "sys")?;
        sys.getattr("path")?.call_method1("append", ("..",))?;

        // Import the existing Python driver and reuse its run_script helper.
        let omg = PyModule::import(py, "omg")?;
        let run_script = omg.getattr("run_script")?;

        // Assemble arguments passed to the self-hosted interpreter.
        let mut full_args = Vec::new();
        full_args.push(script_path.clone());
        full_args.extend(script_args.iter().cloned());
        let py_args = PyList::new(py, &full_args);

        // Execute the interpreter source file with the provided arguments.
        run_script.call1((interpreter_path, py_args))?;
        Ok(())
    })
}
