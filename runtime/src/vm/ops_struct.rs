use std::cell::RefCell;
use std::rc::Rc;

use crate::error::RuntimeError;
use crate::value::Value;

use super::pop;

/// Handle the `INDEX` instruction.
pub fn handle_index(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let idx = pop(stack)?;
    let base = pop(stack)?;
    match (base, idx) {
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
        (Value::Dict(map), Value::Str(k)) => {
            if let Some(v) = map.borrow().get(&k).cloned() {
                stack.push(v);
            } else {
                return Err(RuntimeError::KeyError(k));
            }
        }
        (Value::Dict(map), Value::Int(i)) => {
            let key = i.to_string();
            if let Some(v) = map.borrow().get(&key).cloned() {
                stack.push(v);
            } else {
                return Err(RuntimeError::KeyError(key));
            }
        }
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
        (other, _) => {
            return Err(RuntimeError::TypeError(format!(
                "{} is not indexable",
                other.to_string()
            )));
        }
    }
    Ok(())
}

/// Handle the `SLICE` instruction.
pub fn handle_slice(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let end_val = pop(stack)?;
    let start = pop(stack)?.as_int() as usize;
    let base = pop(stack)?;
    match base {
        Value::List(list) => {
            let list_ref = list.borrow();
            let end = match end_val {
                Value::None => list_ref.len(),
                v => v.as_int() as usize,
            };
            let slice = list_ref[start..end].to_vec();
            stack.push(Value::List(Rc::new(RefCell::new(slice))));
        }
        Value::Str(s) => {
            let chars: Vec<char> = s.chars().collect();
            let end = match end_val {
                Value::None => chars.len(),
                v => v.as_int() as usize,
            };
            let slice: String = chars[start..end].iter().collect();
            stack.push(Value::Str(slice));
        }
        _ => stack.push(Value::Int(0)),
    }
    Ok(())
}

/// Handle the `STOREINDEX` instruction.
pub fn handle_store_index(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
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

/// Handle the `ATTR` instruction.
pub fn handle_attr(stack: &mut Vec<Value>, attr: &str) -> Result<(), RuntimeError> {
    let base = pop(stack)?;
    match base {
        Value::Dict(map) => {
            if let Some(v) = map.borrow().get(attr).cloned() {
                stack.push(v);
            } else {
                return Err(RuntimeError::KeyError(attr.to_string()));
            }
        }
        Value::FrozenDict(map) => {
            if let Some(v) = map.get(attr).cloned() {
                stack.push(v);
            } else {
                return Err(RuntimeError::KeyError(attr.to_string()));
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

/// Handle the `STOREATTR` instruction.
pub fn handle_store_attr(stack: &mut Vec<Value>, attr: &str) -> Result<(), RuntimeError> {
    let val = pop(stack)?;
    let base = pop(stack)?;
    match base {
        Value::Dict(map) => {
            map.borrow_mut().insert(attr.to_string(), val);
        }
        Value::FrozenDict(_) => {
            return Err(RuntimeError::FrozenWriteError);
        }
        _ => {}
    }
    Ok(())
}
