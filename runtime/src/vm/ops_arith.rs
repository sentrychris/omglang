//! # Arithmetic, Bitwise, and Boolean Operations for the OMG VM
//!
//! Stack-machine semantics for `+ - * / % == != < <= > >= & | ^ << >> and or
//! ~ neg`. Each handler pops operands (right then left, so the source order
//! is preserved), performs the op, and pushes the result.
//!
//! ## Coercion rules
//! - `+` supports:
//!   - `Str + Str` → concatenation
//!   - `Str + any` / `any + Str` → stringify the non-string side and concat
//!   - `List + List` → **new** list (does not mutate either operand)
//!   - otherwise → integer addition via `as_int()` with overflow check
//! - `-`, `*`, `/`, `%`, bitwise ops, shifts, and unary ops operate on integers
//!   via `as_int()`. Arithmetic is checked: overflow surfaces as
//!   `RuntimeError::ValueError`, not silent wrap.
//! - Boolean `and`/`or` use `as_bool()`.
//! - Equality (`==`/`!=`) uses **typed** value identity (so `5 == "5"` is
//!   false). Strings, ints, bools, lists, dicts compare structurally.
//! - Ordered comparisons accept `Str` vs `Str` lexicographically; otherwise
//!   coerce to int.
//! - Division and modulo use Python-style **floor** semantics
//!   (`(-7) / 2 = -4`, `(-7) % 2 = 1`).

use super::pop;
use crate::error::RuntimeError;
use crate::value::Value;

/// Handle addition. List + List returns a freshly allocated list.
pub(super) fn handle_add(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    match (a, b) {
        (Value::Str(sa), Value::Str(sb)) => stack.push(Value::Str(sa + &sb)),
        (Value::Str(sa), v) => stack.push(Value::Str(sa + &v.to_string())),
        (v, Value::Str(sb)) => stack.push(Value::Str(v.to_string() + &sb)),
        (Value::List(la), Value::List(lb)) => {
            // Allocate a fresh list — never mutate either operand.
            let mut new_vec: Vec<Value> = la.borrow().clone();
            new_vec.extend(lb.borrow().iter().cloned());
            stack.push(Value::List(std::rc::Rc::new(std::cell::RefCell::new(
                new_vec,
            ))));
        }
        (a, b) => {
            let ai = a.as_int()?;
            let bi = b.as_int()?;
            let result = ai.checked_add(bi).ok_or_else(|| {
                RuntimeError::ValueError("integer overflow on addition".to_string())
            })?;
            stack.push(Value::Int(result));
        }
    }
    Ok(())
}

pub(super) fn handle_sub(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    let result = a
        .checked_sub(b)
        .ok_or_else(|| RuntimeError::ValueError("integer overflow on subtraction".to_string()))?;
    stack.push(Value::Int(result));
    Ok(())
}

pub(super) fn handle_mul(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    let result = a
        .checked_mul(b)
        .ok_or_else(|| RuntimeError::ValueError("integer overflow on multiplication".to_string()))?;
    stack.push(Value::Int(result));
    Ok(())
}

/// Floor division (Python `//`). `-7 / 2 == -4`.
pub(super) fn handle_div(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    if b == 0 {
        return Err(RuntimeError::ZeroDivisionError);
    }
    let a = pop(stack)?.as_int()?;
    // i64::MIN / -1 overflows.
    if a == i64::MIN && b == -1 {
        return Err(RuntimeError::ValueError(
            "integer overflow on division".to_string(),
        ));
    }
    stack.push(Value::Int(a.div_euclid(b)));
    Ok(())
}

/// Floor modulus (Python `%`). `-7 % 2 == 1`.
pub(super) fn handle_mod(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    if b == 0 {
        return Err(RuntimeError::ZeroDivisionError);
    }
    let a = pop(stack)?.as_int()?;
    if a == i64::MIN && b == -1 {
        return Err(RuntimeError::ValueError(
            "integer overflow on modulo".to_string(),
        ));
    }
    stack.push(Value::Int(a.rem_euclid(b)));
    Ok(())
}

/// Typed structural equality.
pub(super) fn handle_eq(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    stack.push(Value::Bool(values_equal(&a, &b)));
    Ok(())
}

pub(super) fn handle_ne(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    stack.push(Value::Bool(!values_equal(&a, &b)));
    Ok(())
}

/// Structural equality without coercion. Two values of incompatible types
/// are never equal.
pub(crate) fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::None, Value::None) => true,
        (Value::List(la), Value::List(lb)) => {
            let la = la.borrow();
            let lb = lb.borrow();
            la.len() == lb.len() && la.iter().zip(lb.iter()).all(|(x, y)| values_equal(x, y))
        }
        (Value::Dict(da), Value::Dict(db)) => {
            let da = da.borrow();
            let db = db.borrow();
            if da.len() != db.len() {
                return false;
            }
            da.iter()
                .all(|(k, v)| db.get(k).map_or(false, |w| values_equal(v, w)))
        }
        (Value::FrozenDict(da), Value::FrozenDict(db)) => {
            if da.len() != db.len() {
                return false;
            }
            da.iter()
                .all(|(k, v)| db.get(k).map_or(false, |w| values_equal(v, w)))
        }
        // Mixed mutable/frozen dicts compare by contents.
        (Value::Dict(da), Value::FrozenDict(db)) | (Value::FrozenDict(db), Value::Dict(da)) => {
            let da = da.borrow();
            if da.len() != db.len() {
                return false;
            }
            da.iter()
                .all(|(k, v)| db.get(k).map_or(false, |w| values_equal(v, w)))
        }
        _ => false,
    }
}

pub(super) fn handle_lt(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    let res = match (&a, &b) {
        (Value::Str(sa), Value::Str(sb)) => sa < sb,
        _ => a.as_int()? < b.as_int()?,
    };
    stack.push(Value::Bool(res));
    Ok(())
}

pub(super) fn handle_le(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    let res = match (&a, &b) {
        (Value::Str(sa), Value::Str(sb)) => sa <= sb,
        _ => a.as_int()? <= b.as_int()?,
    };
    stack.push(Value::Bool(res));
    Ok(())
}

pub(super) fn handle_gt(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    let res = match (&a, &b) {
        (Value::Str(sa), Value::Str(sb)) => sa > sb,
        _ => a.as_int()? > b.as_int()?,
    };
    stack.push(Value::Bool(res));
    Ok(())
}

pub(super) fn handle_ge(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    let res = match (&a, &b) {
        (Value::Str(sa), Value::Str(sb)) => sa >= sb,
        _ => a.as_int()? >= b.as_int()?,
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
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    if !(0..64).contains(&b) {
        return Err(RuntimeError::ValueError(format!(
            "shift count out of range: {}",
            b
        )));
    }
    let result = a
        .checked_shl(b as u32)
        .ok_or_else(|| RuntimeError::ValueError("integer overflow on shift".to_string()))?;
    stack.push(Value::Int(result));
    Ok(())
}

pub(super) fn handle_shr(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    if !(0..64).contains(&b) {
        return Err(RuntimeError::ValueError(format!(
            "shift count out of range: {}",
            b
        )));
    }
    stack.push(Value::Int(a >> (b as u32)));
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
    let result = v
        .checked_neg()
        .ok_or_else(|| RuntimeError::ValueError("integer overflow on negation".to_string()))?;
    stack.push(Value::Int(result));
    Ok(())
}
