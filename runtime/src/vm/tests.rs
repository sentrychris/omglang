use super::*;
use crate::bytecode::{Function, Instr};
use crate::error::RuntimeError;
use std::collections::HashMap;

#[test]
fn store_attr_on_frozen_dict_errors() {
    let code = vec![
        Instr::BuildDict(0),
        Instr::CallBuiltin("freeze".to_string(), 1),
        Instr::PushInt(1),
        Instr::StoreAttr("a".to_string()),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(result, Err(RuntimeError::FrozenWriteError));
}

#[test]
fn store_index_on_frozen_dict_errors() {
    let code = vec![
        Instr::BuildDict(0),
        Instr::CallBuiltin("freeze".to_string(), 1),
        Instr::PushStr("a".to_string()),
        Instr::PushInt(1),
        Instr::StoreIndex,
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(result, Err(RuntimeError::FrozenWriteError));
}

#[test]
fn raise_caught_in_caller() {
    let mut funcs = HashMap::new();
    funcs.insert(
        "boom".to_string(),
        Function {
            params: vec![],
            address: 7,
        },
    );
    let code = vec![
        Instr::SetupExcept(4),
        Instr::Call("boom".to_string()),
        Instr::PopBlock,
        Instr::Jump(6),
        Instr::Pop,
        Instr::Halt,
        Instr::Halt,
        // boom function
        Instr::PushStr("boom".to_string()),
        Instr::Raise,
        Instr::Ret,
    ];
    let result = run(&code, &funcs, &[]);
    assert!(result.is_ok());
}

#[test]
fn uncaught_raise_surfaces() {
    let code = vec![
        Instr::PushStr("boom".to_string()),
        Instr::Raise,
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(result, Err(RuntimeError::Raised("boom".to_string())));
}
