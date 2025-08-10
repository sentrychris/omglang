use std::fmt;

/// Runtime errors that can occur during bytecode execution.
#[derive(Debug, PartialEq)]
pub enum RuntimeError {
    /// An assertion failed.
    AssertionError,
    /// Attempted to write to a frozen dictionary (e.g., imported module).
    FrozenWriteError,
    /// Indexing operation failed.
    IndexError(String),
    /// Dictionary key was not found.
    KeyError(String),
    /// Module import error.
    ModuleImportError(String),
    /// Invalid syntax was attempted..
    SyntaxError(String),
    /// Operation was applied to an inappropriate type.
    TypeError(String),
    /// Undefined identifier was attempted.
    UndefinedIdentError(String),
    /// Value error.
    ValueError(String),
    /// Division or modulo by zero was attempted.
    ZeroDivisionError,
    /// User-raised runtime error.
    Raised(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::AssertionError => {
                write!(f, "AssertionError: assertion failed")
            }
            RuntimeError::FrozenWriteError => {
                write!(f, "FrozenWriteError: Imported modules are read-only")
            }
            RuntimeError::IndexError(msg) => {
                write!(f, "IndexError: {}", msg)
            }
            RuntimeError::KeyError(key) => {
                write!(f, "KeyError: \"Key '{}' not found\"", key)
            }
            RuntimeError::ModuleImportError(msg) => {
                write!(f, "ModuleImportError: {}", msg)
            }
            RuntimeError::SyntaxError(msg) => {
                write!(f, "SyntaxError: {}", msg)
            }
            RuntimeError::TypeError(msg) => {
                write!(f, "TypeError: {}", msg)
            }
            RuntimeError::UndefinedIdentError(msg) => {
                write!(f, "UndefinedIdentError: {}", msg)
            }
            RuntimeError::ValueError(msg) => {
                write!(f, "ValueError: {}", msg)
            }
            RuntimeError::ZeroDivisionError => {
                write!(f, "ZeroDivisionError: integer division or modulo by zero")
            }
            RuntimeError::Raised(msg) => {
                write!(f, "RuntimeError: {}", msg)
            }
        }
    }
}

impl std::error::Error for RuntimeError {}
