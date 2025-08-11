use std::collections::HashMap;

use crate::bytecode::Function;
use crate::error::{ErrorKind, RuntimeError};
use crate::value::Value;

use super::{call_builtin, pop, Block};

/// Handle the `ASSERT` instruction.
pub fn handle_assert(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let cond = pop(stack)?.as_bool();
    if !cond {
        return Err(RuntimeError::AssertionError);
    }
    Ok(())
}

/// Handle the `CALLVALUE` instruction.
pub fn handle_call_value(
    argc: usize,
    stack: &mut Vec<Value>,
    funcs: &HashMap<String, Function>,
    env_stack: &mut Vec<HashMap<String, Value>>,
    ret_stack: &mut Vec<usize>,
    env: &mut HashMap<String, Value>,
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
            env_stack.push(std::mem::take(env));
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

/// Handle the `PUSHNONE` instruction.
pub fn handle_push_none(stack: &mut Vec<Value>) {
    stack.push(Value::None);
}

/// Handle the `JUMP` instruction.
pub fn handle_jump(pc: &mut usize, target: usize, advance_pc: &mut bool) {
    *pc = target;
    *advance_pc = false;
}

/// Handle the `JUMPIFFALSE` instruction.
pub fn handle_jump_if_false(
    stack: &mut Vec<Value>,
    pc: &mut usize,
    target: usize,
    advance_pc: &mut bool,
) -> Result<(), RuntimeError> {
    let cond = pop(stack)?.as_bool();
    if !cond {
        *pc = target;
        *advance_pc = false;
    }
    Ok(())
}

/// Handle the `CALL` instruction.
pub fn handle_call(
    name: &str,
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
        env_stack.push(std::mem::take(env));
        ret_stack.push(*pc + 1);
        *env = new_env;
        *pc = func.address;
        *advance_pc = false;
        Ok(())
    } else {
        Err(RuntimeError::UndefinedIdentError(name.to_string()))
    }
}

/// Handle the `TAILCALL` instruction.
pub fn handle_tail_call(
    name: &str,
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
        Err(RuntimeError::UndefinedIdentError(name.to_string()))
    }
}

/// Handle the `CALLBUILTIN` instruction.
pub fn handle_call_builtin(
    name: &str,
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

/// Handle the `POP` instruction.
pub fn handle_pop(stack: &mut Vec<Value>) {
    stack.pop();
}

/// Handle the `RET` instruction.
pub fn handle_ret(
    stack: &mut Vec<Value>,
    ret_stack: &mut Vec<usize>,
    env_stack: &mut Vec<HashMap<String, Value>>,
    env: &mut HashMap<String, Value>,
    pc: &mut usize,
    advance_pc: &mut bool,
) {
    let ret_val = stack.pop().unwrap_or(Value::Int(0));
    *pc = ret_stack.pop().unwrap();
    *env = env_stack.pop().unwrap();
    stack.push(ret_val);
    *advance_pc = false;
}

/// Handle the `EMIT` instruction.
pub fn handle_emit(stack: &mut Vec<Value>) {
    if let Some(v) = stack.pop() {
        println!("{}", v.to_string());
    }
}

/// Handle the `HALT` instruction.
pub fn handle_halt(pc: &mut usize, code_len: usize, advance_pc: &mut bool) {
    *pc = code_len;
    *advance_pc = false;
}

/// Handle the `SETUPEXCEPT` instruction.
pub fn handle_setup_except(
    block_stack: &mut Vec<Block>,
    handler: usize,
    stack_size: usize,
    env_depth: usize,
    ret_depth: usize,
) {
    block_stack.push(Block {
        handler,
        stack_size,
        env_depth,
        ret_depth,
    });
}

/// Handle the `POPBLOCK` instruction.
pub fn handle_pop_block(block_stack: &mut Vec<Block>) {
    block_stack.pop();
}

/// Handle the `RAISE` instruction.
pub fn handle_raise(stack: &mut Vec<Value>, kind: ErrorKind) -> Result<(), RuntimeError> {
    let msg_val = stack.pop().ok_or_else(|| {
        RuntimeError::VmInvariant("stack underflow on RAISE".to_string())
    })?;
    let msg = msg_val.to_string();
    Err(kind.into_runtime(msg))
}
