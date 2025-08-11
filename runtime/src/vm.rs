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

pub(super) struct Block {
    handler: usize,
    stack_size: usize,
    env_depth: usize,
    ret_depth: usize,
}

pub(super) fn pop(stack: &mut Vec<Value>) -> Result<Value, RuntimeError> {
    stack
        .pop()
        .ok_or_else(|| RuntimeError::VmInvariant("stack underflow".to_string()))
}

/// Execute bytecode on a stack-based virtual machine.
pub fn run(
    code: &[Instr],
    funcs: &HashMap<String, Function>,
    program_args: &[String],
) -> Result<(), RuntimeError> {
    let mut stack: Vec<Value> = Vec::new();
    let mut globals: HashMap<String, Value> = HashMap::new();
    // Expose command line arguments to bytecode programs via the global `args` list
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
    let mut env: HashMap<String, Value> = HashMap::new();
    let mut env_stack: Vec<HashMap<String, Value>> = Vec::new();
    let mut ret_stack: Vec<usize> = Vec::new();
    let mut pc: usize = 0;
    let mut block_stack: Vec<Block> = Vec::new();
    let mut error_flag: Option<RuntimeError> = None;
    while pc < code.len() {
        let mut advance_pc = true;
        let instr_res: Result<(), RuntimeError> = loop {
            match &code[pc] {
                Instr::PushInt(v) => stack.push(Value::Int(*v)),
                Instr::PushStr(s) => stack.push(Value::Str(s.clone())),
                Instr::PushBool(b) => stack.push(Value::Bool(*b)),
                Instr::BuildList(n) => ops_struct::handle_build_list(*n, &mut stack)?,
                Instr::BuildDict(n) => ops_struct::handle_build_dict(*n, &mut stack)?,
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
                        &globals,
                    ) {
                        break Err(e);
                    }
                }
                Instr::Pop => {
                    ops_control::handle_pop(&mut stack);
                }
                Instr::Ret => {
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
                    ops_control::handle_emit(&mut stack);
                }
                Instr::Halt => {
                    ops_control::handle_halt(code.len(), &mut pc, &mut advance_pc);
                }
                Instr::SetupExcept(target) => {
                    ops_control::handle_setup_except(
                        *target,
                        &stack,
                        &env_stack,
                        &ret_stack,
                        &mut block_stack,
                    );
                }
                Instr::PopBlock => {
                    ops_control::handle_pop_block(&mut block_stack);
                }
                Instr::Raise(kind) => {
                    break ops_control::handle_raise(kind, &mut stack);
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
                    env = env_stack.pop().unwrap();
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
