//! OMG Language Runtime entry point.
//!
//! This binary can do two things:
//! 1) Run a precompiled OMG bytecode file (`.omgb`) directly.
//! 2) Run a plain OMG source script (`.omg`) by invoking the **embedded
//!    interpreter** that’s compiled into this binary at build time.
//!
//! Behavior summary:
//! - With **no args**, start an interactive REPL.
//! - With `-h/--help`, print usage.
//! - With `-v/--version`, print build-target + version.
//! - With a **`.omgb`** path, load bytecode from disk and execute it.
//! - With a **`.omg`** path, run the embedded interpreter bytecode and pass the
//!   `.omg` script path to it (the interpreter will then load/execute it).
//!
//! Argument separator:
//! - If a literal `--` appears *after* the script path, everything after it is
//!   considered program arguments and will be exposed to the OMG program via
//!   the VM’s `args` global.

use std::env;
use std::fs;

mod bytecode;
mod error;
mod repl;
mod value;
mod vm;

use bytecode::parse_bytecode;
use repl::repl_interpret;
use vm::run;

/// Embedded `interpreter.omgb` generated at build time.
///
/// The build script places the compiled interpreter bytecode in
/// `$OUT_DIR/interpreter.omgb`. We bake it into the final binary so that
/// users can run `.omg` source files **without** needing a separate
/// interpreter executable at runtime.
const INTERP_OMGBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/interpreter.omgb"));

/// Human-facing runtime version string.
///
/// This reflects the runtime wrapper, not the language version.
/// It’s printed by `--version` and included in help text.
const VERSION: &str = "0.1.2";

/// Construct the help/usage text shown for `-h/--help`.
///
/// Kept as a function so it can interpolate the current `VERSION`.
fn usage() -> String {
    format!(
        r#"OMG Language Runtime v{0}

Usage:
    omg <script.omg>

Arguments:
    <script.omg>
        Path to an OMG language source file to execute. The file must
        include the required header ';;;omg' on the first non-empty line.

Example:
    omg hello.omg

Options:
    -h, --help
        Show this help message and exit.
    -v, --version
        Show runtime version."#,
        VERSION
    )
}

/// Program entry point.
///
/// High-level flow:
/// 1) Read CLI args.
/// 2) If no args → start REPL.
/// 3) If `--help/--version` → print and exit.
/// 4) Otherwise treat the first arg as a path. If it ends with `.omgb`,
///    load bytecode and execute. Otherwise, assume it is a `.omg` source
///    path and execute it through the embedded interpreter bytecode.
///
/// Argument forwarding:
/// - For `.omgb`, we forward args *after* the script path to the program.
/// - For `.omg`, we invoke the embedded interpreter and pass a vector where
///   the first argument is the source file path, followed by any extra args.
/// - In both modes we support an optional literal `--` that separates
///   runtime flags from program args (everything after `--` is treated as
///   program input).
fn main() {
    // Capture raw command-line arguments as owned Strings.
    let args: Vec<String> = env::args().collect();

    // --- Mode selection & meta commands ------------------------------------

    // No arguments → interactive REPL (dev-friendly quick start).
    if args.len() == 1 {
        repl_interpret();
        return;
    }

    // Help flag → show usage and exit 0.
    if args[1] == "-h" || args[1] == "--help" {
        println!("{}", usage());
        return;
    }

    // Version flag → print OS/arch/build-friendly identifier + runtime version.
    if args[1] == "-v" || args[1] == "--version" {
        println!(
            "omg-runtime-build-{}-{}: v{}",
            env::consts::OS,
            env::consts::ARCH,
            VERSION
        );
        return;
    }

    // --- Execution modes ----------------------------------------------------

    if args[1].ends_with(".omgb") {
        // === Bytecode mode: execute a precompiled .omgb binary ===
        //
        // Layout: omg <file.omgb> [--] [program args...]
        // We slice the original `args` to obtain "program args" exposed to the VM.
        let bc_path = &args[1];

        // Extract program arguments after the `.omgb` path.
        // If `--` is present immediately after the path, skip it.
        let program_args: &[String] = if args.len() > 2 {
            if args[2] == "--" {
                &args[3..]
            } else {
                &args[2..]
            }
        } else {
            &[]
        };

        // Read bytecode from disk; any I/O error is a hard failure (panic)
        // because we cannot proceed. Using `expect` yields a concise message.
        let src = fs::read(bc_path).expect("failed to read bytecode file");

        // Decode the bytecode image into instruction stream + function table.
        let (code, funcs) = parse_bytecode(&src);

        // Hand off to the VM. On runtime error we print to stderr and exit 1
        // (so that shells/scripts can detect failure).
        if let Err(e) = run(&code, &funcs, program_args) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    } else {
        // === Source mode: execute a .omg script using the embedded interpreter ===
        //
        // Layout: omg <file.omg> [--] [program args...]
        //
        // We do **not** parse/execute the `.omg` file directly here. Instead we
        // run the *embedded interpreter* (compiled as `.omgb`) and pass it the
        // source path and args. The interpreter bytecode knows how to read the
        // `.omg` file, lex/parse/interpret it.
        let prog_path = &args[1];

        // Extract the trailing program args in the same way as above.
        let program_args_slice: &[String] = if args.len() > 2 {
            if args[2] == "--" {
                &args[3..]
            } else {
                &args[2..]
            }
        } else {
            &[]
        };

        // The embedded interpreter expects argv-style input where argv[0] is the
        // script path, so we construct that here and then append user args.
        let mut full_args = Vec::with_capacity(program_args_slice.len() + 1);
        full_args.push(prog_path.clone());
        full_args.extend_from_slice(program_args_slice);

        // Load the embedded interpreter bytecode image from the build output.
        let (code, funcs) = parse_bytecode(INTERP_OMGBC);

        // Execute the interpreter, providing it with the constructed arguments.
        // On error, forward message to stderr and exit 1.
        if let Err(e) = run(&code, &funcs, &full_args) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
