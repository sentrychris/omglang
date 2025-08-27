//! # Arithmetic, Bitwise, and Boolean Operations for the OMG VM
//!
//! This module implements the stack machine semantics for all arithmetic,
//! comparison, bitwise, and boolean operators used by the runtime VM.
//!
//! ## Execution model
//! Each handler:
//! - **Pops operands** from the VM operand stack using [`super::pop`]
//!   (right operand first, then left), which guarantees underflow-safe pops.
//! - **Performs** the operation with minimal coercions (see below).
//! - **Pushes** a single [`Value`] result back onto the stack.
//! - **Returns** `Result<(), RuntimeError>` so the caller can propagate faults.
//!
//! ## Type & coercion rules
//! - `+` supports:
//!   - `Str + Str` → concatenation
//!   - `Str + any` or `any + Str` → stringify the non-string side and concat
//!   - `List + List` → in‑place extend left list (preserving its Rc identity)
//!   - otherwise → integer addition via `as_int()`
//! - `-`, `*`, `/`, `%`, bitwise ops, shifts, and unary ops operate on **integers**
//!   via `as_int()`.
//! - Boolean `and`/`or` use `as_bool()` (see [`Value`] for truthiness rules).
//! - Comparisons allow **string vs string** lexicographic comparison; otherwise
//!   they fall back to integer comparison via `as_int()`.
//! - Equality/inequality (`==`, `!=`) compare **stringified** values so that
//!   heterogenous types can be compared consistently at the VM layer.
//!
//! ## Error behavior
//! - Division/modulo by zero → `RuntimeError::ZeroDivisionError`.
//! - Type mismatches bubble up from `Value::as_int()` / `as_bool()`.
//! - `handle_mul` uses `checked_mul`; on overflow it **returns 0** (by design).
//!
//! ## Notes
//! - `handle_not` implements **bitwise NOT** (`~`), *not* logical negation.
//! - Operand order is **left then right** (pop `b`, then `a`) to match infix `a op b`.

use super::pop;
use crate::error::RuntimeError;
use crate::value::Value;

/// Handle addition operation.
///
/// Supports:
/// - Integer addition
/// - String concatenation
/// - List concatenation
/// - Mixed string + other (converted via `to_string`)
pub(super) fn handle_add(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?; // right operand
    let a = pop(stack)?; // left operand
    match (a, b) {
        // String + String → concatenate
        (Value::Str(sa), Value::Str(sb)) => stack.push(Value::Str(sa + &sb)),
        // String + any value → stringify right side
        (Value::Str(sa), v) => stack.push(Value::Str(sa + &v.to_string())),
        // Any value + String → stringify left side
        (v, Value::Str(sb)) => stack.push(Value::Str(v.to_string() + &sb)),
        // List + List → extend left list in place
        (Value::List(la), Value::List(lb)) => {
            {
                let mut la_mut = la.borrow_mut();
                la_mut.extend(lb.borrow().iter().cloned());
            }
            stack.push(Value::List(la));
        }
        // Otherwise: integer addition
        (a, b) => {
            let ai = a.as_int()?;
            let bi = b.as_int()?;
            stack.push(Value::Int(ai + bi));
        }
    }
    Ok(())
}

/// Handle subtraction of two integers.
pub(super) fn handle_sub(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a - b));
    Ok(())
}

/// Handle multiplication of two integers.
pub(super) fn handle_mul(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    // Use checked_mul to prevent overflow panics; fallback to 0 on overflow.
    stack.push(Value::Int(a.checked_mul(b).unwrap_or(0)));
    Ok(())
}

/// Handle integer division. Errors on division by zero.
pub(super) fn handle_div(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    if b == 0 {
        return Err(RuntimeError::ZeroDivisionError);
    }
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a / b));
    Ok(())
}

/// Handle integer modulus. Errors on division by zero.
pub(super) fn handle_mod(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    if b == 0 {
        return Err(RuntimeError::ZeroDivisionError);
    }
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a % b));
    Ok(())
}

/// Handle equality check. Compares stringified values.
pub(super) fn handle_eq(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.to_string();
    let a = pop(stack)?.to_string();
    stack.push(Value::Bool(a == b));
    Ok(())
}

/// Handle inequality check. Compares stringified values.
pub(super) fn handle_ne(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.to_string();
    let a = pop(stack)?.to_string();
    stack.push(Value::Bool(a != b));
    Ok(())
}

/// Handle less-than comparison. Supports integers and strings.
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

/// Handle <= comparison. Supports integers and strings.
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

/// Handle greater-than comparison. Supports integers and strings.
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

/// Handle >= comparison. Supports integers and strings.
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

/// Handle bitwise AND (&) between integers.
pub(super) fn handle_band(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a & b));
    Ok(())
}

/// Handle bitwise OR (|) between integers.
pub(super) fn handle_bor(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a | b));
    Ok(())
}

/// Handle bitwise XOR (^) between integers.
pub(super) fn handle_bxor(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()?;
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a ^ b));
    Ok(())
}

/// Handle left shift (<<) between integers.
pub(super) fn handle_shl(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()? as u32;
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a << b));
    Ok(())
}

/// Handle right shift (>>) between integers.
pub(super) fn handle_shr(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_int()? as u32;
    let a = pop(stack)?.as_int()?;
    stack.push(Value::Int(a >> b));
    Ok(())
}

/// Handle logical AND between booleans.
pub(super) fn handle_and(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_bool();
    let a = pop(stack)?.as_bool();
    stack.push(Value::Bool(a && b));
    Ok(())
}

/// Handle logical OR between booleans.
pub(super) fn handle_or(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let b = pop(stack)?.as_bool();
    let a = pop(stack)?.as_bool();
    stack.push(Value::Bool(a || b));
    Ok(())
}

/// Handle bitwise NOT (~) of an integer.
pub(super) fn handle_not(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let v = pop(stack)?.as_int()?;
    stack.push(Value::Int(!v));
    Ok(())
}

/// Handle unary negation (-) of an integer.
pub(super) fn handle_neg(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let v = pop(stack)?.as_int()?;
    stack.push(Value::Int(-v));
    Ok(())
}
