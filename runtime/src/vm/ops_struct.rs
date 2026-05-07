//! # Structural Operations for the OMG VM
//!
//! Stack handlers for compound values: list/dict construction, indexing,
//! slicing, attribute and indexed assignment.
//!
//! ## Notable invariants
//! - List/dict literals never silently coerce or mutate; type mismatches
//!   produce `RuntimeError::TypeError` (no silent no-op fallthroughs).
//! - `store_index` requires the index to be in bounds — out-of-range writes
//!   are an `IndexError`, not auto-grow with zeros.
//! - Slicing only accepts list / string / frozen-list-shaped bases; other
//!   types raise `TypeError`.
//! - Frozen dictionaries are immutable: any write attempt yields
//!   `FrozenWriteError`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::pop;
use crate::error::RuntimeError;
use crate::value::Value;

/// Build a list from the top `n` stack values, preserving source order.
pub(super) fn handle_build_list(n: usize, stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let mut elements = Vec::new();
    for _ in 0..n {
        elements.push(pop(stack)?);
    }
    elements.reverse();
    stack.push(Value::List(Rc::new(RefCell::new(elements))));
    Ok(())
}

/// Build a dict from the top `n` (key, value) pairs. Keys are stringified
/// (matching Python interpreter behaviour).
pub(super) fn handle_build_dict(n: usize, stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let mut map: HashMap<String, Value> = HashMap::new();
    for _ in 0..n {
        let val = pop(stack)?;
        let key = pop(stack)?.to_string();
        map.insert(key, val);
    }
    stack.push(Value::Dict(Rc::new(RefCell::new(map))));
    Ok(())
}

pub(super) fn handle_index(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let idx = pop(stack)?;
    let base = pop(stack)?;
    match (base, idx) {
        (Value::List(list), Value::Int(i)) => {
            let l = list.borrow();
            let real = normalize_index(i, l.len())?;
            stack.push(l[real].clone());
        }
        (Value::Dict(map), key) => {
            let key_str = key.to_string();
            if let Some(v) = map.borrow().get(&key_str).cloned() {
                stack.push(v);
            } else {
                return Err(RuntimeError::KeyError(key_str));
            }
        }
        (Value::FrozenDict(map), key) => {
            let key_str = key.to_string();
            if let Some(v) = map.get(&key_str).cloned() {
                stack.push(v);
            } else {
                return Err(RuntimeError::KeyError(key_str));
            }
        }
        (Value::Str(s), Value::Int(i)) => {
            let chars: Vec<char> = s.chars().collect();
            let real = normalize_index(i, chars.len())?;
            stack.push(Value::Str(chars[real].to_string()));
        }
        (other, _) => {
            return Err(RuntimeError::TypeError(format!(
                "{} is not indexable",
                other.to_string()
            )));
        }
    }
    Ok(())
}

/// Slice a list or string. Negative indices count from the end (matching
/// Python and JavaScript). `None` for the end means "to the end".
pub(super) fn handle_slice(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let end_val = pop(stack)?;
    let start_val = pop(stack)?;
    let base = pop(stack)?;
    match base {
        Value::List(list) => {
            let list_ref = list.borrow();
            let len = list_ref.len();
            let (start, end) = resolve_slice_bounds(&start_val, &end_val, len)?;
            let slice = list_ref[start..end].to_vec();
            stack.push(Value::List(Rc::new(RefCell::new(slice))));
        }
        Value::Str(s) => {
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len();
            let (start, end) = resolve_slice_bounds(&start_val, &end_val, len)?;
            let slice: String = chars[start..end].iter().collect();
            stack.push(Value::Str(slice));
        }
        other => {
            return Err(RuntimeError::TypeError(format!(
                "{} is not sliceable",
                other.to_string()
            )));
        }
    }
    Ok(())
}

pub(super) fn handle_store_index(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let val = pop(stack)?;
    let idx = pop(stack)?;
    let base = pop(stack)?;
    match (base, idx) {
        (Value::List(list), Value::Int(i)) => {
            let mut l = list.borrow_mut();
            let real = normalize_index(i, l.len())?;
            l[real] = val;
        }
        (Value::Dict(map), key) => {
            let key_str = key.to_string();
            map.borrow_mut().insert(key_str, val);
        }
        (Value::FrozenDict(_), _) => {
            return Err(RuntimeError::FrozenWriteError);
        }
        (other, idx) => {
            return Err(RuntimeError::TypeError(format!(
                "cannot index-assign {} with {}",
                other.to_string(),
                idx.to_string()
            )));
        }
    }
    Ok(())
}

pub(super) fn handle_attr(attr: &String, stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let base = pop(stack)?;
    match base {
        Value::Dict(map) => {
            if let Some(v) = map.borrow().get(attr).cloned() {
                stack.push(v);
            } else {
                return Err(RuntimeError::KeyError(attr.clone()));
            }
        }
        Value::FrozenDict(map) => {
            if let Some(v) = map.get(attr).cloned() {
                stack.push(v);
            } else {
                return Err(RuntimeError::KeyError(attr.clone()));
            }
        }
        other => {
            return Err(RuntimeError::TypeError(format!(
                "{} has no attribute '{}'",
                other.to_string(),
                attr
            )));
        }
    }
    Ok(())
}

pub(super) fn handle_store_attr(
    attr: &String,
    stack: &mut Vec<Value>,
) -> Result<(), RuntimeError> {
    let val = pop(stack)?;
    let base = pop(stack)?;
    match base {
        Value::Dict(map) => {
            map.borrow_mut().insert(attr.clone(), val);
        }
        Value::FrozenDict(_) => {
            return Err(RuntimeError::FrozenWriteError);
        }
        other => {
            return Err(RuntimeError::TypeError(format!(
                "{} has no settable attribute '{}'",
                other.to_string(),
                attr
            )));
        }
    }
    Ok(())
}

// --- Helpers --------------------------------------------------------------

/// Convert a possibly-negative index into a real `usize`. Returns
/// `IndexError` if the result is out of bounds (matching the Python
/// interpreter's behaviour: writes and reads both fail rather than silently
/// expanding the container).
fn normalize_index(i: i64, len: usize) -> Result<usize, RuntimeError> {
    let real = if i < 0 { (len as i64) + i } else { i };
    if real < 0 || (real as usize) >= len {
        return Err(RuntimeError::IndexError(format!(
            "index {} out of range for length {}",
            i, len
        )));
    }
    Ok(real as usize)
}

/// Compute `(start, end)` for slicing. Negative indices count from the end;
/// `None` for `end_val` means "len". Out-of-range bounds clamp into
/// `[0, len]`, matching Python's slice semantics.
fn resolve_slice_bounds(
    start_val: &Value,
    end_val: &Value,
    len: usize,
) -> Result<(usize, usize), RuntimeError> {
    let start_i = match start_val {
        Value::None => 0,
        v => v.as_int()?,
    };
    let end_i = match end_val {
        Value::None => len as i64,
        v => v.as_int()?,
    };
    let resolve = |i: i64| -> i64 {
        if i < 0 {
            (len as i64) + i
        } else {
            i
        }
    };
    let mut start = resolve(start_i);
    let mut end = resolve(end_i);
    // Clamp.
    if start < 0 {
        start = 0;
    }
    if end < 0 {
        end = 0;
    }
    let mut start_u = start as usize;
    let mut end_u = end as usize;
    if start_u > len {
        start_u = len;
    }
    if end_u > len {
        end_u = len;
    }
    if start_u > end_u {
        // Match Python semantics: empty slice on inverted bounds.
        end_u = start_u;
    }
    Ok((start_u, end_u))
}
