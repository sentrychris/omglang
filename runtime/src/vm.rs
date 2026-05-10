//! # Stack-Based Bytecode Virtual Machine (Runtime)
//!
//! Compact stack VM that executes a sequence of [`Instr`] produced by the
//! Rust [`crate::compiler`].
//!
//! ## High-level model
//! - **Operand stack (`stack`)**: holds `Value`s consumed/produced by ops.
//! - **Global env (`globals`)**: persists across function frames.
//! - **Local env (`env`)**: the current function's locals.
//! - **Env stack (`env_stack`)**: saved local frames for nested calls.
//! - **Return stack (`ret_stack`)**: saved program counters.
//! - **Block stack (`block_stack`)**: exception-handling frames.
//! - **Program counter (`pc`)**: index into `code`.
//! - **Advance flag (`advance_pc`)**: lets control-flow ops manage the PC.
//!
//! Two surfaces are exposed:
//! - [`run`] — one-shot: build a fresh VM state from scratch and execute
//!   a complete program. Used by `main.rs` for normal script execution.
//! - [`run_program`] — resident: execute a chunk of bytecode against a
//!   caller-owned `globals` and `funcs` table. Used by [`crate::repl`] so
//!   state survives across REPL turns.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::bytecode::{Function, Instr};
use crate::error::RuntimeError;
use crate::value::{new_cell, Env, Value};

/// If `name` looks like a module-mangled identifier (`__mod_N__bare`),
/// return the bare suffix. Otherwise None. Used by `MakeFunc` to bind
/// nested-proc closures under both their mangled funcs-table key and
/// the bare source name that `Load` instructions reference.
fn strip_mod_prefix(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("__mod_")?;
    let end_idx = rest.find("__")?;
    let digits = &rest[..end_idx];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let bare = &rest[end_idx + 2..];
    if bare.is_empty() {
        None
    } else {
        Some(bare)
    }
}

mod builtins;
mod ops_arith;
mod ops_control;
mod ops_struct;

/// Exception-handling frame.
///
/// Pushed by `SetupExcept`, popped by `PopBlock`. On `Raise`, the VM unwinds
/// to the most-recent block: restoring stack height, env-stack depth, and
/// return-stack depth that were current when the block was set up, then
/// transferring control to its `handler` PC.
pub(super) struct Block {
    handler: usize,
    stack_size: usize,
    env_depth: usize,
    ret_depth: usize,
}

/// Pop one [`Value`] from the operand stack with an underflow check.
pub(super) fn pop(stack: &mut Vec<Value>) -> Result<Value, RuntimeError> {
    stack
        .pop()
        .ok_or_else(|| RuntimeError::VmInvariant("stack underflow".to_string()))
}

/// Seed a globals map with `args`, `module_file`, and `current_dir`. Useful
/// from both the entry-point runner and the REPL.
pub fn seed_program_globals(globals: &mut HashMap<String, Value>, program_args: &[String]) {
    let arg_values: Vec<Value> = program_args.iter().map(|s| Value::Str(s.clone())).collect();
    globals.insert(
        "args".to_string(),
        Value::List(Rc::new(RefCell::new(arg_values))),
    );
    // `module_file` is the script path, kept for diagnostics.  Falls back
    // to "<stdin>" when there is no script (e.g. REPL).
    let module_file = program_args
        .first()
        .map(|s| s.replace("\\", "/"))
        .unwrap_or_else(|| "<stdin>".to_string());
    globals.insert("module_file".to_string(), Value::Str(module_file));

    // `current_dir` is the *shell's* current working directory, not the
    // script's parent.  This matches every other CLI tool — `omg
    // tools/wc.omg foo.txt` resolves `foo.txt` against where the user is
    // standing, the way `wc foo.txt` would.  Imports don't read this
    // global (they're resolved at compile time using `current_file`), so
    // no part of the language relies on the older script-dir-based value.
    let cwd = std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().replace("\\", "/"))
        .unwrap_or_else(|| ".".to_string());
    globals.insert("current_dir".to_string(), Value::Str(cwd));
}

/// One-shot execution of a complete program.
///
/// Builds a fresh globals map (via [`seed_program_globals`]) and a fresh
/// function table from `funcs`, then runs the program to completion.
pub fn run(
    code: &[Instr],
    funcs: &HashMap<String, Function>,
    program_args: &[String],
) -> Result<(), RuntimeError> {
    let mut globals: HashMap<String, Value> = HashMap::new();
    seed_program_globals(&mut globals, program_args);
    let mut funcs_owned: HashMap<String, Function> = funcs.clone();
    run_program(code, funcs, &mut globals, &mut funcs_owned)
}

/// Resident execution against caller-owned globals and a function table.
///
/// `funcs_in` is the function table baked into the bytecode being executed
/// right now (e.g. just compiled by the REPL). `funcs_persistent` is the
/// caller's accumulated table from prior turns; on entry we merge `funcs_in`
/// into it so newly defined procs become visible to subsequent invocations.
pub fn run_program(
    code: &[Instr],
    funcs_in: &HashMap<String, Function>,
    globals: &mut HashMap<String, Value>,
    funcs_persistent: &mut HashMap<String, Function>,
) -> Result<(), RuntimeError> {
    for (name, f) in funcs_in {
        funcs_persistent.insert(name.clone(), f.clone());
    }
    // The caller passes the full bytecode each turn (REPL) or once (one-shot
    // run). Function addresses are absolute indexes into `code`, so the same
    // table works for both call paths.
    execute(code, funcs_persistent, globals, 0)
}

/// Execute `code` against caller-owned `globals` and `funcs`, starting at
/// the given program counter. Used by the REPL to run only the freshly
/// appended chunk of an accumulated bytecode buffer.
pub fn run_program_from(
    code: &[Instr],
    funcs: &HashMap<String, Function>,
    globals: &mut HashMap<String, Value>,
    start_pc: usize,
) -> Result<(), RuntimeError> {
    execute(code, funcs, globals, start_pc)
}

fn execute(
    code: &[Instr],
    funcs: &HashMap<String, Function>,
    globals: &mut HashMap<String, Value>,
    start_pc: usize,
) -> Result<(), RuntimeError> {
    let mut stack: Vec<Value> = Vec::new();
    let mut env: Env = HashMap::new();
    let mut env_stack: Vec<Env> = Vec::new();
    let mut ret_stack: Vec<usize> = Vec::new();
    let mut block_stack: Vec<Block> = Vec::new();
    let mut error_flag: Option<RuntimeError> = None;
    let mut pc: usize = start_pc;

    while pc < code.len() {
        let mut advance_pc = true;

        let instr_res: Result<(), RuntimeError> = loop {
            match &code[pc] {
                Instr::PushInt(v) => stack.push(Value::Int(*v)),
                Instr::PushFloat(v) => stack.push(Value::Float(*v)),
                Instr::PushStr(s) => stack.push(Value::Str(s.clone())),
                Instr::PushBool(b) => stack.push(Value::Bool(*b)),
                Instr::BuildList(n) => {
                    if let Err(e) = ops_struct::handle_build_list(*n, &mut stack) {
                        break Err(e);
                    }
                }
                Instr::BuildDict(n) => {
                    if let Err(e) = ops_struct::handle_build_dict(*n, &mut stack) {
                        break Err(e);
                    }
                }
                Instr::Load(name) => {
                    // Read through the cell so closures see live state.
                    if let Some(cell) = env.get(name) {
                        stack.push(cell.borrow().clone());
                    } else if let Some(v) = globals.get(name) {
                        stack.push(v.clone());
                    } else {
                        break Err(RuntimeError::UndefinedIdentError(name.clone()));
                    }
                }
                Instr::Store(name) => {
                    if let Some(v) = stack.pop() {
                        // `:=` is *re-assignment only*: the name must already
                        // be bound somewhere. Use `alloc x := v` to introduce
                        // a new binding. This catches typos like `cont := 5`
                        // that would otherwise silently shadow / clobber.
                        //
                        // For local bindings, mutate the cell *in place* so
                        // closures that captured this name see the update.
                        if env_stack.is_empty() {
                            if globals.contains_key(name) {
                                globals.insert(name.clone(), v);
                            } else {
                                break Err(RuntimeError::UndefinedIdentError(
                                    name.clone(),
                                ));
                            }
                        } else if let Some(cell) = env.get(name) {
                            *cell.borrow_mut() = v;
                        } else if globals.contains_key(name) {
                            globals.insert(name.clone(), v);
                        } else {
                            break Err(RuntimeError::UndefinedIdentError(name.clone()));
                        }
                    }
                }
                Instr::Add => {
                    if let Err(e) = ops_arith::handle_add(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Sub => {
                    if let Err(e) = ops_arith::handle_sub(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Mul => {
                    if let Err(e) = ops_arith::handle_mul(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Div => {
                    if let Err(e) = ops_arith::handle_div(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::FloorDiv => {
                    if let Err(e) = ops_arith::handle_floor_div(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Mod => {
                    if let Err(e) = ops_arith::handle_mod(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Eq => {
                    if let Err(e) = ops_arith::handle_eq(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Ne => {
                    if let Err(e) = ops_arith::handle_ne(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Lt => {
                    if let Err(e) = ops_arith::handle_lt(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Le => {
                    if let Err(e) = ops_arith::handle_le(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Gt => {
                    if let Err(e) = ops_arith::handle_gt(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Ge => {
                    if let Err(e) = ops_arith::handle_ge(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::BAnd => {
                    if let Err(e) = ops_arith::handle_band(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::BOr => {
                    if let Err(e) = ops_arith::handle_bor(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::BXor => {
                    if let Err(e) = ops_arith::handle_bxor(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Shl => {
                    if let Err(e) = ops_arith::handle_shl(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Shr => {
                    if let Err(e) = ops_arith::handle_shr(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::And => {
                    if let Err(e) = ops_arith::handle_and(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Or => {
                    if let Err(e) = ops_arith::handle_or(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Not => {
                    if let Err(e) = ops_arith::handle_not(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Neg => {
                    if let Err(e) = ops_arith::handle_neg(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Index => {
                    if let Err(e) = ops_struct::handle_index(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Slice => {
                    if let Err(e) = ops_struct::handle_slice(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::StoreIndex => {
                    if let Err(e) = ops_struct::handle_store_index(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::Attr(attr) => {
                    if let Err(e) = ops_struct::handle_attr(attr, &mut stack) {
                        break Err(e);
                    }
                }
                Instr::StoreAttr(attr) => {
                    if let Err(e) = ops_struct::handle_store_attr(attr, &mut stack) {
                        break Err(e);
                    }
                }
                Instr::Assert => {
                    if let Err(e) = ops_control::handle_assert(&mut stack) {
                        break Err(e);
                    }
                }
                Instr::CallValue(argc) => {
                    if let Err(e) = ops_control::handle_call_value(
                        *argc,
                        &mut stack,
                        funcs,
                        &mut env,
                        &mut env_stack,
                        &mut ret_stack,
                        &mut pc,
                        &mut advance_pc,
                    ) {
                        break Err(e);
                    }
                }
                Instr::PushNone => {
                    stack.push(Value::None);
                }
                Instr::Jump(target) => {
                    ops_control::handle_jump(*target, &mut pc, &mut advance_pc);
                }
                Instr::JumpIfFalse(target) => {
                    if let Err(e) = ops_control::handle_jump_if_false(
                        *target,
                        &mut stack,
                        &mut pc,
                        &mut advance_pc,
                    ) {
                        break Err(e);
                    }
                }
                Instr::Call(name) => {
                    if let Err(e) = ops_control::handle_call(
                        name,
                        funcs,
                        &mut stack,
                        &mut env,
                        &mut env_stack,
                        &mut ret_stack,
                        &mut pc,
                        &mut advance_pc,
                    ) {
                        break Err(e);
                    }
                }
                Instr::TailCall(name) => {
                    if let Err(e) = ops_control::handle_tail_call(
                        name,
                        funcs,
                        &mut stack,
                        &mut env,
                        &mut pc,
                        &mut advance_pc,
                    ) {
                        break Err(e);
                    }
                }
                Instr::CallBuiltin(name, argc) => {
                    if let Err(e) = ops_control::handle_call_builtin(
                        name,
                        *argc,
                        &mut stack,
                        &env,
                        globals,
                    ) {
                        break Err(e);
                    }
                }
                Instr::Pop => ops_control::handle_pop(&mut stack),
                Instr::Ret => {
                    if let Err(e) = ops_control::handle_ret(
                        &mut stack,
                        &mut pc,
                        &mut env,
                        &mut env_stack,
                        &mut ret_stack,
                        &mut advance_pc,
                    ) {
                        break Err(e);
                    }
                }
                Instr::Emit => ops_control::handle_emit(&mut stack),
                Instr::Halt => {
                    ops_control::handle_halt(code.len(), &mut pc, &mut advance_pc);
                }
                Instr::SetupExcept(target) => ops_control::handle_setup_except(
                    *target,
                    &stack,
                    &env_stack,
                    &ret_stack,
                    &mut block_stack,
                ),
                Instr::PopBlock => ops_control::handle_pop_block(&mut block_stack),
                Instr::Raise(kind) => {
                    break ops_control::handle_raise(kind, &mut stack);
                }
                Instr::StoreLocal(name) => {
                    // `alloc` semantics: always create a binding in the
                    // innermost scope without consulting outer scopes — so
                    // a local can shadow a global of the same name.
                    //
                    // Same-scope re-declaration is rejected at *compile
                    // time* (see compiler.rs / bootstrap/src/compiler.omg);
                    // at runtime an `alloc` inside a loop body legitimately
                    // re-runs every iteration. We always install a *fresh*
                    // cell so the new binding doesn't reach back into any
                    // closure that captured the previous value.
                    if let Some(v) = stack.pop() {
                        if env_stack.is_empty() {
                            globals.insert(name.clone(), v);
                        } else {
                            env.insert(name.clone(), new_cell(v));
                        }
                    }
                }
                Instr::MakeFunc(name) => {
                    if env_stack.is_empty() {
                        // Top-level: bind a non-capturing reference. We store
                        // a `Closure` (with empty captures) so callers always
                        // see the same value type whether they reference a
                        // top-level proc or a nested closure.
                        globals.insert(
                            name.clone(),
                            Value::Closure {
                                name: name.clone(),
                                captured: std::rc::Rc::new(HashMap::new()),
                            },
                        );
                    } else {
                        // Inside a function: capture the *cells* of the
                        // current local env, not their contents. Cloning a
                        // HashMap of Rcs just clones the Rcs — the closure
                        // and the enclosing scope end up referencing the
                        // same RefCells, so mutations flow both ways
                        // (Python/JS-style by-reference capture).
                        let captured: Rc<Env> = Rc::new(env.clone());
                        let closure = Value::Closure {
                            name: name.clone(),
                            captured,
                        };
                        env.insert(name.clone(), new_cell(closure.clone()));
                        // The compiler mangles names in imported modules
                        // with `__mod_N__` for module isolation, but
                        // source-level `Load` references the bare name.
                        // Bind both so a closure value passed across an
                        // import boundary resolves correctly.
                        if let Some(bare) = strip_mod_prefix(name) {
                            env.insert(bare.to_string(), new_cell(closure));
                        }
                    }
                }
            }
            break Ok(());
        };

        if let Err(e) = instr_res {
            error_flag = Some(e);
        }

        if let Some(err) = error_flag.take() {
            let mut handled = false;
            while let Some(block) = block_stack.pop() {
                while env_stack.len() > block.env_depth {
                    env = env_stack.pop().unwrap_or_default();
                    ret_stack.pop();
                }
                ret_stack.truncate(block.ret_depth);
                stack.truncate(block.stack_size);
                pc = block.handler;
                stack.push(Value::Str(err.to_string()));
                handled = true;
                break;
            }
            if !handled {
                return Err(err);
            } else {
                continue;
            }
        }

        if advance_pc {
            pc += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
