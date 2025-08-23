//! # Stack-Based Bytecode Virtual Machine (Runtime)
//!
//! This module implements a compact, stack-based virtual machine that executes
//! a sequence of bytecode instructions (`Instr`) produced by the compiler.
//!
//! ## High-level model
//! - **Operand stack (`stack`)**: holds `Value`s consumed/produced by ops.
//! - **Global env (`globals`)**: process-wide, persists across function frames.
//! - **Local env (`env`)**: the current function’s locals (top of `env_stack`).
//! - **Env stack (`env_stack`)**: call frames for user-defined functions.
//! - **Return stack (`ret_stack`)**: return program counters for calls.
//! - **Block stack (`block_stack`)**: exception-handling frames capturing
//!   handler location and stack/env depths for unwinding.
//! - **Program counter (`pc`)**: index into `code` (the instruction stream).
//! - **Advance flag (`advance_pc`)**: lets control-flow ops manage the PC.
//!
//! The VM supports:
//! - Arithmetic/logical/bitwise ops (delegated to `ops_arith`)
//! - Structured operations (lists/dicts, indexing, slicing) via `ops_struct`
//! - Control flow (jumps, calls, returns, builtins, asserts, exceptions)
//! - Exception-style unwinding via `SetupExcept/PopBlock/Raise`
//!
//! The machine is deterministic and “fails fast”: any instruction error sets
//! `error_flag`, triggers block unwinding if a handler is present, or terminates
//! with a `RuntimeError` if unhandled.

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

/// # Exception-handling metadata for a protected region.
///
/// A `Block` is pushed by `SetupExcept` and popped by `PopBlock`. When a
/// `Raise` occurs, we unwind to the last block, restoring the operand stack
/// size, local env depth, and return stack depth that were current when the
/// block was set up, then jump to its `handler` PC.
///
/// Fields:
/// - `handler`: program counter to jump to if an error is raised
/// - `stack_size`: operand stack height to restore on unwind
/// - `env_depth`: number of local env frames to keep (truncate above this)
/// - `ret_depth`: number of return addresses to keep (truncate above this)
pub(super) struct Block {
    handler: usize,
    stack_size: usize,
    env_depth: usize,
    ret_depth: usize,
}

/// Pop a single [`Value`] from the operand stack.
///
/// Returns a VM invariant error on underflow. Prefer this helper when a caller
/// requires exactly one operand and wants a typed error message.
///
/// Note: Many operations pop multiple operands and do their own checks.
pub(super) fn pop(stack: &mut Vec<Value>) -> Result<Value, RuntimeError> {
    stack
        .pop()
        .ok_or_else(|| RuntimeError::VmInvariant("stack underflow".to_string()))
}

/// Execute bytecode on a stack-based virtual machine.
///
/// # Parameters
/// - `code`: the linear bytecode stream to execute
/// - `funcs`: user-defined function table (name → `Function`)
/// - `program_args`: CLI args; exposed as globals `args`, `module_file`, `current_dir`
///
/// # Returns
/// `Ok(())` on a clean halt (`Instr::Halt` or reaching end of `code`);
/// `Err(RuntimeError)` if an unhandled runtime error escapes.
///
/// # Runtime overview
/// The main loop fetches `code[pc]` and executes it. Most instructions set
/// `advance_pc = true`, and the VM increments `pc` after the step. Jumps/calls
/// explicitly set `advance_pc = false` and update `pc` themselves.
///
/// Error-producing operations return a `RuntimeError`. The VM captures it into
/// `error_flag`, attempts to unwind to the nearest exception `Block` (if any),
/// pushes the error message string to the operand stack for the handler to
/// consume, and resumes at the handler `pc`. Without a handler, the error ends
/// execution immediately.
pub fn run(
    code: &[Instr],
    funcs: &HashMap<String, Function>,
    program_args: &[String],
) -> Result<(), RuntimeError> {
    // Operand/value stack. All computation flows through here.
    let mut stack: Vec<Value> = Vec::new();

    // Global variables are visible to all frames. Locals live in `env`.
    let mut globals: HashMap<String, Value> = HashMap::new();

    // Expose command line arguments to programs as a list in `globals["args"]`.
    let arg_values: Vec<Value> = program_args.iter().map(|s| Value::Str(s.clone())).collect();
    globals.insert(
        "args".to_string(),
        Value::List(Rc::new(RefCell::new(arg_values))),
    );

    // Derive `module_file` and `current_dir` from the first argument if present,
    // else assume REPL-like execution.
    if let Some(first) = program_args.first() {
        let path = PathBuf::from(first.replace("\\", "/"));
        globals.insert(
            "module_file".to_string(),
            Value::Str(path.to_string_lossy().replace("\\", "/")),
        );
        if let Some(parent) = path.parent() {
            globals.insert(
                "current_dir".to_string(),
                Value::Str(parent.to_string_lossy().replace("\\", "/")),
            );
        } else {
            globals.insert("current_dir".to_string(), Value::Str(".".to_string()));
        }
    } else {
        globals.insert("module_file".to_string(), Value::Str("<stdin>".to_string()));
        globals.insert("current_dir".to_string(), Value::Str(".".to_string()));
    }

    // Current local environment (top frame) and the stack of saved locals.
    // Function calls push a new local env; returns restore the previous.
    let mut env: HashMap<String, Value> = HashMap::new();
    let mut env_stack: Vec<HashMap<String, Value>> = Vec::new();

    // Return address stack for user-defined function calls (stores PCs).
    let mut ret_stack: Vec<usize> = Vec::new();

    // Program counter: index of the current instruction.
    let mut pc: usize = 0;

    // Exception handling blocks for try/except-like semantics.
    let mut block_stack: Vec<Block> = Vec::new();

    // Pending error from an instruction, to be handled by a block or returned.
    let mut error_flag: Option<RuntimeError> = None;

    // === Fetch–Decode–Execute loop ===
    while pc < code.len() {
        // By default we advance to the next instruction after executing.
        // Control-flow ops (jumps, calls) will set this to false.
        let mut advance_pc = true;

        // Execute the current instruction, capturing any runtime error.
        let instr_res: Result<(), RuntimeError> = loop {
            match &code[pc] {
                // ----- Literals / Basic pushes -----
                Instr::PushInt(v) => stack.push(Value::Int(*v)),
                Instr::PushStr(s) => stack.push(Value::Str(s.clone())),
                Instr::PushBool(b) => stack.push(Value::Bool(*b)),
                // ----- Aggregate construction -----
                Instr::BuildList(n) => ops_struct::handle_build_list(*n, &mut stack)?,
                Instr::BuildDict(n) => ops_struct::handle_build_dict(*n, &mut stack)?,
                // ----- Variable load/store with lexical/global resolution -----
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
                    // Store precedence:
                    // 1) If no local frames, write to globals
                    // 2) If name exists in current locals, overwrite local
                    // 3) Else if exists in globals, overwrite global
                    // 4) Else (new symbol within a function), create local
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
                // ----- Arithmetic / Comparison / Bitwise / Boolean -----
                Instr::Add => ops_arith::handle_add(&mut stack)?,
                Instr::Sub => ops_arith::handle_sub(&mut stack)?,
                Instr::Mul => ops_arith::handle_mul(&mut stack)?,
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
                Instr::Eq => ops_arith::handle_eq(&mut stack)?,
                Instr::Ne => ops_arith::handle_ne(&mut stack)?,
                Instr::Lt => ops_arith::handle_lt(&mut stack)?,
                Instr::Le => ops_arith::handle_le(&mut stack)?,
                Instr::Gt => ops_arith::handle_gt(&mut stack)?,
                Instr::Ge => ops_arith::handle_ge(&mut stack)?,
                Instr::BAnd => ops_arith::handle_band(&mut stack)?,
                Instr::BOr => ops_arith::handle_bor(&mut stack)?,
                Instr::BXor => ops_arith::handle_bxor(&mut stack)?,
                Instr::Shl => ops_arith::handle_shl(&mut stack)?,
                Instr::Shr => ops_arith::handle_shr(&mut stack)?,
                Instr::And => ops_arith::handle_and(&mut stack)?,
                Instr::Or => ops_arith::handle_or(&mut stack)?,
                Instr::Not => ops_arith::handle_not(&mut stack)?,
                Instr::Neg => ops_arith::handle_neg(&mut stack)?,
                // ----- Indexing / slicing / attribute access -----
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
                // ----- Assertions / exceptions -----
                Instr::Assert => {
                    if let Err(e) = ops_control::handle_assert(&mut stack) {
                        break Err(e);
                    }
                }
                // ----- Calls (first-class & named), tail calls, builtins -----
                Instr::CallValue(argc) => {
                    // Pops `argc` args + 1 callee from the stack and dispatches.
                    // May push new env and return address; sets PC/advance flag.
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
                    // Unconditional jump. Mutates `pc` and disables auto-advance.
                    ops_control::handle_jump(*target, &mut pc, &mut advance_pc);
                }
                Instr::JumpIfFalse(target) => {
                    // Conditionally jump if top-of-stack is falsey.
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
                    // Call a named function. Pushes frame + return address.
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
                    // Tail-call optimization: reuse current frame where possible.
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
                    // Invoke a builtin by name with `argc` args sourced from stack.
                    if let Err(e) = ops_control::handle_call_builtin(
                        name,
                        *argc,
                        &mut stack,
                        &env,
                        &globals,
                    ) {
                        break Err(e);
                    }
                }
                // ----- Misc stack / control -----
                Instr::Pop => {
                    // Discard top-of-stack (no error if empty; handler decides).
                    ops_control::handle_pop(&mut stack);
                }
                Instr::Ret => {
                     // Return from current function frame. Restores env and PC.
                    ops_control::handle_ret(
                        &mut stack,
                        &mut pc,
                        &mut env,
                        &mut env_stack,
                        &mut ret_stack,
                        &mut advance_pc,
                    );
                }
                Instr::Emit => {
                    // Print top-of-stack (implementation-defined output).
                    ops_control::handle_emit(&mut stack);
                }
                Instr::Halt => {
                    // Force termination by jumping to end of code.
                    ops_control::handle_halt(code.len(), &mut pc, &mut advance_pc);
                }
                // ----- Structured exception handling blocks -----
                Instr::SetupExcept(target) => {
                    // Push a handler frame capturing current depths and a handler PC.
                    ops_control::handle_setup_except(
                        *target,
                        &stack,
                        &env_stack,
                        &ret_stack,
                        &mut block_stack,
                    );
                }
                Instr::PopBlock => {
                    // Pop the most recent exception handler (leaving state as-is).
                    ops_control::handle_pop_block(&mut block_stack);
                }
                Instr::Raise(kind) => {
                    // Raise an exception; converts to `RuntimeError`, bubbled to VM.
                    break ops_control::handle_raise(kind, &mut stack);
                }
            }
            // If we got here, the instruction completed without error.
            break Ok(());
        };

        // Capture any fault from the just-executed instruction.
        if let Err(e) = instr_res {
            error_flag = Some(e);
        }

        // If an error occurred, attempt to unwind to the nearest handler.
        if let Some(err) = error_flag.take() {
            let mut handled = false;
            // Pop blocks until one handles the error. For the first viable block:
            // - restore env/ret/stack depths
            // - jump to its handler
            // - push the error message string as the handler’s input
            while let Some(block) = block_stack.pop() {
                // Restore local frames to the captured depth.
                while env_stack.len() > block.env_depth {
                    env = env_stack.pop().unwrap();
                    ret_stack.pop();
                }
                // Ensure return addresses match the captured depth.
                ret_stack.truncate(block.ret_depth);
                // Restore operand stack height.
                stack.truncate(block.stack_size);
                // Transfer control to handler and provide error info.
                pc = block.handler;
                stack.push(Value::Str(err.to_string()));
                handled = true;
                break;
            }

            // No handler: abort with the original error.
            if !handled {
                return Err(err);
            } else {
                // We transferred control to a handler; do not auto-advance PC.
                continue;
            }
        }

        // Normal flow: move to the next instruction unless a jump/call overrode it.
        if advance_pc {
            pc += 1;
        }
    }
    // Graceful termination (fell off end or `Halt` moved PC beyond code).
    Ok(())
}

#[cfg(test)]
mod tests;
