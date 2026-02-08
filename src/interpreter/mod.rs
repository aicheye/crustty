//! C interpreter execution engine
//!
//! This module provides the core execution logic:
//! - [`engine`]: Main interpreter with AST execution
//! - [`errors`]: Runtime error types
//!
//! # Execution Model
//!
//! The interpreter walks the AST and executes statements one at a time.
//! After each statement, a snapshot is taken to enable time-travel debugging.
//!
//! # Built-in Functions
//!
//! Built-in functions (`printf`, `malloc`, `free`, `sizeof`) are implemented
//! directly in the engine rather than as separate modules.

pub mod errors;
pub mod engine;
