//! Runtime error types for the C interpreter
//!
//! This module defines [`RuntimeError`], which represents all errors that can occur
//! during program execution (as opposed to parse errors or system errors).
//!
//! All runtime errors are fatal - they halt execution and display diagnostic information.

use crate::parser::ast::SourceLocation;
use std::fmt;

/// Runtime errors that can occur during execution
#[derive(Debug, Clone)]
pub enum RuntimeError {
    /// Attempted to read an uninitialized variable
    UninitializedRead {
        var: String,
        location: SourceLocation,
    },

    /// Null pointer dereference
    NullDereference {
        location: SourceLocation,
    },

    /// Buffer overrun (array index out of bounds or invalid pointer offset)
    BufferOverrun {
        index: usize,
        size: usize,
        location: SourceLocation,
    },

    /// Attempted to modify a const variable
    ConstModification {
        var: String,
        location: SourceLocation,
    },

    /// Integer overflow in arithmetic operation
    IntegerOverflow {
        operation: String,
        location: SourceLocation,
    },

    /// Out of heap memory
    OutOfMemory {
        requested: usize,
        limit: usize,
    },

    /// Snapshot history limit exceeded
    SnapshotLimitExceeded {
        current: usize,
        limit: usize,
    },

    /// Use-after-free (accessing freed memory)
    UseAfterFree {
        address: u64,
        location: SourceLocation,
    },

    /// Double free
    DoubleFree {
        address: u64,
        location: SourceLocation,
    },

    /// Invalid free (freeing non-allocated memory)
    InvalidFree {
        address: u64,
        location: SourceLocation,
    },

    /// Undefined function call
    UndefinedFunction {
        name: String,
        location: SourceLocation,
    },

    /// Undefined variable reference
    UndefinedVariable {
        name: String,
        location: SourceLocation,
    },

    /// Type error
    TypeError {
        expected: String,
        got: String,
        location: SourceLocation,
    },

    /// Invalid printf format string
    InvalidPrintfFormat {
        message: String,
        location: SourceLocation,
    },

    /// Generic error with message
    Generic {
        message: String,
        location: SourceLocation,
    },
}

impl RuntimeError {
    pub fn location(&self) -> Option<&SourceLocation> {
        match self {
            RuntimeError::UninitializedRead { location, .. } => Some(location),
            RuntimeError::NullDereference { location } => Some(location),
            RuntimeError::BufferOverrun { location, .. } => Some(location),
            RuntimeError::ConstModification { location, .. } => Some(location),
            RuntimeError::IntegerOverflow { location, .. } => Some(location),
            RuntimeError::UseAfterFree { location, .. } => Some(location),
            RuntimeError::DoubleFree { location, .. } => Some(location),
            RuntimeError::InvalidFree { location, .. } => Some(location),
            RuntimeError::UndefinedFunction { location, .. } => Some(location),
            RuntimeError::UndefinedVariable { location, .. } => Some(location),
            RuntimeError::TypeError { location, .. } => Some(location),
            RuntimeError::InvalidPrintfFormat { location, .. } => Some(location),
            RuntimeError::Generic { location, .. } => Some(location),
            RuntimeError::OutOfMemory { .. } => None,
            RuntimeError::SnapshotLimitExceeded { .. } => None,
        }
    }

    /// Format the error for display
    pub fn format(&self, source_lines: &[String]) -> String {
        let mut output = String::new();

        match self {
            RuntimeError::UninitializedRead { var, location } => {
                output.push_str(&format!("Runtime Error at line {}:\n", location.line));
                output.push_str(&format!("  Read from uninitialized variable '{}'\n\n", var));
                if location.line > 0 && location.line <= source_lines.len() {
                    output.push_str(&format!("  {} | {}\n", location.line, source_lines[location.line - 1]));
                }
            }
            RuntimeError::NullDereference { location } => {
                output.push_str(&format!("Runtime Error at line {}:\n", location.line));
                output.push_str("  Null pointer dereference\n\n");
                if location.line > 0 && location.line <= source_lines.len() {
                    output.push_str(&format!("  {} | {}\n", location.line, source_lines[location.line - 1]));
                }
            }
            RuntimeError::BufferOverrun { index, size, location } => {
                output.push_str(&format!("Runtime Error at line {}:\n", location.line));
                output.push_str(&format!("  Buffer overrun: index {} out of bounds for size {}\n\n", index, size));
                if location.line > 0 && location.line <= source_lines.len() {
                    output.push_str(&format!("  {} | {}\n", location.line, source_lines[location.line - 1]));
                }
            }
            RuntimeError::ConstModification { var, location } => {
                output.push_str(&format!("Runtime Error at line {}:\n", location.line));
                output.push_str(&format!("  Attempted to modify const variable '{}'\n\n", var));
                if location.line > 0 && location.line <= source_lines.len() {
                    output.push_str(&format!("  {} | {}\n", location.line, source_lines[location.line - 1]));
                }
            }
            RuntimeError::IntegerOverflow { operation, location } => {
                output.push_str(&format!("Runtime Error at line {}:\n", location.line));
                output.push_str(&format!("  Integer overflow in operation: {}\n\n", operation));
                if location.line > 0 && location.line <= source_lines.len() {
                    output.push_str(&format!("  {} | {}\n", location.line, source_lines[location.line - 1]));
                }
            }
            RuntimeError::OutOfMemory { requested, limit } => {
                output.push_str("Runtime Error:\n");
                output.push_str(&format!("  Out of memory: requested {} bytes, limit is {} bytes\n", requested, limit));
            }
            RuntimeError::SnapshotLimitExceeded { current, limit } => {
                output.push_str("Runtime Error:\n");
                output.push_str(&format!("  Snapshot memory limit exceeded: {} bytes used, limit is {} bytes\n", current, limit));
                output.push_str("  Consider using a smaller program or fewer execution steps.\n");
            }
            RuntimeError::UseAfterFree { address, location } => {
                output.push_str(&format!("Runtime Error at line {}:\n", location.line));
                output.push_str(&format!("  Use-after-free: address 0x{:x} has been freed\n\n", address));
                if location.line > 0 && location.line <= source_lines.len() {
                    output.push_str(&format!("  {} | {}\n", location.line, source_lines[location.line - 1]));
                }
            }
            RuntimeError::DoubleFree { address, location } => {
                output.push_str(&format!("Runtime Error at line {}:\n", location.line));
                output.push_str(&format!("  Double free detected at address 0x{:x}\n\n", address));
                if location.line > 0 && location.line <= source_lines.len() {
                    output.push_str(&format!("  {} | {}\n", location.line, source_lines[location.line - 1]));
                }
            }
            RuntimeError::InvalidFree { address, location } => {
                output.push_str(&format!("Runtime Error at line {}:\n", location.line));
                output.push_str(&format!("  Invalid free: address 0x{:x} was never allocated\n\n", address));
                if location.line > 0 && location.line <= source_lines.len() {
                    output.push_str(&format!("  {} | {}\n", location.line, source_lines[location.line - 1]));
                }
            }
            RuntimeError::UndefinedFunction { name, location } => {
                output.push_str(&format!("Runtime Error at line {}:\n", location.line));
                output.push_str(&format!("  Undefined function: '{}'\n\n", name));
                if location.line > 0 && location.line <= source_lines.len() {
                    output.push_str(&format!("  {} | {}\n", location.line, source_lines[location.line - 1]));
                }
            }
            RuntimeError::UndefinedVariable { name, location } => {
                output.push_str(&format!("Runtime Error at line {}:\n", location.line));
                output.push_str(&format!("  Undefined variable: '{}'\n\n", name));
                if location.line > 0 && location.line <= source_lines.len() {
                    output.push_str(&format!("  {} | {}\n", location.line, source_lines[location.line - 1]));
                }
            }
            RuntimeError::TypeError { expected, got, location } => {
                output.push_str(&format!("Runtime Error at line {}:\n", location.line));
                output.push_str(&format!("  Type error: expected {}, got {}\n\n", expected, got));
                if location.line > 0 && location.line <= source_lines.len() {
                    output.push_str(&format!("  {} | {}\n", location.line, source_lines[location.line - 1]));
                }
            }
            RuntimeError::InvalidPrintfFormat { message, location } => {
                output.push_str(&format!("Runtime Error at line {}:\n", location.line));
                output.push_str(&format!("  Invalid printf format: {}\n\n", message));
                if location.line > 0 && location.line <= source_lines.len() {
                    output.push_str(&format!("  {} | {}\n", location.line, source_lines[location.line - 1]));
                }
            }
            RuntimeError::Generic { message, location } => {
                output.push_str(&format!("Runtime Error at line {}:\n", location.line));
                output.push_str(&format!("  {}\n\n", message));
                if location.line > 0 && location.line <= source_lines.len() {
                    output.push_str(&format!("  {} | {}\n", location.line, source_lines[location.line - 1]));
                }
            }
        }

        output
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::UninitializedRead { var, location } => {
                write!(f, "Read from uninitialized variable '{}' at line {}", var, location.line)
            }
            RuntimeError::NullDereference { location } => {
                write!(f, "Null pointer dereference at line {}", location.line)
            }
            RuntimeError::BufferOverrun { index, size, location } => {
                write!(f, "Buffer overrun at line {}: index {} out of bounds for size {}",
                       location.line, index, size)
            }
            RuntimeError::ConstModification { var, location } => {
                write!(f, "Attempted to modify const variable '{}' at line {}", var, location.line)
            }
            RuntimeError::IntegerOverflow { operation, location } => {
                write!(f, "Integer overflow in operation: {} at line {}", operation, location.line)
            }
            RuntimeError::OutOfMemory { requested, limit } => {
                write!(f, "Out of memory: requested {} bytes, limit is {}", requested, limit)
            }
            RuntimeError::SnapshotLimitExceeded { current, limit } => {
                write!(f, "Snapshot memory limit exceeded: {} bytes used, limit is {}", current, limit)
            }
            RuntimeError::UseAfterFree { address, location } => {
                write!(f, "Use-after-free: address 0x{:x} at line {}", address, location.line)
            }
            RuntimeError::DoubleFree { address, location } => {
                write!(f, "Double free at address 0x{:x} at line {}", address, location.line)
            }
            RuntimeError::InvalidFree { address, location } => {
                write!(f, "Invalid free: address 0x{:x} at line {}", address, location.line)
            }
            RuntimeError::UndefinedFunction { name, location } => {
                write!(f, "Undefined function '{}' at line {}", name, location.line)
            }
            RuntimeError::UndefinedVariable { name, location } => {
                write!(f, "Undefined variable '{}' at line {}", name, location.line)
            }
            RuntimeError::TypeError { expected, got, location } => {
                write!(f, "Type error at line {}: expected {}, got {}", location.line, expected, got)
            }
            RuntimeError::InvalidPrintfFormat { message, location } => {
                write!(f, "Invalid printf format at line {}: {}", location.line, message)
            }
            RuntimeError::Generic { message, location } => {
                write!(f, "Error at line {}: {}", location.line, message)
            }
        }
    }
}

impl std::error::Error for RuntimeError {}
