use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Value type for the VM stack.
#[derive(Clone)]
pub enum Value {
    Int(i64),
    Str(String),
    Bool(bool),
    List(Rc<RefCell<Vec<Value>>>),
    Dict(Rc<RefCell<HashMap<String, Value>>>),
    None,
}

impl Value {
    /// Convert the value to an integer.
    pub fn as_int(&self) -> i64 {
        match self {
            Value::Int(i) => *i,
            Value::Str(s) => s.parse::<i64>().unwrap_or(0),
            Value::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            Value::List(l) => l.borrow().len() as i64,
            Value::Dict(d) => d.borrow().len() as i64,
            Value::None => 0,
        }
    }

    /// Convert the value to a boolean.
    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.borrow().is_empty(),
            Value::Dict(d) => !d.borrow().is_empty(),
            Value::None => false,
        }
    }

    /// Convert the value to a string representation.
    pub fn to_string(&self) -> String {
        fn helper(val: &Value, seen: &mut HashSet<usize>) -> String {
            match val {
                Value::Int(i) => i.to_string(),
                Value::Str(s) => s.clone(),
                Value::Bool(b) => b.to_string(),
                Value::List(list) => {
                    let ptr = Rc::as_ptr(list) as usize;
                    if !seen.insert(ptr) {
                        return "[...]".to_string();
                    }
                    let inner: Vec<String> =
                        list.borrow().iter().map(|v| helper(v, seen)).collect();
                    format!("[{}]", inner.join(", "))
                }
                Value::Dict(map) => {
                    let ptr = Rc::as_ptr(map) as usize;
                    if !seen.insert(ptr) {
                        return "{...}".to_string();
                    }
                    let inner: Vec<String> = map
                        .borrow()
                        .iter()
                        .map(|(k, v)| format!("{}: {}", k, helper(v, seen)))
                        .collect();
                    format!("{{{}}}", inner.join(", "))
                }
                Value::None => "".to_string(),
            }
        }
        let mut seen = HashSet::new();
        helper(self, &mut seen)
    }
}
