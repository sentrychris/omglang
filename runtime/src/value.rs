//! # Value Representation for the OMG VM
//!
//! This module defines [`Value`], the universal runtime type used on the
//! OMG virtual machine’s operand stack, in environments, and in data
//! structures.
//!
//! ## Supported types
//! - `Int(i64)` – 64-bit signed integer
//! - `Str(String)` – UTF-8 string
//! - `Bool(bool)` – boolean truth values
//! - `List(Rc<RefCell<Vec<Value>>>)` – mutable, reference-counted lists
//! - `Dict(Rc<RefCell<HashMap<String, Value>>>)` – mutable, reference-counted dictionaries
//! - `FrozenDict(Rc<HashMap<String, Value>>)` – immutable dictionaries (used for imports)
//! - `None` – sentinel for “no value” (similar to Python’s `None` / JS’s `undefined`)
//!
//! ## Design
//! - `Rc<RefCell<...>>` enables multiple references to a collection while allowing
//!   safe mutation when borrowed mutably at runtime.
//! - `FrozenDict` ensures that imported namespaces and constants remain immutable.
//! - Convenience methods (`as_int`, `as_bool`, `to_string`) provide coercion rules
//!   consistent with OMG’s dynamic typing.
//!
//! ## Coercion rules
//! - **Integer conversion (`as_int`)**:
//!   - `Int` → itself
//!   - `Str` → parse as integer or error
//!   - `Bool` → true → 1, false → 0
//!   - `List`/`Dict`/`FrozenDict` → length
//!   - `None` → 0
//! - **Boolean conversion (`as_bool`)**:
//!   - Falsy: `false`, `0`, `""`, `[]`, `{}`, `None`
//!   - Truthy: everything else
//! - **String conversion (`to_string`)**:
//!   - Provides human-readable representations, with recursion detection
//!     (`[...]`, `{...}`) to prevent infinite loops on cyclic structures.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::error::RuntimeError;

/// Value type for the VM stack and environments.
#[derive(Clone)]
pub enum Value {
    /// 64-bit signed integer.
    Int(i64),
    /// UTF-8 string.
    Str(String),
    /// Boolean truth value.
    Bool(bool),
    /// Mutable list (reference-counted, interior-mutable).
    List(Rc<RefCell<Vec<Value>>>),
    /// Mutable dictionary (reference-counted, interior-mutable).
    Dict(Rc<RefCell<HashMap<String, Value>>>),
    /// Immutable dictionary (reference-counted).
    FrozenDict(Rc<HashMap<String, Value>>),
    /// Sentinel for “no value”.
    None,
}

impl Value {
    /// Convert the value into an integer, applying OMG coercion rules.
    ///
    /// Returns `Ok(i64)` on success, or a [`RuntimeError::TypeError`] if conversion fails.
    pub fn as_int(&self) -> Result<i64, RuntimeError> {
        match self {
            Value::Int(i) => Ok(*i),
            Value::Str(s) => s.parse::<i64>().map_err(|_| {
                RuntimeError::TypeError(format!("Invalid literal for int(): '{}'", s))
            }),
            Value::Bool(b) => Ok(if *b { 1 } else { 0 }),
            Value::List(l) => Ok(l.borrow().len() as i64),
            Value::Dict(d) => Ok(d.borrow().len() as i64),
            Value::FrozenDict(d) => Ok(d.len() as i64),
            Value::None => Ok(0),
        }
    }

    /// Convert the value into a boolean (truthiness semantics).
    ///
    /// - Falsy: `false`, `0`, `""`, `[]`, `{}`, `None`
    /// - Truthy: everything else
    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.borrow().is_empty(),
            Value::Dict(d) => !d.borrow().is_empty(),
            Value::FrozenDict(d) => !d.is_empty(),
            Value::None => false,
        }
    }

    /// Convert the value into a human-readable string.
    ///
    /// Cyclic structures are handled gracefully:
    /// - Lists that refer back to themselves print as `[...]`.
    /// - Dicts that refer back to themselves print as `{...}`.
    ///
    /// This prevents infinite recursion during formatting.
    pub fn to_string(&self) -> String {
        /// Helper for recursive stringification, tracking seen pointers.
        fn helper(val: &Value, seen: &mut HashSet<usize>) -> String {
            match val {
                Value::Int(i) => i.to_string(),
                Value::Str(s) => s.clone(),
                Value::Bool(b) => b.to_string(),

                // List: detect cycles by pointer identity
                Value::List(list) => {
                    let ptr = Rc::as_ptr(list) as usize;
                    if !seen.insert(ptr) {
                        return "[...]".to_string();
                    }
                    let inner: Vec<String> =
                        list.borrow().iter().map(|v| helper(v, seen)).collect();
                    format!("[{}]", inner.join(", "))
                }

                // Dict: detect cycles by pointer identity
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

                // FrozenDict: same as Dict but without mutability
                Value::FrozenDict(map) => {
                    let ptr = Rc::as_ptr(map) as usize;
                    if !seen.insert(ptr) {
                        return "{...}".to_string();
                    }
                    let inner: Vec<String> = map
                        .iter()
                        .map(|(k, v)| format!("{}: {}", k, helper(v, seen)))
                        .collect();
                    format!("{{{}}}", inner.join(", "))
                }

                // None → empty string
                Value::None => "".to_string(),
            }
        }

        let mut seen = HashSet::new();
        helper(self, &mut seen)
    }
}
