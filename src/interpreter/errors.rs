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
        address: Option<u64>,
        location: SourceLocation,
    },

    /// Null pointer dereference
    NullDereference { location: SourceLocation },

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
    OutOfMemory { requested: usize, limit: usize },

    /// Snapshot history limit exceeded
    SnapshotLimitExceeded { current: usize, limit: usize },

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

    /// No stack frame available
    NoStackFrame { location: SourceLocation },

    /// Main function not found
    NoMainFunction,

    /// Division by zero or modulo by zero
    DivisionError {
        operation: String,
        location: SourceLocation,
    },

    /// Function argument count mismatch
    ArgumentCountMismatch {
        function: String,
        expected: usize,
        got: usize,
        location: SourceLocation,
    },

    /// Invalid pointer (invalid stack pointer, unknown pointer type, etc.)
    InvalidPointer {
        message: String,
        address: Option<u64>,
        location: SourceLocation,
    },

    /// Invalid stack frame depth
    InvalidFrameDepth { location: SourceLocation },

    /// Struct field not found
    MissingStructField {
        struct_name: String,
        field_name: String,
        location: SourceLocation,
    },

    /// Struct definition not found
    StructNotDefined {
        name: String,
        location: SourceLocation,
    },

    /// Unsupported operation or feature
    UnsupportedOperation {
        message: String,
        location: SourceLocation,
    },

    /// Invalid memory operation (heap read/write failure)
    InvalidMemoryOperation {
        message: String,
        location: SourceLocation,
    },

    /// Invalid string (too long, invalid UTF-8, etc.)
    InvalidString {
        message: String,
        location: SourceLocation,
    },

    /// Invalid malloc size
    InvalidMallocSize { size: i32, location: SourceLocation },

    /// History/snapshot operation failed
    HistoryOperationFailed {
        message: String,
        location: SourceLocation,
    },

    /// Execution is paused waiting for scanf input (internal signal, not a real error)
    ScanfNeedsInput { location: SourceLocation },
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
            RuntimeError::NoStackFrame { location } => Some(location),
            RuntimeError::DivisionError { location, .. } => Some(location),
            RuntimeError::ArgumentCountMismatch { location, .. } => Some(location),
            RuntimeError::InvalidPointer { location, .. } => Some(location),
            RuntimeError::InvalidFrameDepth { location } => Some(location),
            RuntimeError::MissingStructField { location, .. } => Some(location),
            RuntimeError::StructNotDefined { location, .. } => Some(location),
            RuntimeError::UnsupportedOperation { location, .. } => Some(location),
            RuntimeError::InvalidMemoryOperation { location, .. } => Some(location),
            RuntimeError::InvalidString { location, .. } => Some(location),
            RuntimeError::InvalidMallocSize { location, .. } => Some(location),
            RuntimeError::HistoryOperationFailed { location, .. } => Some(location),
            RuntimeError::ScanfNeedsInput { location } => Some(location),
            RuntimeError::OutOfMemory { .. } => None,
            RuntimeError::SnapshotLimitExceeded { .. } => None,
            RuntimeError::NoMainFunction => None,
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::UninitializedRead { var, location, .. } => {
                write!(
                    f,
                    "Read from uninitialized variable '{}' at line {}",
                    var, location.line
                )
            }
            RuntimeError::NullDereference { location } => {
                write!(f, "Null pointer dereference at line {}", location.line)
            }
            RuntimeError::BufferOverrun {
                index,
                size,
                location,
            } => {
                write!(
                    f,
                    "Buffer overrun at line {}: index {} out of bounds for size {}",
                    location.line, index, size
                )
            }
            RuntimeError::ConstModification { var, location } => {
                write!(
                    f,
                    "Attempted to modify const variable '{}' at line {}",
                    var, location.line
                )
            }
            RuntimeError::IntegerOverflow {
                operation,
                location,
            } => {
                write!(
                    f,
                    "Integer overflow in operation: {} at line {}",
                    operation, location.line
                )
            }
            RuntimeError::OutOfMemory { requested, limit } => {
                write!(
                    f,
                    "Out of memory: requested {} bytes, limit is {}",
                    requested, limit
                )
            }
            RuntimeError::SnapshotLimitExceeded { current, limit } => {
                write!(
                    f,
                    "Snapshot memory limit exceeded: {} bytes used, limit is {}",
                    current, limit
                )
            }
            RuntimeError::UseAfterFree { address, location } => {
                write!(
                    f,
                    "Use-after-free: address 0x{:x} at line {}",
                    address, location.line
                )
            }
            RuntimeError::DoubleFree { address, location } => {
                write!(
                    f,
                    "Double free at address 0x{:x} at line {}",
                    address, location.line
                )
            }
            RuntimeError::InvalidFree { address, location } => {
                write!(
                    f,
                    "Invalid free: address 0x{:x} at line {}",
                    address, location.line
                )
            }
            RuntimeError::UndefinedFunction { name, location } => {
                write!(f, "Undefined function '{}' at line {}", name, location.line)
            }
            RuntimeError::UndefinedVariable { name, location } => {
                write!(f, "Undefined variable '{}' at line {}", name, location.line)
            }
            RuntimeError::TypeError {
                expected,
                got,
                location,
            } => {
                write!(
                    f,
                    "Type error at line {}: expected {}, got {}",
                    location.line, expected, got
                )
            }
            RuntimeError::InvalidPrintfFormat { message, location } => {
                write!(
                    f,
                    "Invalid printf format at line {}: {}",
                    location.line, message
                )
            }
            RuntimeError::NoStackFrame { location } => {
                write!(f, "No stack frame available at line {}", location.line)
            }
            RuntimeError::NoMainFunction => {
                write!(f, "No main() function found")
            }
            RuntimeError::DivisionError {
                operation,
                location,
            } => {
                write!(f, "{} at line {}", operation, location.line)
            }
            RuntimeError::ArgumentCountMismatch {
                function,
                expected,
                got,
                location,
            } => {
                write!(
                    f,
                    "Function '{}' expects {} argument{}, got {} at line {}",
                    function,
                    expected,
                    if *expected == 1 { "" } else { "s" },
                    got,
                    location.line
                )
            }
            RuntimeError::InvalidPointer {
                message,
                address,
                location,
            } => {
                if let Some(addr) = address {
                    write!(
                        f,
                        "Invalid pointer at 0x{:x}: {} at line {}",
                        addr, message, location.line
                    )
                } else {
                    write!(f, "Invalid pointer: {} at line {}", message, location.line)
                }
            }
            RuntimeError::InvalidFrameDepth { location } => {
                write!(f, "Invalid stack frame depth at line {}", location.line)
            }
            RuntimeError::MissingStructField {
                struct_name,
                field_name,
                location,
            } => {
                write!(
                    f,
                    "Struct '{}' does not have field '{}' at line {}",
                    struct_name, field_name, location.line
                )
            }
            RuntimeError::StructNotDefined { name, location } => {
                write!(
                    f,
                    "Struct '{}' is not defined at line {}",
                    name, location.line
                )
            }
            RuntimeError::UnsupportedOperation { message, location } => {
                write!(
                    f,
                    "Unsupported operation: {} at line {}",
                    message, location.line
                )
            }
            RuntimeError::InvalidMemoryOperation { message, location } => {
                write!(
                    f,
                    "Memory operation failed: {} at line {}",
                    message, location.line
                )
            }
            RuntimeError::InvalidString { message, location } => {
                write!(f, "Invalid string: {} at line {}", message, location.line)
            }
            RuntimeError::InvalidMallocSize { size, location } => {
                write!(
                    f,
                    "Invalid malloc size: {} (must be positive) at line {}",
                    size, location.line
                )
            }
            RuntimeError::HistoryOperationFailed { message, location } => {
                write!(
                    f,
                    "History operation failed: {} at line {}",
                    message, location.line
                )
            }
            RuntimeError::ScanfNeedsInput { location } => {
                write!(f, "scanf needs input at line {}", location.line)
            }
        }
    }
}

impl std::error::Error for RuntimeError {}
