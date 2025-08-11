use std::collections::HashMap;
use std::mem;

use crate::bytecode::Function;
use crate::error::{ErrorKind, RuntimeError};
use crate::value::Value;
use super::{pop, Block};
use super::builtins::call_builtin;

pub(super) fn handle_assert(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let cond = pop(stack)?.as_bool();
    if !cond {
        return Err(RuntimeError::AssertionError);
    }
    Ok(())
}

pub(super) fn handle_jump(target: usize, pc: &mut usize, advance_pc: &mut bool) {
    *pc = target;
    *advance_pc = false;
}

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

pub(super) fn handle_pop(stack: &mut Vec<Value>) {
    stack.pop();
}

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

pub(super) fn handle_emit(stack: &mut Vec<Value>) {
    if let Some(v) = stack.pop() {
        println!("{}", v.to_string());
    }
}

pub(super) fn handle_halt(code_len: usize, pc: &mut usize, advance_pc: &mut bool) {
    *pc = code_len;
    *advance_pc = false;
}

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

pub(super) fn handle_pop_block(block_stack: &mut Vec<Block>) {
    block_stack.pop();
}

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
