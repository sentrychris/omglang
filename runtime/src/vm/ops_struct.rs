use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::error::RuntimeError;
use crate::value::Value;
use super::pop;

pub(super) fn handle_build_list(n: usize, stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let mut elements = Vec::new();
    for _ in 0..n {
        elements.push(pop(stack)?);
    }
    elements.reverse();
    stack.push(Value::List(Rc::new(RefCell::new(elements))));
    Ok(())
}

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
            return Err(RuntimeError::TypeError(format!("{} is not indexable", other.to_string())));
        }
    }
    Ok(())
}

pub(super) fn handle_slice(stack: &mut Vec<Value>) -> Result<(), RuntimeError> {
    let end_val = pop(stack)?;
    let start_val = pop(stack)?;
    let base = pop(stack)?;
    let start_i64 = start_val.as_int();
    match base {
        Value::List(list) => {
            let list_ref = list.borrow();
            let len = list_ref.len();
            if start_i64 < 0 {
                return Err(RuntimeError::IndexError("Slice indices out of bounds!".to_string()));
            }
            let start = start_i64 as usize;
            let end_i64 = match end_val {
                Value::None => len as i64,
                v => v.as_int(),
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
        Value::Str(s) => {
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len();
            if start_i64 < 0 {
                return Err(RuntimeError::IndexError("Slice indices out of bounds!".to_string()));
            }
            let start = start_i64 as usize;
            let end_i64 = match end_val {
                Value::None => len as i64,
                v => v.as_int(),
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
        _ => stack.push(Value::Int(0)),
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
