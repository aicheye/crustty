//! TUI pane rendering modules
//!
//! This module provides the rendering logic for all visual panes in the TUI,
//! organized by responsibility for maintainability.
//!
//! # Pane Modules
//!
//! - [`source`]: Source code display with syntax highlighting and current line indicator
//! - [`stack`]: Call stack visualization with local variables and function frames
//! - [`heap`]: Heap memory display with allocation tracking and hex dumps
//! - [`terminal`]: Terminal output from `printf` and other output functions
//! - [`status`]: Status bar with keybindings and execution state
//! - `utils`: Shared utility functions for value formatting and rendering
//!
//! # Architecture
//!
//! Each pane module exports:
//! - A primary `render_*_pane()` function
//! - Associated state types (e.g., `ScrollState`, `RenderData`)
//! - Helper functions specific to that pane
//!
//! The utils module provides shared functionality used across multiple panes,
//! such as value formatting, array/struct rendering, and type annotations.

mod utils;

pub mod heap;
pub mod source;
pub mod stack;
pub mod status;
pub mod terminal;

// Re-export render functions for convenience
pub use heap::{render_heap_pane, HeapRenderData, HeapScrollState};
pub use source::{render_source_pane, SourceRenderData, SourceScrollState};
pub use stack::{render_stack_pane, StackRenderData, StackScrollState};
pub use status::{render_status_bar, StatusRenderData};
pub use terminal::{
    render_terminal_pane, TerminalRenderData, TerminalScrollState,
};
