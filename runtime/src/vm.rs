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
use std::path::PathBuf;
use std::rc::Rc;

use crate::bytecode::{Function, Instr};
use crate::error::RuntimeError;
use crate::value::Value;

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
    if let Some(first) = program_args.first() {
        let path = PathBuf::from(first.replace("\\", "/"));
        globals.insert(
            "module_file".to_string(),
            Value::Str(path.to_string_lossy().replace("\\", "/")),
        );
        if let Some(parent) = path.parent() {
            let s = parent.to_string_lossy().replace("\\", "/");
            globals.insert(
                "current_dir".to_string(),
                Value::Str(if s.is_empty() {
                    ".".to_string()
                } else {
                    s
                }),
            );
        } else {
            globals.insert("current_dir".to_string(), Value::Str(".".to_string()));
        }
    } else {
        globals.insert("module_file".to_string(), Value::Str("<stdin>".to_string()));
        globals.insert("current_dir".to_string(), Value::Str(".".to_string()));
    }
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
    execute(code, funcs_persistent, globals)
}

fn execute(
    code: &[Instr],
    funcs: &HashMap<String, Function>,
    globals: &mut HashMap<String, Value>,
) -> Result<(), RuntimeError> {
    let mut stack: Vec<Value> = Vec::new();
    let mut env: HashMap<String, Value> = HashMap::new();
    let mut env_stack: Vec<HashMap<String, Value>> = Vec::new();
    let mut ret_stack: Vec<usize> = Vec::new();
    let mut block_stack: Vec<Block> = Vec::new();
    let mut error_flag: Option<RuntimeError> = None;
    let mut pc: usize = 0;

    while pc < code.len() {
        let mut advance_pc = true;

        let instr_res: Result<(), RuntimeError> = loop {
            match &code[pc] {
                Instr::PushInt(v) => stack.push(Value::Int(*v)),
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
                    if let Some(v) = env.get(name) {
                        stack.push(v.clone());
                    } else if let Some(v) = globals.get(name) {
                        stack.push(v.clone());
                    } else {
                        break Err(RuntimeError::UndefinedIdentError(name.clone()));
                    }
                }
                Instr::Store(name) => {
                    if let Some(v) = stack.pop() {
                        if env_stack.is_empty() {
                            globals.insert(name.clone(), v);
                        } else if env.contains_key(name) {
                            env.insert(name.clone(), v);
                        } else if globals.contains_key(name) {
                            globals.insert(name.clone(), v);
                        } else {
                            env.insert(name.clone(), v);
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
                    // innermost scope without consulting globals.  Prevents
                    // accidental clobbering of runtime-injected globals
                    // (`args`, `module_file`, `current_dir`) by user locals
                    // that happen to share the name.
                    if let Some(v) = stack.pop() {
                        if env_stack.is_empty() {
                            globals.insert(name.clone(), v);
                        } else {
                            env.insert(name.clone(), v);
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
                                captured: std::rc::Rc::new(std::collections::HashMap::new()),
                            },
                        );
                    } else {
                        // Inside a function: capture a snapshot of the
                        // current local env. The compiler always emits
                        // `MakeFunc` immediately after the function body has
                        // been registered, so the closure is constructed
                        // exactly where the user wrote `proc`.
                        let captured = std::rc::Rc::new(env.clone());
                        env.insert(
                            name.clone(),
                            Value::Closure {
                                name: name.clone(),
                                captured,
                            },
                        );
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
