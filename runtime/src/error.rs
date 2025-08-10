use std::fmt;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Generic = 0,
    Syntax = 1,
    Type = 2,
    UndefinedIdent = 3,
    Value = 4,
    ModuleImport = 5,
}

impl ErrorKind {
    pub fn into_runtime(self, msg: String) -> RuntimeError {
        match self {
            ErrorKind::Generic => RuntimeError::Raised(msg),
            ErrorKind::Syntax => RuntimeError::SyntaxError(msg),
            ErrorKind::Type => RuntimeError::TypeError(msg),
            ErrorKind::UndefinedIdent => RuntimeError::UndefinedIdentError(msg),
            ErrorKind::Value => RuntimeError::ValueError(msg),
            ErrorKind::ModuleImport => RuntimeError::ModuleImportError(msg),
        }
    }
}

impl TryFrom<u8> for ErrorKind {
    type Error = ();
    fn try_from(v: u8) -> Result<Self, ()> {
        use ErrorKind::*;
        Ok(match v {
            0 => Generic,
            1 => Syntax,
            2 => Type,
            3 => UndefinedIdent,
            4 => Value,
            5 => ModuleImport,
            _ => return Err(()),
        })
    }
}

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
    /// Internal VM invariant violation.
    VmInvariant(String),
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
            RuntimeError::VmInvariant(msg) => {
                write!(f, "VmInvariant: {}", msg)
            }
        }
    }
}

impl std::error::Error for RuntimeError {}
