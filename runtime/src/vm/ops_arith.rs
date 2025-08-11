use super::pop;
use crate::error::RuntimeError;
use crate::value::Value;

pub(super) fn handle_add(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
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
        (a, b) => {
            let ai = a.as_int()?;
            let bi = b.as_int()?;
            stack.push(Value::Int(ai + bi));
        }
    }
    Ok(())
}

pub(super) fn handle_sub(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a - b));
    Ok(())
}

pub(super) fn handle_mul(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a.checked_mul(b).unwrap_or(0)));
    Ok(())
}

pub(super) fn handle_div(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    if b == 0 {
        return Err(RuntimeError::ZeroDivisionError);
    }
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a / b));
    Ok(())
}

pub(super) fn handle_mod(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    if b == 0 {
        return Err(RuntimeError::ZeroDivisionError);
    }
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a % b));
    Ok(())
}

pub(super) fn handle_eq(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.to_string();
    let a = pop(stack)?.to_string();
    stack.push(Value::Bool(a == b));
    Ok(())
}

pub(super) fn handle_ne(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.to_string();
    let a = pop(stack)?.to_string();
    stack.push(Value::Bool(a != b));
    Ok(())
}

pub(super) fn handle_lt(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    let res = match (&a, &b) {
        (Value::Str(sa), Value::Str(sb)) => sa < sb,
        _ => {
            let ai = a.as_int()?;
            let bi = b.as_int()?;
            ai < bi
        }
    };
    stack.push(Value::Bool(res));
    Ok(())
}

pub(super) fn handle_le(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    let res = match (&a, &b) {
        (Value::Str(sa), Value::Str(sb)) => sa <= sb,
        _ => {
            let ai = a.as_int()?;
            let bi = b.as_int()?;
            ai <= bi
        }
    };
    stack.push(Value::Bool(res));
    Ok(())
}

pub(super) fn handle_gt(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    let res = match (&a, &b) {
        (Value::Str(sa), Value::Str(sb)) => sa > sb,
        _ => {
            let ai = a.as_int()?;
            let bi = b.as_int()?;
            ai > bi
        }
    };
    stack.push(Value::Bool(res));
    Ok(())
}

pub(super) fn handle_ge(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    let res = match (&a, &b) {
        (Value::Str(sa), Value::Str(sb)) => sa >= sb,
        _ => {
            let ai = a.as_int()?;
            let bi = b.as_int()?;
            ai >= bi
        }
    };
    stack.push(Value::Bool(res));
    Ok(())
}

pub(super) fn handle_band(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a & b));
    Ok(())
}

pub(super) fn handle_bor(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a | b));
    Ok(())
}

pub(super) fn handle_bxor(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a ^ b));
    Ok(())
}

pub(super) fn handle_shl(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()? as u32;
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a << b));
    Ok(())
}

pub(super) fn handle_shr(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()? as u32;
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a >> b));
    Ok(())
}

pub(super) fn handle_and(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_bool();
    let a = pop(stack)?.as_bool();
    stack.push(Value::Bool(a && b));
    Ok(())
}

pub(super) fn handle_or(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_bool();
    let a = pop(stack)?.as_bool();
    stack.push(Value::Bool(a || b));
    Ok(())
}

pub(super) fn handle_not(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let v = pop(stack)?.as_int()?;
    stack.push(Value::Int(!v));
    Ok(())
}

pub(super) fn handle_neg(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let v = pop(stack)?.as_int()?;
    stack.push(Value::Int(-v));
    Ok(())
}
