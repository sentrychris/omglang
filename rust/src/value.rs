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
//! - `Dict(Rc<RefCell<OmgDict>>)` – mutable, reference-counted, insertion-ordered dictionaries
//! - `FrozenDict(Rc<OmgDict>)` – immutable dictionaries (used for imports)
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

/// One slot in a function's local environment. Wrapped in
/// `Rc<RefCell<...>>` so closures can share live storage with the
/// scope they captured — mutation in either place is visible from
/// the other, like Python or JavaScript closures. (See
/// `rust/src/vm.rs` for the read/write paths through this cell.)
pub type EnvCell = Rc<RefCell<Value>>;

/// A function's local environment (params + `alloc`-allocated locals).
/// Each binding is its own [`EnvCell`]; cloning the map clones only
/// the `Rc`s, not the values, so closures end up referencing the same
/// cells as the enclosing scope.
pub type Env = HashMap<String, EnvCell>;

pub fn new_cell(v: Value) -> EnvCell {
    Rc::new(RefCell::new(v))
}

/// An insertion-ordered string-keyed dictionary — the storage behind
/// OMG's `{...}` values.
///
/// A plain `HashMap` iterates in an order that changes from run to run,
/// which made `emit some_dict` and `dict_keys(d)` print differently every
/// time and disagree with the C and JS backends (both insertion-ordered,
/// as the language reference promises). Entries live in a `Vec` in the
/// order they were first inserted; a side index gives O(1) lookup.
/// Assigning to an existing key replaces its value in place and keeps its
/// position, like Python and JavaScript. There is no removal: the
/// language has no way to delete a key.
#[derive(Clone, Default)]
pub struct OmgDict {
    entries: Vec<(String, Value)>,
    index: HashMap<String, usize>,
}

impl OmgDict {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: String, value: Value) {
        match self.index.get(&key) {
            Some(&i) => self.entries[i].1 = value,
            None => {
                self.index.insert(key.clone(), self.entries.len());
                self.entries.push((key, value));
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.index.get(key).map(|&i| &self.entries[i].1)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Keys in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.iter().map(|(k, _)| k)
    }

    /// `(key, value)` pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }
}

/// Value type for the VM stack and environments.
#[derive(Clone)]
pub enum Value {
    /// 64-bit signed integer.
    Int(i64),
    /// 64-bit IEEE-754 floating-point number.
    Float(f64),
    /// UTF-8 string.
    Str(String),
    /// Boolean truth value.
    Bool(bool),
    /// Mutable list (reference-counted, interior-mutable).
    List(Rc<RefCell<Vec<Value>>>),
    /// Mutable dictionary (reference-counted, interior-mutable).
    Dict(Rc<RefCell<OmgDict>>),
    /// Immutable dictionary (reference-counted).
    FrozenDict(Rc<OmgDict>),
    /// First-class function reference. Top-level procs are stored as
    /// `Closure { name, captured: empty }`; nested procs capture the
    /// surrounding local environment at the point of definition.
    /// Captured bindings are [`EnvCell`]s — closures and their
    /// enclosing scope share the same cells, so mutation flows both
    /// ways (Python/JS-style by-reference capture).
    Closure {
        name: String,
        captured: Rc<Env>,
    },
    /// Sentinel for "no value".
    None,
}

impl Value {
    /// Convert the value into an integer, applying OMG coercion rules.
    ///
    /// Returns `Ok(i64)` on success, or a [`RuntimeError::TypeError`] if conversion fails.
    /// Coerce to f64. Used by mixed int/float arithmetic and the
    /// `float()` builtin.
    pub fn as_float(&self) -> Result<f64, RuntimeError> {
        match self {
            Value::Int(i) => Ok(*i as f64),
            Value::Float(f) => Ok(*f),
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Value::Str(s) => s.trim().parse::<f64>().map_err(|_| {
                RuntimeError::TypeError(format!("Invalid literal for float(): '{}'", s))
            }),
            Value::None => Ok(0.0),
            Value::List(_) | Value::Dict(_) | Value::FrozenDict(_) => {
                Err(RuntimeError::TypeError(
                    "cannot convert collection to float".to_string(),
                ))
            }
            Value::Closure { name, .. } => Err(RuntimeError::TypeError(format!(
                "cannot convert function '{}' to float",
                name
            ))),
        }
    }

    pub fn as_int(&self) -> Result<i64, RuntimeError> {
        match self {
            Value::Int(i) => Ok(*i),
            Value::Float(f) => {
                if !f.is_finite() {
                    return Err(RuntimeError::ValueError(format!(
                        "cannot convert non-finite float to int: {}",
                        f
                    )));
                }
                if !float_fits_i64(*f) {
                    return Err(RuntimeError::ValueError(format!(
                        "float {} is outside the i64 range",
                        f
                    )));
                }
                Ok(f.trunc() as i64)
            }
            Value::Str(s) => s.parse::<i64>().map_err(|_| {
                RuntimeError::TypeError(format!("Invalid literal for int(): '{}'", s))
            }),
            Value::Bool(b) => Ok(if *b { 1 } else { 0 }),
            // Compound values are *not* implicitly convertible to int. Use
            // `length(x)` if you want the count. Allowing the conversion
            // hides bugs like `5 + [1,2,3]` silently producing `8`.
            Value::List(_) => Err(RuntimeError::TypeError(
                "cannot convert list to int (use length() instead)".to_string(),
            )),
            Value::Dict(_) => Err(RuntimeError::TypeError(
                "cannot convert dict to int (use length() instead)".to_string(),
            )),
            Value::FrozenDict(_) => Err(RuntimeError::TypeError(
                "cannot convert frozen dict to int".to_string(),
            )),
            Value::Closure { name, .. } => Err(RuntimeError::TypeError(format!(
                "cannot convert function '{}' to int",
                name
            ))),
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
            // 0.0 and -0.0 are falsy; NaN is truthy (matches Python).
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.borrow().is_empty(),
            Value::Dict(d) => !d.borrow().is_empty(),
            Value::FrozenDict(d) => !d.is_empty(),
            // A live function reference is always truthy.
            Value::Closure { .. } => true,
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
                Value::Float(f) => format_float(*f),
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
                    // Only containers on the *current path* count as a
                    // cycle. Without this, a list that merely appears
                    // twice (`[a, a]`) printed its second occurrence
                    // as `[...]`.
                    seen.remove(&ptr);
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
                    seen.remove(&ptr);
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
                    seen.remove(&ptr);
                    format!("{{{}}}", inner.join(", "))
                }

                // Closures render as their function name (handy for
                // `emit some_function` in REPL sessions).
                Value::Closure { name, .. } => format!("<proc {}>", name),

                // None → empty string
                Value::None => "".to_string(),
            }
        }

        let mut seen = HashSet::new();
        helper(self, &mut seen)
    }
}

/// True when a finite `f` truncates to a value representable as `i64`.
///
/// `i64::MAX as f64` rounds *up* to 2^63, which is one past the largest
/// i64, so the upper bound has to be exclusive; `i64::MIN as f64` is
/// exactly -2^63 and is inclusive. (An inclusive upper bound let 2^63
/// through, and the `as i64` cast then silently saturated it.)
pub(crate) fn float_fits_i64(f: f64) -> bool {
    f >= -9_223_372_036_854_775_808.0 && f < 9_223_372_036_854_775_808.0
}

/// Format an f64 the way OMG prints floats: always with a decimal point so
/// `1.0` and `1` are visually distinguishable, with NaN/Infinity rendered
/// in lowercase. Uses Rust's shortest round-trippable repr (`{}`) and
/// appends `.0` if no exponent or decimal point is already present.
pub(crate) fn format_float(f: f64) -> String {
    if f.is_nan() {
        return "nan".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    let s = format!("{}", f);
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{}.0", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(items: Vec<Value>) -> Value {
        Value::List(Rc::new(RefCell::new(items)))
    }

    #[test]
    fn shared_list_is_not_mistaken_for_a_cycle() {
        let a = list(vec![Value::Int(1)]);
        let outer = list(vec![a.clone(), a.clone()]);
        assert_eq!(outer.to_string(), "[[1], [1]]");
    }

    #[test]
    fn shared_dict_is_not_mistaken_for_a_cycle() {
        let mut m = OmgDict::new();
        m.insert("x".to_string(), Value::Int(1));
        let d = Value::Dict(Rc::new(RefCell::new(m)));
        let outer = list(vec![d.clone(), d.clone()]);
        assert_eq!(outer.to_string(), "[{x: 1}, {x: 1}]");
    }

    #[test]
    fn dicts_keep_insertion_order() {
        let mut m = OmgDict::new();
        for (i, k) in ["zeta", "alpha", "mid", "beta", "omega"].iter().enumerate() {
            m.insert(k.to_string(), Value::Int(i as i64));
        }
        // Re-assigning an existing key keeps its position.
        m.insert("alpha".to_string(), Value::Int(99));
        let keys: Vec<&str> = m.keys().map(|k| k.as_str()).collect();
        assert_eq!(keys, ["zeta", "alpha", "mid", "beta", "omega"]);
        assert_eq!(m.len(), 5);
        assert_eq!(
            Value::Dict(Rc::new(RefCell::new(m))).to_string(),
            "{zeta: 0, alpha: 99, mid: 2, beta: 3, omega: 4}"
        );
    }

    #[test]
    fn real_cycles_still_print_an_ellipsis() {
        let inner = Rc::new(RefCell::new(vec![Value::Int(1)]));
        let v = Value::List(inner.clone());
        inner.borrow_mut().push(v.clone());
        assert_eq!(v.to_string(), "[1, [...]]");
        // Break the cycle so the test doesn't leak.
        inner.borrow_mut().clear();
    }

    #[test]
    fn float_to_int_range_is_exact_at_the_edges() {
        assert!(Value::Float(9.223372036854775e18).as_int().is_ok());
        assert!(Value::Float(9_223_372_036_854_775_808.0).as_int().is_err());
        assert_eq!(
            Value::Float(-9_223_372_036_854_775_808.0).as_int(),
            Ok(i64::MIN)
        );
        assert!(Value::Float(-9.3e18).as_int().is_err());
    }
}
