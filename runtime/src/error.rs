use std::fmt;

/// Runtime errors that can occur during bytecode execution.
#[derive(Debug, PartialEq)]
pub enum RuntimeError {
    /// Attempted to write to a frozen dictionary (e.g., imported module).
    FrozenWriteError,
    /// Accessed a variable that does not exist in the current scope.
    UndefinedVariable(String),
    /// Type mismatch for an operation.
    TypeError(String),
    /// Invalid index access (e.g., out of bounds).
    IndexError(String),
    /// Missing key or attribute access on dictionaries.
    KeyError(String),
    /// Explicit failure raised from the `panic` builtin.
    Panic(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::FrozenWriteError => {
                write!(f, "FrozenWriteError: Imported modules are read-only")
            }
            RuntimeError::UndefinedVariable(name) => {
                write!(f, "UndefinedVariable: {}", name)
            }
            RuntimeError::TypeError(msg) => write!(f, "TypeError: {}", msg),
            RuntimeError::IndexError(msg) => write!(f, "IndexError: {}", msg),
            RuntimeError::KeyError(key) => write!(f, "KeyError: {}", key),
            RuntimeError::Panic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for RuntimeError {}
