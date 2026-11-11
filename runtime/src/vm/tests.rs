use super::*;
use crate::bytecode::{Function, Instr, SourceMap};
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
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
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
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
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
            source_file_idx: u32::MAX,
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
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
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
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
    assert_eq!(result, Err(RuntimeError::Raised("boom".to_string())));
}

#[test]
fn panic_builtin_raises() {
    let code = vec![
        Instr::PushStr("boom".to_string()),
        Instr::CallBuiltin("panic".to_string(), 1),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
    assert_eq!(result, Err(RuntimeError::Raised("boom".to_string())));
}

#[test]
fn read_file_missing_raises_module_import_error() {
    let code = vec![
        Instr::PushStr("no_such_file.omg".to_string()),
        Instr::CallBuiltin("read_file".to_string(), 1),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
    match result {
        Err(RuntimeError::ModuleImportError(msg)) => {
            assert!(msg.contains("no_such_file.omg"));
        }
        other => panic!("expected ModuleImportError, got {:?}", other),
    }
}

#[test]
fn uncaught_syntax_error_surfaces() {
    let code = vec![
        Instr::PushStr("boom".to_string()),
        Instr::Raise(ErrorKind::Syntax),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
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
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
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
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
    assert_eq!(
        result,
        Err(RuntimeError::UndefinedIdentError("boom".to_string()))
    );
}

#[test]
fn raise_stack_underflow_errors() {
    let code = vec![Instr::Raise(ErrorKind::Generic), Instr::Halt];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
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
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
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
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
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
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
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
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
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
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
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
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
    assert_eq!(
        result,
        Err(RuntimeError::TypeError(
            "length() expects list, string or dict (type mismatch)".to_string()
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
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
    assert!(result.is_ok());
}

#[test]
fn call_builtin_dispatches_raise() {
    let code = vec![
        Instr::PushStr("raise".to_string()),
        Instr::PushStr("boom".to_string()),
        Instr::BuildList(1),
        Instr::CallBuiltin("call_builtin".to_string(), 2),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
    assert_eq!(result, Err(RuntimeError::Raised("boom".to_string())));
}

#[test]
fn load_unknown_name_errors() {
    let code = vec![Instr::Load("foo".to_string()), Instr::Halt];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
    assert_eq!(
        result,
        Err(RuntimeError::UndefinedIdentError("foo".to_string()))
    );
}

#[test]
fn call_undefined_function_errors() {
    let code = vec![Instr::Call("foo".to_string()), Instr::Halt];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
    assert_eq!(
        result,
        Err(RuntimeError::UndefinedIdentError("foo".to_string()))
    );
}

#[test]
fn tail_call_undefined_function_errors() {
    let code = vec![Instr::TailCall("foo".to_string()), Instr::Halt];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
    assert_eq!(
        result,
        Err(RuntimeError::UndefinedIdentError("foo".to_string()))
    );
}

#[test]
fn call_value_type_error_when_non_string() {
    let code = vec![Instr::PushInt(0), Instr::CallValue(0), Instr::Halt];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
    // Now that CallValue accepts both `Value::Str` (named function) and
    // `Value::Closure`, the error message reports the rejected operand
    // instead of a generic message.
    assert!(matches!(result, Err(RuntimeError::TypeError(_))));
}

#[test]
fn call_value_unknown_function_errors() {
    let code = vec![
        Instr::PushStr("foo".to_string()),
        Instr::CallValue(0),
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
    assert_eq!(
        result,
        Err(RuntimeError::UndefinedIdentError("foo".to_string()))
    );
}

#[test]
fn list_slice_with_inverted_bounds_yields_empty() {
    // Python semantics: [a:b] where a > b returns an empty slice rather
    // than raising. The VM now matches that, so this just runs cleanly.
    let code = vec![
        Instr::BuildList(0),
        Instr::PushInt(1),
        Instr::PushInt(0),
        Instr::Slice,
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    assert!(run(&code, &funcs, &SourceMap::default(), &[]).is_ok());
}

#[test]
fn string_slice_clamps_out_of_range_end() {
    // "ab"[0:3] is "ab" in Python; bounds clamp instead of erroring.
    let code = vec![
        Instr::PushStr("ab".to_string()),
        Instr::PushInt(0),
        Instr::PushInt(3),
        Instr::Slice,
        Instr::Halt,
    ];
    let funcs = HashMap::new();
    assert!(run(&code, &funcs, &SourceMap::default(), &[]).is_ok());
}

#[test]
fn neg_on_non_int_string_errors() {
    let code = vec![Instr::PushStr("abc".to_string()), Instr::Neg, Instr::Halt];
    let funcs = HashMap::new();
    let result = run(&code, &funcs, &SourceMap::default(), &[]);
    assert_eq!(
        result,
        Err(RuntimeError::TypeError(
            "Invalid literal for int(): 'abc'".to_string(),
        )),
    );
}

// --- Integer division & modulo: floor semantics --------------------------
//
// Locks in the documented behaviour: `/` and `//` round toward minus
// infinity on int/int, and `%` produces a result with the sign of the
// divisor (Python `%`). Floor and Euclidean only diverge when the
// divisor is negative, so those are the cases that matter.

fn run_int_binop_int(
    op: fn(&mut Vec<Value>) -> Result<(), RuntimeError>,
    a: i64,
    b: i64,
) -> i64 {
    let mut stack = vec![Value::Int(a), Value::Int(b)];
    op(&mut stack).expect("binop ok");
    match stack.pop().expect("binop pushed a result") {
        Value::Int(i) => i,
        _ => panic!("expected Int result"),
    }
}

fn run_int_binop_float(
    op: fn(&mut Vec<Value>) -> Result<(), RuntimeError>,
    a: i64,
    b: i64,
) -> f64 {
    let mut stack = vec![Value::Int(a), Value::Int(b)];
    op(&mut stack).expect("binop ok");
    match stack.pop().expect("binop pushed a result") {
        Value::Float(f) => f,
        _ => panic!("expected Float result"),
    }
}

#[test]
fn int_div_is_true_division() {
    use super::ops_arith::handle_div;
    // Python 3 semantics: int / int always returns a float.
    assert_eq!(run_int_binop_float(handle_div, 7, 2), 3.5);
    assert_eq!(run_int_binop_float(handle_div, -7, 2), -3.5);
    assert_eq!(run_int_binop_float(handle_div, 7, -2), -3.5);
    assert_eq!(run_int_binop_float(handle_div, -7, -2), 3.5);
    assert_eq!(run_int_binop_float(handle_div, 10, 5), 2.0);
}

#[test]
fn int_floor_div_floors_toward_negative_infinity() {
    use super::ops_arith::handle_floor_div;
    assert_eq!(run_int_binop_int(handle_floor_div, 7, 2), 3);
    assert_eq!(run_int_binop_int(handle_floor_div, -7, 2), -4);
    assert_eq!(run_int_binop_int(handle_floor_div, 7, -2), -4);
    assert_eq!(run_int_binop_int(handle_floor_div, -7, -2), 3);
}

#[test]
fn int_mod_takes_sign_of_divisor() {
    use super::ops_arith::handle_mod;
    assert_eq!(run_int_binop_int(handle_mod, 7, 2), 1);
    assert_eq!(run_int_binop_int(handle_mod, -7, 2), 1);
    assert_eq!(run_int_binop_int(handle_mod, 7, -2), -1);
    assert_eq!(run_int_binop_int(handle_mod, -7, -2), -1);
}
// --- Builtin regression tests ------------------------------------------------

fn call(name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
    builtins::call_builtin(name, args, &Env::new(), &HashMap::new())
}

fn as_str(v: Result<Value, RuntimeError>) -> String {
    match v {
        Ok(v) => v.to_string(),
        Err(e) => panic!("builtin failed: {}", e),
    }
}

#[test]
fn round_only_treats_exact_halves_as_ties() {
    let r = |f: f64| as_str(call("round", &[Value::Float(f)]));
    // Exact ties go to the even neighbour...
    assert_eq!(r(0.5), "0");
    assert_eq!(r(1.5), "2");
    assert_eq!(r(2.5), "2");
    assert_eq!(r(3.5), "4");
    assert_eq!(r(-0.5), "0");
    assert_eq!(r(-1.5), "-2");
    assert_eq!(r(-2.5), "-2");
    // ...but values that are merely *near* a tie round normally.
    assert_eq!(r(0.5000000001), "1");
    assert_eq!(r(1.4999999999), "1");
    assert_eq!(r(2.5000000001), "3");
    assert_eq!(r(-1.4999999999), "-1");
    assert_eq!(r(2.4), "2");
    assert_eq!(r(2.6), "3");
}

#[test]
fn hex_and_binary_use_a_sign_prefix_for_negatives() {
    assert_eq!(as_str(call("hex", &[Value::Int(255)])), "ff");
    assert_eq!(as_str(call("hex", &[Value::Int(-255)])), "-ff");
    assert_eq!(as_str(call("hex", &[Value::Int(0)])), "0");
    assert_eq!(
        as_str(call("hex", &[Value::Int(i64::MIN)])),
        "-8000000000000000"
    );
    assert_eq!(as_str(call("binary", &[Value::Int(5)])), "101");
    assert_eq!(as_str(call("binary", &[Value::Int(-5)])), "-101");
}

#[test]
fn binary_with_width_masks_and_pads() {
    let b = |n: i64, w: i64| as_str(call("binary", &[Value::Int(n), Value::Int(w)]));
    assert_eq!(b(5, 8), "00000101");
    assert_eq!(b(-1, 8), "11111111");
    assert_eq!(b(300, 8), "00101100");
    // Width 64 used to shift by 64 (an overflow) and print all zeros.
    assert_eq!(b(-1, 64), "1".repeat(64));
    assert_eq!(b(1, 64), format!("{}1", "0".repeat(63)));
    assert_eq!(b(-1, 70), format!("{}{}", "0".repeat(6), "1".repeat(64)));
    assert!(call("binary", &[Value::Int(1), Value::Int(0)]).is_err());
    assert!(call("binary", &[Value::Int(1), Value::Int(1_000_000)]).is_err());
}

#[test]
fn floor_and_ceil_reject_values_outside_i64() {
    assert!(call("floor", &[Value::Float(9_223_372_036_854_775_808.0)]).is_err());
    assert!(call("ceil", &[Value::Float(1e300)]).is_err());
    assert_eq!(as_str(call("floor", &[Value::Float(-0.5)])), "-1");
    assert_eq!(as_str(call("ceil", &[Value::Float(-0.5)])), "0");
}

#[test]
fn unsupported_platform_builtins_fail_cleanly() {
    // On Unix these are real; elsewhere they must raise a catchable
    // ValueError rather than abort or fail to compile.
    if cfg!(not(unix)) {
        let argv = Value::List(std::rc::Rc::new(std::cell::RefCell::new(vec![Value::Str(
            "sh".to_string(),
        )])));
        assert!(matches!(
            call("pty_spawn", &[argv]),
            Err(RuntimeError::ValueError(_))
        ));
        assert!(matches!(call("fork", &[]), Err(RuntimeError::ValueError(_))));
    }
}

#[test]
fn functions_compare_equal_to_themselves() {
    let f = || Value::Closure {
        name: "f".to_string(),
        captured: std::rc::Rc::new(Env::new()),
    };
    let g = Value::Closure {
        name: "g".to_string(),
        captured: std::rc::Rc::new(Env::new()),
    };
    assert!(ops_arith::values_equal(&f(), &f()));
    assert!(!ops_arith::values_equal(&f(), &g));
    // Closures over different environments are different values.
    let mut env_a = Env::new();
    env_a.insert("x".to_string(), crate::value::new_cell(Value::Int(1)));
    let mut env_b = Env::new();
    env_b.insert("x".to_string(), crate::value::new_cell(Value::Int(1)));
    let ca = Value::Closure {
        name: "c".to_string(),
        captured: std::rc::Rc::new(env_a),
    };
    let cb = Value::Closure {
        name: "c".to_string(),
        captured: std::rc::Rc::new(env_b),
    };
    assert!(ops_arith::values_equal(&ca, &ca.clone()));
    assert!(!ops_arith::values_equal(&ca, &cb));
}

#[test]
fn dict_literal_keeps_source_order_and_last_duplicate_wins() {
    // {b: 1, a: 2, b: 3}
    let mut stack = vec![
        Value::Str("b".to_string()),
        Value::Int(1),
        Value::Str("a".to_string()),
        Value::Int(2),
        Value::Str("b".to_string()),
        Value::Int(3),
    ];
    ops_struct::handle_build_dict(3, &mut stack).unwrap();
    assert_eq!(stack.len(), 1);
    assert_eq!(stack[0].to_string(), "{b: 3, a: 2}");
    assert_eq!(
        as_str(call("dict_keys", &[stack[0].clone()])),
        "[b, a]"
    );
    // Asking for more pairs than the stack holds is an error, not a panic.
    let mut short = vec![Value::Int(1)];
    assert!(ops_struct::handle_build_dict(1, &mut short).is_err());
}

#[test]
fn length_counts_dict_entries() {
    let mut stack = vec![
        Value::Str("name".to_string()),
        Value::Str("Chris".to_string()),
        Value::Str("age".to_string()),
        Value::Int(32),
    ];
    ops_struct::handle_build_dict(2, &mut stack).unwrap();
    let dict = stack.pop().unwrap();
    assert_eq!(as_str(call("length", &[dict.clone()])), "2");
    let frozen = call("freeze", &[dict]).unwrap();
    assert_eq!(as_str(call("length", &[frozen])), "2");
}
