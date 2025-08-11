use crate::error::RuntimeError;
use crate::value::Value;

use super::pop;

/// Handle the `ADD` instruction.
pub fn handle_add(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    match (a, b) {
        (Value::Str(sa), Value::Str(sb)) => stack.push(Value::Str(sa + &sb)),
        (Value::Str(sa), v) => stack.push(Value::Str(sa + &v.to_string())),
        (v, Value::Str(sb)) => stack.push(Value::Str(v.to_string() + &sb)),
        (Value::List(la), Value::List(lb)) => {
            {
                let mut la_mut = la.borrow_mut();
                la_mut.extend(lb.borrow().iter().cloned());
            }
            stack.push(Value::List(la));
        }
        (a, b) => stack.push(Value::Int(a.as_int() + b.as_int())),
    }
    Ok(())
}

/// Handle the `SUB` instruction.
pub fn handle_sub(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int();
    let a = pop(stack)?.as_int();
    stack.push(Value::Int(a - b));
    Ok(())
}

/// Handle the `MUL` instruction.
pub fn handle_mul(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int();
    let a = pop(stack)?.as_int();
    stack.push(Value::Int(a.checked_mul(b).unwrap_or(0)));
    Ok(())
}

/// Handle the `DIV` instruction.
pub fn handle_div(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int();
    if b == 0 {
        return Err(RuntimeError::ZeroDivisionError);
    }
    let a = pop(stack)?.as_int();
    stack.push(Value::Int(a / b));
    Ok(())
}

/// Handle the `MOD` instruction.
pub fn handle_mod(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int();
    if b == 0 {
        return Err(RuntimeError::ZeroDivisionError);
    }
    let a = pop(stack)?.as_int();
    stack.push(Value::Int(a % b));
    Ok(())
}

/// Handle the `EQ` instruction.
pub fn handle_eq(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.to_string();
    let a = pop(stack)?.to_string();
    stack.push(Value::Bool(a == b));
    Ok(())
}

/// Handle the `NE` instruction.
pub fn handle_ne(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.to_string();
    let a = pop(stack)?.to_string();
    stack.push(Value::Bool(a != b));
    Ok(())
}

/// Handle the `LT` instruction.
pub fn handle_lt(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    let res = match (&a, &b) {
        (Value::Str(sa), Value::Str(sb)) => sa < sb,
        _ => a.as_int() < b.as_int(),
    };
    stack.push(Value::Bool(res));
    Ok(())
}

/// Handle the `LE` instruction.
pub fn handle_le(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    let res = match (&a, &b) {
        (Value::Str(sa), Value::Str(sb)) => sa <= sb,
        _ => a.as_int() <= b.as_int(),
    };
    stack.push(Value::Bool(res));
    Ok(())
}

/// Handle the `GT` instruction.
pub fn handle_gt(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    let res = match (&a, &b) {
        (Value::Str(sa), Value::Str(sb)) => sa > sb,
        _ => a.as_int() > b.as_int(),
    };
    stack.push(Value::Bool(res));
    Ok(())
}

/// Handle the `GE` instruction.
pub fn handle_ge(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    let res = match (&a, &b) {
        (Value::Str(sa), Value::Str(sb)) => sa >= sb,
        _ => a.as_int() >= b.as_int(),
    };
    stack.push(Value::Bool(res));
    Ok(())
}

/// Handle the `BAND` instruction.
pub fn handle_band(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int();
    let a = pop(stack)?.as_int();
    stack.push(Value::Int(a & b));
    Ok(())
}

/// Handle the `BOR` instruction.
pub fn handle_bor(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int();
    let a = pop(stack)?.as_int();
    stack.push(Value::Int(a | b));
    Ok(())
}

/// Handle the `BXOR` instruction.
pub fn handle_bxor(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int();
    let a = pop(stack)?.as_int();
    stack.push(Value::Int(a ^ b));
    Ok(())
}

/// Handle the `SHL` instruction.
pub fn handle_shl(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int() as u32;
    let a = pop(stack)?.as_int();
    stack.push(Value::Int(a << b));
    Ok(())
}

/// Handle the `SHR` instruction.
pub fn handle_shr(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int() as u32;
    let a = pop(stack)?.as_int();
    stack.push(Value::Int(a >> b));
    Ok(())
}

/// Handle the `AND` instruction.
pub fn handle_and(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_bool();
    let a = pop(stack)?.as_bool();
    stack.push(Value::Bool(a && b));
    Ok(())
}

/// Handle the `OR` instruction.
pub fn handle_or(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_bool();
    let a = pop(stack)?.as_bool();
    stack.push(Value::Bool(a || b));
    Ok(())
}

/// Handle the `NOT` instruction.
pub fn handle_not(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let v = pop(stack)?.as_int();
    stack.push(Value::Int(!v));
    Ok(())
}

/// Handle the `NEG` instruction.
pub fn handle_neg(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let v = pop(stack)?.as_int();
    stack.push(Value::Int(-v));
    Ok(())
}
