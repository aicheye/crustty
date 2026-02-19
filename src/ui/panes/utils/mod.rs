//! Shared rendering utilities used by the TUI panes.
//!
//! Re-exports everything from three focused submodules so that pane code can write
//! `use crate::ui::panes::utils::*` and access all helpers from a single namespace.
//!
//! | Submodule      | Contents |
//! |----------------|----------|
//! | [`formatting`] | Value/address formatting helpers (hex, decimal, type labels) |
//! | [`memory`]     | Helpers for reading and presenting stack/heap memory data |
//! | [`rendering`]  | Low-level ratatui span/line builders used across panes |

pub mod formatting;
pub mod memory;
pub mod rendering;

pub(crate) use formatting::*;
pub(crate) use memory::*;
pub(crate) use rendering::*;
