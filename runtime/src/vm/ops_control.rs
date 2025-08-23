//! # Control-Flow and Function Operations for the OMG VM
//!
//! This module implements all non-arithmetic VM instructions related to:
//! - **Assertions** (`assert`)
//! - **Jumps and branches** (unconditional / conditional)
//! - **Function calls** (direct, tail, first-class, builtins)
//! - **Stack control** (`pop`, return value handling)
//! - **Program termination** (`halt`)
//! - **I/O** (`emit`)
//! - **Exception handling** (`setup_except`, `pop_block`, `raise`)
//!
//! ## Execution model
//! - Handlers operate directly on the operand stack (`Vec<Value>`) and update
//!   control registers like the program counter (`pc`) and advance flag
//!   (`advance_pc`).
//! - Call instructions manipulate:
//!   - `env`: current local variables
//!   - `env_stack`: previous local scopes
//!   - `ret_stack`: return addresses
//! - Exceptions use `Block` frames (capturing stack/env depth and handler PC).
//!
//! ## Notes
//! - All `handle_*` functions return `Result<(), RuntimeError>` when an error
//!   can occur (type errors, undefined functions, assertion failure, bad raise).
//! - Jumps and calls disable `advance_pc` so the main loop does not auto-advance
//!   the PC afterward.
//! - `handle_ret` restores both the previous environment and program counter,
//!   then pushes the return value back to the operand stack.

use std::collections::HashMap;
use std::mem;

use crate::bytecode::Function;
use crate::error::{ErrorKind, RuntimeError};
use crate::value::Value;
use super::{pop, Block};
use super::builtins::call_builtin;

/// Handle `assert`: pops a boolean condition; errors if false.
pub(super) fn handle_assert(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let cond = pop(stack)?.as_bool();
    if !cond {
        return Err(RuntimeError::AssertionError);
    }
    Ok(())
}

/// Handle unconditional jump: set PC to target and disable auto-advance.
pub(super) fn handle_jump(target: usize, pc: &mut usize, advance_pc: &mut bool) {
    *pc = target;
    *advance_pc = false;
}

/// Handle conditional jump: pop condition; if false, set PC to target.
pub(super) fn handle_jump_if_false(
    target: usize,
    stack: &mut Vec<Value>,
    pc: &mut usize,
    advance_pc: &mut bool,
) -> Result<(), RuntimeError> {
    let cond = pop(stack)?.as_bool();
    if !cond {
        *pc = target;
        *advance_pc = false;
    }
    Ok(())
}

/// Handle first-class call: pop callee (must be a string function name) + args,
/// push new environment, save return address, and jump to function address.
pub(super) fn handle_call_value(
    argc: usize,
    stack: &mut Vec<Value>,
    funcs: &HashMap<String, Function>,
    env: &mut HashMap<String, Value>,
    env_stack: &mut Vec<HashMap<String, Value>>,
    ret_stack: &mut Vec<usize>,
    pc: &mut usize,
    advance_pc: &mut bool,
) -> Result<(), RuntimeError> {
    let mut args_vec: Vec<Value> = Vec::new();
    for _ in 0..argc {
        args_vec.push(pop(stack)?);
    }
    args_vec.reverse();
    let func_val = pop(stack)?;
    if let Value::Str(name) = func_val {
        if let Some(func) = funcs.get(&name) {
            let mut new_env = HashMap::new();
            for param in func.params.iter().rev() {
                let arg = args_vec.pop().unwrap();
                new_env.insert(param.clone(), arg);
            }
            env_stack.push(mem::take(env));
            ret_stack.push(*pc + 1);
            *env = new_env;
            *pc = func.address;
            *advance_pc = false;
            Ok(())
        } else {
            Err(RuntimeError::UndefinedIdentError(name))
        }
    } else {
        Err(RuntimeError::TypeError(
            "Call value expects function name".to_string(),
        ))
    }
}

/// Handle direct named function call (like `call foo`).
pub(super) fn handle_call(
    name: &String,
    funcs: &HashMap<String, Function>,
    stack: &mut Vec<Value>,
    env: &mut HashMap<String, Value>,
    env_stack: &mut Vec<HashMap<String, Value>>,
    ret_stack: &mut Vec<usize>,
    pc: &mut usize,
    advance_pc: &mut bool,
) -> Result<(), RuntimeError> {
    if let Some(func) = funcs.get(name) {
        let mut new_env = HashMap::new();
        for param in func.params.iter().rev() {
            let arg = pop(stack)?;
            new_env.insert(param.clone(), arg);
        }
        env_stack.push(mem::take(env));
        ret_stack.push(*pc + 1);
        *env = new_env;
        *pc = func.address;
        *advance_pc = false;
        Ok(())
    } else {
        Err(RuntimeError::UndefinedIdentError(name.clone()))
    }
}

/// Handle tail-call optimization: reuse current frame (no push to env_stack/ret_stack).
pub(super) fn handle_tail_call(
    name: &String,
    funcs: &HashMap<String, Function>,
    stack: &mut Vec<Value>,
    env: &mut HashMap<String, Value>,
    pc: &mut usize,
    advance_pc: &mut bool,
) -> Result<(), RuntimeError> {
    if let Some(func) = funcs.get(name) {
        let mut new_env = HashMap::new();
        for param in func.params.iter().rev() {
            let arg = pop(stack)?;
            new_env.insert(param.clone(), arg);
        }
        *env = new_env;
        *pc = func.address;
        *advance_pc = false;
        Ok(())
    } else {
        Err(RuntimeError::UndefinedIdentError(name.clone()))
    }
}

/// Handle call to builtin function: pops arguments, invokes dispatcher, pushes result.
pub(super) fn handle_call_builtin(
    name: &String,
    argc: usize,
    stack: &mut Vec<Value>,
    env: &HashMap<String, Value>,
    globals: &HashMap<String, Value>,
) -> Result<(), RuntimeError> {
    let mut args: Vec<Value> = Vec::new();
    for _ in 0..argc {
        args.push(pop(stack)?);
    }
    args.reverse();
    match call_builtin(name, &args, env, globals) {
        Ok(val) => {
            stack.push(val);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Handle `pop`: discard the top of the stack if present.
pub(super) fn handle_pop(stack: &mut Vec<Value>) {
    stack.pop();
}

/// Handle `ret`: restore caller’s PC + environment and push return value.
pub(super) fn handle_ret(
    stack: &mut Vec<Value>,
    pc: &mut usize,
    env: &mut HashMap<String, Value>,
    env_stack: &mut Vec<HashMap<String, Value>>,
    ret_stack: &mut Vec<usize>,
    advance_pc: &mut bool,
) {
    let ret_val = stack.pop().unwrap_or(Value::Int(0));
    *pc = ret_stack.pop().unwrap();
    *env = env_stack.pop().unwrap();
    stack.push(ret_val);
    *advance_pc = false;
}

/// Handle `emit`: pop and print top-of-stack.
pub(super) fn handle_emit(stack: &mut Vec<Value>) {
    if let Some(v) = stack.pop() {
        println!("{}", v.to_string());
    }
}

/// Handle `halt`: set PC beyond code length to stop execution.
pub(super) fn handle_halt(code_len: usize, pc: &mut usize, advance_pc: &mut bool) {
    *pc = code_len;
    *advance_pc = false;
}

/// Handle `setup_except`: push an exception handler block.
pub(super) fn handle_setup_except(
    target: usize,
    stack: &Vec<Value>,
    env_stack: &Vec<HashMap<String, Value>>,
    ret_stack: &Vec<usize>,
    block_stack: &mut Vec<Block>,
) {
    block_stack.push(Block {
        handler: target,
        stack_size: stack.len(),
        env_depth: env_stack.len(),
        ret_depth: ret_stack.len(),
    });
}

/// Handle `pop_block`: discard most recent exception handler.
pub(super) fn handle_pop_block(block_stack: &mut Vec<Block>) {
    block_stack.pop();
}

/// Handle `raise`: pop message value and raise a runtime error of given kind.
pub(super) fn handle_raise(
    kind: &ErrorKind,
    stack: &mut Vec<Value>,
) -> Result<(), RuntimeError> {
    let msg_val = match stack.pop() {
        Some(v) => v,
        None => {
            return Err(RuntimeError::VmInvariant(
                "stack underflow on RAISE".to_string(),
            ));
        }
    };
    let msg = msg_val.to_string();
    Err(kind.into_runtime(msg))
}
