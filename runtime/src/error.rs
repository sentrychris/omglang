//! # Error Handling for the OMG VM
//!
//! This module defines the **error kinds** and **runtime errors** used
//! throughout the OMG bytecode virtual machine.
//!
//! ## Design
//! - [`ErrorKind`] is a compact, `repr(u8)` enumeration of *categories* of
//!   errors. These map directly to bytecode-level opcodes (`Instr::Raise`) and
//!   serialized values in `.omgb` files.
//! - [`RuntimeError`] is a richer enum representing actual errors that can
//!   occur at runtime. It includes both categorized errors (`TypeError`,
//!   `ValueError`, etc.) and structural ones (`AssertionError`, `VmInvariant`).
//!
//! ## Conversion
//! - `ErrorKind::into_runtime(msg)` upgrades an `ErrorKind` into the
//!   appropriate [`RuntimeError`] variant, embedding a descriptive message.
//! - `TryFrom<u8>` allows decoding error kinds from bytecode.
//!
//! ## Display
//! - Implements [`fmt::Display`] for `RuntimeError`, providing
//!   human-readable messages (similar to Python/Java exceptions).
//! - Implements [`std::error::Error`] so `RuntimeError` integrates with Rust’s
//!   standard error handling ecosystem.

use std::fmt;

/// Compact enum of error categories used in bytecode and raise instructions.
///
/// Each variant has a fixed numeric representation (`repr(u8)`), ensuring
/// compatibility with serialized bytecode.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// Generic user-raised error (maps to `RuntimeError::Raised`).
    Generic = 0,
    /// Syntax error (usually caught at parse time, but may surface dynamically).
    Syntax = 1,
    /// Type mismatch (wrong operand type, invalid builtin args, etc).
    Type = 2,
    /// Undefined identifier reference.
    UndefinedIdent = 3,
    /// General value error (bad range, invalid argument, etc).
    Value = 4,
    /// Failure to import a module.
    ModuleImport = 5,
}

impl ErrorKind {
    /// Convert this `ErrorKind` into a fully descriptive [`RuntimeError`],
    /// embedding the provided error message.
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
    /// Attempt to convert a raw `u8` (from bytecode) into an [`ErrorKind`].
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

/// Errors that can occur during OMG bytecode execution.
///
/// Unlike [`ErrorKind`], this enum provides *structured* error information and
/// detailed messages for debugging and user reporting.
#[derive(Debug, PartialEq)]
pub enum RuntimeError {
    /// An `assert` instruction failed.
    AssertionError,
    /// Attempted to write to a frozen dictionary (e.g., imported module).
    FrozenWriteError,
    /// Indexing operation failed (list/str index out of bounds).
    IndexError(String),
    /// Dictionary key was not found.
    KeyError(String),
    /// Module import failed.
    ModuleImportError(String),
    /// Invalid or unexpected syntax was encountered.
    SyntaxError(String),
    /// Operation was applied to an inappropriate type.
    TypeError(String),
    /// Undefined identifier was referenced.
    UndefinedIdentError(String),
    /// General value error (e.g., bad argument).
    ValueError(String),
    /// Division or modulo by zero attempted.
    ZeroDivisionError,
    /// User-raised error (`raise` or `panic`).
    Raised(String),
    /// Internal VM invariant violation (represents a bug or logic failure).
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

/// Integrates `RuntimeError` with the standard `Error` trait so it can be
/// used in `Result<T, RuntimeError>` and interoperate with libraries like `anyhow`.
impl std::error::Error for RuntimeError {}
