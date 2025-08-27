//! # Structural Operations for the OMG VM
//!
//! This module implements all VM instructions that manipulate **compound
//! values**:
//! - **List construction** (`build_list`) and concatenation
//! - **Dictionary construction** (`build_dict`) and key/value access
//! - **Indexing** (`list[i]`, `dict[k]`, `str[i]`)
//! - **Slicing** (`list[start:end]`, `str[start:end]`)
//! - **Attribute access** (`dict.key` shorthand)
//! - **Mutable updates** (`list[i] = v`, `dict[k] = v`, `obj.key = v`)
//!
//! ## Execution model
//! - Each handler pops operands from the VM operand stack via [`super::pop`],
//!   validates them, performs the requested structural operation, and pushes
//!   the result back.
//! - Lists and dictionaries are heap-allocated and wrapped in `Rc<RefCell<_>>`
//!   for shared ownership + interior mutability.
//! - Immutable dictionaries are represented as `Value::FrozenDict`.
//!
//! ## Error behavior
//! - Out-of-bounds indexes and invalid slice ranges → `RuntimeError::IndexError`.
//! - Missing keys → `RuntimeError::KeyError`.
//! - Writes to frozen dicts → `RuntimeError::FrozenWriteError`.
//! - Wrong operand types → `RuntimeError::TypeError`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::pop;
use crate::error::RuntimeError;
use crate::value::Value;

/// Build a list from the top `n` stack values.
/// Pops `n` elements, reverses them to preserve order, and wraps in `Rc<RefCell<Vec<Value>>>`.
pub(super) fn handle_build_list(n: usize, stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let mut elements = Vec::new();
    for _ in 0..n {
        elements.push(pop(stack)?);
    }
    elements.reverse();
    stack.push(Value::List(Rc::new(RefCell::new(elements))));
    Ok(())
}

/// Build a dictionary from the top `n` key/value pairs on the stack.
/// Each pair is popped as (key, value); keys are converted to string.
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

/// Handle indexing into a list, dict, or string.
/// - `list[i]` → element at index
/// - `dict[k]` → value for key
/// - `string[i]` → single-character string
pub(super) fn handle_index(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let idx = pop(stack)?;
    let base = pop(stack)?;
    match (base, idx) {
        // List indexing
        (Value::List(list), Value::Int(i)) => {
            if i < 0 {
                return Err(RuntimeError::IndexError("List index out of bounds!".to_string()));
            }
            let l = list.borrow();
            let idx_usize = i as usize;
            if idx_usize < l.len() {
                stack.push(l[idx_usize].clone());
            } else {
                return Err(RuntimeError::IndexError("List index out of bounds!".to_string()));
            }
        }
        // Dict key lookup (string key)
        (Value::Dict(map), Value::Str(k)) => {
            if let Some(v) = map.borrow().get(&k).cloned() {
                stack.push(v);
            } else {
                return Err(RuntimeError::KeyError(k));
            }
        }
        // Dict key lookup (integer → stringified key)
        (Value::Dict(map), Value::Int(i)) => {
            let key = i.to_string();
            if let Some(v) = map.borrow().get(&key).cloned() {
                stack.push(v);
            } else {
                return Err(RuntimeError::KeyError(key));
            }
        }
        // Frozen dict behaves like immutable dict
        (Value::FrozenDict(map), Value::Str(k)) => {
            if let Some(v) = map.get(&k).cloned() {
                stack.push(v);
            } else {
                return Err(RuntimeError::KeyError(k));
            }
        }
        (Value::FrozenDict(map), Value::Int(i)) => {
            let key = i.to_string();
            if let Some(v) = map.get(&key).cloned() {
                stack.push(v);
            } else {
                return Err(RuntimeError::KeyError(key));
            }
        }
        // String indexing → return one-character string
        (Value::Str(s), Value::Int(i)) => {
            if i < 0 {
                return Err(RuntimeError::IndexError("String index out of bounds!".to_string()));
            }
            let chars: Vec<char> = s.chars().collect();
            let idx_usize = i as usize;
            if idx_usize < chars.len() {
                stack.push(Value::Str(chars[idx_usize].to_string()));
            } else {
                return Err(RuntimeError::IndexError("String index out of bounds!".to_string()));
            }
        }
        // Invalid base type
        (other, _) => {
            return Err(RuntimeError::TypeError(format!("{} is not indexable", other.to_string())));
        }
    }
    Ok(())
}

/// Handle slicing of lists and strings: `base[start:end]`.
/// Both `start` and `end` are required to be non-negative; `end` may be `None`.
pub(super) fn handle_slice(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let end_val = pop(stack)?;
    let start_val = pop(stack)?;
    let base = pop(stack)?;
    let start_i64 = start_val.as_int()?;
    match base {
        // List slicing
        Value::List(list) => {
            let list_ref = list.borrow();
            let len = list_ref.len();
            if start_i64 < 0 {
                return Err(RuntimeError::IndexError("Slice indices out of bounds!".to_string()));
            }
            let start = start_i64 as usize;
            let end_i64 = match end_val {
                Value::None => len as i64,
                v => v.as_int()?,
            };
            if end_i64 < 0 {
                return Err(RuntimeError::IndexError("Slice indices out of bounds!".to_string()));
            }
            let end = end_i64 as usize;
            if start > end || end > len {
                return Err(RuntimeError::IndexError("Slice indices out of bounds!".to_string()));
            }
            let slice = list_ref[start..end].to_vec();
            stack.push(Value::List(Rc::new(RefCell::new(slice))));
        }
        // String slicing
        Value::Str(s) => {
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len();
            if start_i64 < 0 {
                return Err(RuntimeError::IndexError("Slice indices out of bounds!".to_string()));
            }
            let start = start_i64 as usize;
            let end_i64 = match end_val {
                Value::None => len as i64,
                v => v.as_int()?,
            };
            if end_i64 < 0 {
                return Err(RuntimeError::IndexError("Slice indices out of bounds!".to_string()));
            }
            let end = end_i64 as usize;
            if start > end || end > len {
                return Err(RuntimeError::IndexError("Slice indices out of bounds!".to_string()));
            }
            let slice: String = chars[start..end].iter().collect();
            stack.push(Value::Str(slice));
        }
        // Invalid base → push dummy 0 (VM design choice)
        _ => stack.push(Value::Int(0)),
    }
    Ok(())
}

/// Handle indexed assignment: `base[idx] = val`.
/// - Lists grow automatically if index >= len.
/// - Dict keys accept string or integer (stringified).
/// - Frozen dicts error on write.
pub(super) fn handle_store_index(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let val = pop(stack)?;
    let idx = pop(stack)?;
    let base = pop(stack)?;
    match (base, idx) {
        (Value::List(list), Value::Int(i)) => {
            let mut l = list.borrow_mut();
            let idx_usize = i as usize;
            if idx_usize >= l.len() {
                l.resize(idx_usize + 1, Value::Int(0));
            }
            l[idx_usize] = val;
        }
        (Value::Dict(map), Value::Str(k)) => {
            map.borrow_mut().insert(k, val);
        }
        (Value::Dict(map), Value::Int(i)) => {
            map.borrow_mut().insert(i.to_string(), val);
        }
        (Value::FrozenDict(_), _) => {
            return Err(RuntimeError::FrozenWriteError);
        }
        _ => {}
    }
    Ok(())
}

/// Handle attribute access: `base.attr`.
/// Only dictionaries (mutable or frozen) support attributes.
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

/// Handle attribute assignment: `base.attr = val`.
/// - Only mutable dicts support writes.
/// - Frozen dicts error on write.
pub(super) fn handle_store_attr(attr: &String, stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let val = pop(stack)?;
    let base = pop(stack)?;
    match base {
        Value::Dict(map) => {
            map.borrow_mut().insert(attr.clone(), val);
        }
        Value::FrozenDict(_) => {
            return Err(RuntimeError::FrozenWriteError);
        }
        _ => {}
    }
    Ok(())
}
