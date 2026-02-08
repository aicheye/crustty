//! C interpreter execution engine
//!
//! This module provides the core C language interpreter, organized into
//! focused submodules for maintainability and clarity.
//!
//! # Module Organization
//!
//! - [`engine`]: Core interpreter struct, initialization, execution loop, and snapshot management
//! - [`statements`]: Statement execution (if, while, for, switch, return, variable declarations)
//! - [`expressions`]: Expression evaluation, operators, and arithmetic
//! - [`builtins`]: Built-in function implementations (printf, malloc, free)
//! - [`memory_ops`]: Memory operations, assignments, heap serialization, struct field access
//! - [`type_system`]: Type inference for expressions and type compatibility
//! - [`errors`]: Comprehensive runtime error types
//! - [`constants`]: Interpreter constants (address spaces, size limits)
//!
//! # Execution Model
//!
//! The interpreter executes C programs by walking the parsed AST:
//!
//! 1. **Initialization**: Load struct and function definitions from the AST
//! 2. **Execution**: Start from `main()` and execute statements sequentially
//! 3. **Snapshots**: After each statement, capture a snapshot for time-travel debugging
//! 4. **Stack/Heap**: Manage call stack and dynamic memory allocation
//!
//! # Example
//!
//! ```rust,no_run
//! use crustty::interpreter::engine::Interpreter;
//! use crustty::parser::parse::Parser;
//!
//! let code = r#"
//!     int main() {
//!         int x = 42;
//!         return x;
//!     }
//! "#;
//!
//! let mut parser = Parser::new(code).unwrap();
//! let program = parser.parse_program().unwrap();
//! let mut interpreter = Interpreter::new(program, 1000);
//! interpreter.run().unwrap();
//! ```
//!
//! # Memory Management
//!
//! - **Stack**: Automatic memory for local variables, function parameters, and return addresses
//! - **Heap**: Dynamic memory allocated via `malloc()` and freed via `free()`
//! - **Address Space**: Stack grows from `0x7fff_0000`, heap grows from `0x0000_1000`
//!
//! # Time-Travel Debugging
//!
//! The interpreter maintains snapshots of execution state, enabling:
//! - Step backward through execution history
//! - Jump to any previous execution point
//! - Inspect past state of stack and heap

pub mod builtins;
pub mod constants;
pub mod engine;
pub mod errors;
pub mod expressions;
pub mod memory_ops;
pub mod statements;
pub mod type_system;
