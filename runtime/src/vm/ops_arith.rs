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
//! - `/` is **true division**: always returns a float, matching Python 3
//!   (`7 / 2 = 3.5`, `7 / -2 = -3.5`).
//! - `//` is **floor division**: rounds the quotient toward minus infinity,
//!   keeping the integer type when both operands are integers
//!   (`-7 // 2 = -4`, `7 // -2 = -4`).
//! - `%` is **floor modulo**: the result carries the sign of the divisor
//!   (`-7 % 2 = 1`, `7 % -2 = -1`).

use super::pop;
use crate::error::RuntimeError;
use crate::value::Value;

/// Floor division on i64: rounds the quotient toward minus infinity.
/// Matches Python's `//`. `i64::div_floor` is still unstable on stable
/// Rust (rust-lang/rust#88581), so we hand-roll it. Caller must already
/// have rejected `b == 0` and the `i64::MIN / -1` overflow case.
fn idiv_floor(a: i64, b: i64) -> i64 {
    let q = a / b;
    let r = a % b;
    if r != 0 && ((r < 0) != (b < 0)) { q - 1 } else { q }
}

/// Floor modulo on i64: result carries the sign of the divisor.
/// Matches Python's `%`. Same overflow / zero-divisor preconditions as
/// [`idiv_floor`].
fn imod_floor(a: i64, b: i64) -> i64 {
    let r = a % b;
    if r != 0 && ((r < 0) != (b < 0)) { r + b } else { r }
}

/// Either operand is a float? Used to dispatch arithmetic between the
/// pure-int and promoted-to-float code paths.
fn is_float(v: &Value) -> bool {
    matches!(v, Value::Float(_))
}

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
        (a, b) if is_float(&a) || is_float(&b) => {
            stack.push(Value::Float(a.as_float()? + b.as_float()?));
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
    let b = pop(stack)?;
    let a = pop(stack)?;
    if is_float(&a) || is_float(&b) {
        stack.push(Value::Float(a.as_float()? - b.as_float()?));
        return Ok(());
    }
    let ai = a.as_int()?;
    let bi = b.as_int()?;
    let result = ai
        .checked_sub(bi)
        .ok_or_else(|| RuntimeError::ValueError("integer overflow on subtraction".to_string()))?;
    stack.push(Value::Int(result));
    Ok(())
}

pub(super) fn handle_mul(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    if is_float(&a) || is_float(&b) {
        stack.push(Value::Float(a.as_float()? * b.as_float()?));
        return Ok(());
    }
    let ai = a.as_int()?;
    let bi = b.as_int()?;
    let result = ai
        .checked_mul(bi)
        .ok_or_else(|| RuntimeError::ValueError("integer overflow on multiplication".to_string()))?;
    stack.push(Value::Int(result));
    Ok(())
}

/// True division (`/`). Always returns a float, matching Python 3.
/// Both operands are coerced to f64; integer operands are widened. Use
/// `//` when you want integer (floor) division.
pub(super) fn handle_div(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    let bf = b.as_float()?;
    if bf == 0.0 {
        return Err(RuntimeError::ZeroDivisionError);
    }
    stack.push(Value::Float(a.as_float()? / bf));
    Ok(())
}

/// Explicit floor division (`//`). Always rounds toward minus infinity.
/// Returns int when both operands are int; returns float (still floored)
/// when either operand is float, matching Python's `//`.
pub(super) fn handle_floor_div(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    if is_float(&a) || is_float(&b) {
        let bf = b.as_float()?;
        if bf == 0.0 {
            return Err(RuntimeError::ZeroDivisionError);
        }
        stack.push(Value::Float((a.as_float()? / bf).floor()));
        return Ok(());
    }
    let bi = b.as_int()?;
    if bi == 0 {
        return Err(RuntimeError::ZeroDivisionError);
    }
    let ai = a.as_int()?;
    if ai == i64::MIN && bi == -1 {
        return Err(RuntimeError::ValueError(
            "integer overflow on division".to_string(),
        ));
    }
    stack.push(Value::Int(idiv_floor(ai, bi)));
    Ok(())
}

/// Floor modulus (Python `%`). Result carries the sign of the divisor:
/// `-7 % 2 == 1`, `7 % -2 == -1`. Promotes on float.
pub(super) fn handle_mod(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    if is_float(&a) || is_float(&b) {
        let bf = b.as_float()?;
        if bf == 0.0 {
            return Err(RuntimeError::ZeroDivisionError);
        }
        // Python-style float modulo: result has the sign of the divisor.
        let af = a.as_float()?;
        let r = af - (af / bf).floor() * bf;
        stack.push(Value::Float(r));
        return Ok(());
    }
    let bi = b.as_int()?;
    if bi == 0 {
        return Err(RuntimeError::ZeroDivisionError);
    }
    let ai = a.as_int()?;
    if ai == i64::MIN && bi == -1 {
        return Err(RuntimeError::ValueError(
            "integer overflow on modulo".to_string(),
        ));
    }
    stack.push(Value::Int(imod_floor(ai, bi)));
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
        (Value::Float(x), Value::Float(y)) => x == y,
        // Numeric cross-type: compare via f64. Inside i64 range this is exact;
        // outside it is consistent with `as f64` rounding (the float side
        // already lost precision when constructed).
        (Value::Int(x), Value::Float(y)) | (Value::Float(y), Value::Int(x)) => {
            (*x as f64) == *y
        }
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
        (a, b) if is_float(a) || is_float(b) => a.as_float()? < b.as_float()?,
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
        (a, b) if is_float(a) || is_float(b) => a.as_float()? <= b.as_float()?,
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
        (a, b) if is_float(a) || is_float(b) => a.as_float()? > b.as_float()?,
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
        (a, b) if is_float(a) || is_float(b) => a.as_float()? >= b.as_float()?,
        _ => a.as_int()? >= b.as_int()?,
    };
    stack.push(Value::Bool(res));
    Ok(())
}

/// Pop two operands and assert neither is a float. Bitwise operators are
/// integer-only; silent truncation of a float to int would be a footgun.
fn pop_two_ints(stack: &mut Vec<Value>, op: &str) -> Result<(i64, i64), RuntimeError> {
    let b = pop(stack)?;
    let a = pop(stack)?;
    if is_float(&a) || is_float(&b) {
        return Err(RuntimeError::TypeError(format!(
            "operator '{}' is not defined for floats",
            op
        )));
    }
    Ok((a.as_int()?, b.as_int()?))
}

pub(super) fn handle_band(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let (a, b) = pop_two_ints(stack, "&")?;
    stack.push(Value::Int(a & b));
    Ok(())
}

pub(super) fn handle_bor(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let (a, b) = pop_two_ints(stack, "|")?;
    stack.push(Value::Int(a | b));
    Ok(())
}

pub(super) fn handle_bxor(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let (a, b) = pop_two_ints(stack, "^")?;
    stack.push(Value::Int(a ^ b));
    Ok(())
}

pub(super) fn handle_shl(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let (a, b) = pop_two_ints(stack, "<<")?;
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
    let (a, b) = pop_two_ints(stack, ">>")?;
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
    let v = pop(stack)?;
    if is_float(&v) {
        return Err(RuntimeError::TypeError(
            "operator '~' is not defined for floats".to_string(),
        ));
    }
    stack.push(Value::Int(!v.as_int()?));
    Ok(())
}

pub(super) fn handle_neg(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let v = pop(stack)?;
    if let Value::Float(f) = v {
        stack.push(Value::Float(-f));
        return Ok(());
    }
    let i = v.as_int()?;
    let result = i
        .checked_neg()
        .ok_or_else(|| RuntimeError::ValueError("integer overflow on negation".to_string()))?;
    stack.push(Value::Int(result));
    Ok(())
}
