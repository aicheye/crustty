//! # Introduction
//!
//! CRusTTY parses and executes a subset of C, capturing a snapshot of the full
//! interpreter state before each statement.  The snapshot history is then
//! navigated forward and backward through a terminal UI built with
//! [ratatui](https://docs.rs/ratatui).
//!
//! ## Execution pipeline
//!
//! ```text
//! Source → Lexer → Parser → AST → Interpreter → Snapshots → TUI
//! ```
//!
//! 1. [`parser`] — tokenises the source and builds an AST.
//! 2. [`interpreter`] — walks the AST, executes statements, and captures
//!    [`snapshot::Snapshot`]s at each step.
//! 3. [`memory`] — the in-process memory model: tagged [`memory::value::Value`]
//!    variants stored in a virtual [`memory::stack::Stack`] and
//!    [`memory::heap::Heap`].
//! 4. [`snapshot`] — snapshot ring with configurable memory limit and a
//!    [`snapshot::MockTerminal`] that records `printf` output.
//! 5. [`ui`] — ratatui-based TUI; not part of the stable library API.
//!
//! ## Supported C subset
//!
//! Types: `int`, `char`, `void`, structs, pointers, fixed-size arrays.
//! Control flow: `if/else`, `while`, `for`, `do-while`, `switch/case`,
//! `break`, `continue`, forward `goto`, `return`.
//! Built-ins: `printf`, `scanf`, `malloc`, `free`, `sizeof`.

pub mod interpreter;
pub mod memory;
pub mod parser;
pub mod snapshot;
pub mod ui;
