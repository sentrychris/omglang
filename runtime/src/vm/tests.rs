use super::*;
use crate::bytecode::Instr;
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
fn list_index_out_of_bounds_errors() {
    let code = vec![
        Instr::BuildList(0),
        Instr::PushInt(1),
        Instr::Index,
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert!(matches!(result, Err(RuntimeError::IndexError(_))));
}

#[test]
fn list_index_type_errors() {
    let code = vec![
        Instr::BuildList(0),
        Instr::PushStr("a".to_string()),
        Instr::Index,
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert!(matches!(result, Err(RuntimeError::TypeError(_))));
}

#[test]
fn dict_missing_key_errors() {
    let code = vec![
        Instr::BuildDict(0),
        Instr::PushStr("a".to_string()),
        Instr::Index,
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert!(matches!(result, Err(RuntimeError::KeyError(_))));
}
