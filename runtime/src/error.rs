use std::fmt;

/// Runtime errors that can occur during bytecode execution.
#[derive(Debug, PartialEq)]
pub enum RuntimeError {
    /// Attempted to write to a frozen dictionary (e.g., imported module).
    FrozenWriteError,
    /// Division or modulo by zero was attempted.
    ZeroDivisionError,
    /// Dictionary key was not found.
    KeyError(String),
    /// Operation was applied to an inappropriate type.
    TypeError(String),
    /// Indexing operation failed.
    IndexError(String),
    /// User-raised runtime error.
    Raised(String),
    /// An assertion failed.
    AssertionError,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::FrozenWriteError => {
                write!(f, "FrozenWriteError: Imported modules are read-only")
            }
            RuntimeError::ZeroDivisionError => {
                write!(f, "ZeroDivisionError: integer division or modulo by zero")
            }
            RuntimeError::KeyError(key) => {
                write!(f, "KeyError: \"Key '{}' not found\"", key)
            }
            RuntimeError::TypeError(msg) => {
                write!(f, "TypeError: {}", msg)
            }
            RuntimeError::IndexError(msg) => {
                write!(f, "RuntimeError: {}", msg)
            }
            RuntimeError::Raised(msg) => {
                write!(f, "RuntimeError: {}", msg)
            }
            RuntimeError::AssertionError => {
                write!(f, "AssertionError: assertion failed")
            }
        }
    }
}

impl std::error::Error for RuntimeError {}
