use std::fmt;

/// Runtime errors that can occur during bytecode execution.
#[derive(Debug, PartialEq)]
pub enum RuntimeError {
    /// Attempted to write to a frozen dictionary (e.g., imported module).
    FrozenWriteError,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::FrozenWriteError => {
                write!(f, "FrozenWriteError: Imported modules are read-only")
            }
        }
    }
}

impl std::error::Error for RuntimeError {}
