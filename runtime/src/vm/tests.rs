use super::*;
use crate::bytecode::{Function, Instr};
use crate::error::{ErrorKind, RuntimeError};
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
        Instr::Raise(ErrorKind::Generic),
        Instr::Ret,
    ];
    let result = run(&code, &funcs, &[]);
    assert!(result.is_ok());
}

#[test]
fn uncaught_raise_surfaces() {
    let code = vec![
        Instr::PushStr("boom".to_string()),
        Instr::Raise(ErrorKind::Generic),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(result, Err(RuntimeError::Raised("boom".to_string())));
}

#[test]
fn uncaught_syntax_error_surfaces() {
    let code = vec![
        Instr::PushStr("boom".to_string()),
        Instr::Raise(ErrorKind::Syntax),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(result, Err(RuntimeError::SyntaxError("boom".to_string())));
}

#[test]
fn uncaught_type_error_surfaces() {
    let code = vec![
        Instr::PushStr("boom".to_string()),
        Instr::Raise(ErrorKind::Type),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(result, Err(RuntimeError::TypeError("boom".to_string())));
}

#[test]
fn uncaught_undef_ident_error_surfaces() {
    let code = vec![
        Instr::PushStr("boom".to_string()),
        Instr::Raise(ErrorKind::UndefinedIdent),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::UndefinedIdentError("boom".to_string()))
    );
}

#[test]
fn raise_stack_underflow_errors() {
    let code = vec![Instr::Raise(ErrorKind::Generic), Instr::Halt];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::VmInvariant(
            "stack underflow on RAISE".to_string()
        ))
    );
}

#[test]
fn uncaught_assert_surfaces() {
    let code = vec![Instr::PushBool(false), Instr::Assert, Instr::Halt];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(result, Err(RuntimeError::AssertionError));
}

#[test]
fn assert_caught_in_block() {
    let code = vec![
        Instr::SetupExcept(5),
        Instr::PushBool(false),
        Instr::Assert,
        Instr::PopBlock,
        Instr::Jump(7),
        Instr::Pop,
        Instr::Halt,
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert!(result.is_ok());
}

#[test]
fn hex_with_string_type_error() {
    let code = vec![
        Instr::PushStr("foo".to_string()),
        Instr::CallBuiltin("hex".to_string(), 1),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::TypeError(
            "hex() expects one integer (arity mismatch)".to_string()
        ))
    );
}

#[test]
fn binary_with_string_type_error() {
    let code = vec![
        Instr::PushStr("foo".to_string()),
        Instr::CallBuiltin("binary".to_string(), 1),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::TypeError(
            "binary() expects one or two integers (arity mismatch)".to_string()
        ))
    );
}

#[test]
fn binary_with_non_positive_width_type_error() {
    let code = vec![
        Instr::PushInt(5),
        Instr::PushInt(0),
        Instr::CallBuiltin("binary".to_string(), 2),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::ValueError(
            "binary() width must be positive".to_string()
        ))
    );
}

#[test]
fn length_with_int_type_error() {
    let code = vec![
        Instr::PushInt(5),
        Instr::CallBuiltin("length".to_string(), 1),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::TypeError(
            "length() expects list or string (type mismatch)".to_string()
        ))
    );
}

#[test]
fn call_builtin_dispatches_hex() {
    let code = vec![
        Instr::PushStr("hex".to_string()),
        Instr::PushInt(255),
        Instr::BuildList(1),
        Instr::CallBuiltin("call_builtin".to_string(), 2),
        Instr::PushStr("ff".to_string()),
        Instr::Eq,
        Instr::Assert,
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert!(result.is_ok());
}

#[test]
fn load_unknown_name_errors() {
    let code = vec![Instr::Load("foo".to_string()), Instr::Halt];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::UndefinedIdentError("foo".to_string()))
    );
}

#[test]
fn call_undefined_function_errors() {
    let code = vec![Instr::Call("foo".to_string()), Instr::Halt];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::UndefinedIdentError("foo".to_string()))
    );
}

#[test]
fn tail_call_undefined_function_errors() {
    let code = vec![Instr::TailCall("foo".to_string()), Instr::Halt];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::UndefinedIdentError("foo".to_string()))
    );
}

#[test]
fn call_value_type_error_when_non_string() {
    let code = vec![Instr::PushInt(0), Instr::CallValue(0), Instr::Halt];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::TypeError(
            "Call value expects function name".to_string(),
        ))
    );
}

#[test]
fn call_value_unknown_function_errors() {
    let code = vec![
        Instr::PushStr("foo".to_string()),
        Instr::CallValue(0),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::UndefinedIdentError("foo".to_string()))
    );
}

#[test]
fn list_slice_with_invalid_bounds_errors() {
    let code = vec![
        Instr::BuildList(0),
        Instr::PushInt(1),
        Instr::PushInt(0),
        Instr::Slice,
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::IndexError(
            "Slice indices out of bounds!".to_string()
        ))
    );
}

#[test]
fn string_slice_with_invalid_bounds_errors() {
    let code = vec![
        Instr::PushStr("ab".to_string()),
        Instr::PushInt(0),
        Instr::PushInt(3),
        Instr::Slice,
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::IndexError(
            "Slice indices out of bounds!".to_string()
        ))
    );
}

#[test]
fn neg_on_non_int_string_errors() {
    let code = vec![Instr::PushStr("abc".to_string()), Instr::Neg, Instr::Halt];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &[]);
    assert_eq!(
        result,
        Err(RuntimeError::TypeError(
            "Invalid literal for int(): 'abc'".to_string(),
        )),
    );
}