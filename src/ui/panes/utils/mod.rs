pub mod formatting;
pub mod memory;
pub mod rendering;

// Re-export common functions to avoid breaking existing code too much if possible,
// but since I am refactoring, I should update call sites.
// However, ui/panes/heap.rs and stack.rs likely use crate::ui::panes::utils::...
// If I use `mod.rs`, then `crate::ui::panes::utils` is the module corresponding to `mod.rs`.
// So `crate::ui::panes::utils::rendering` exists.
// Code using `crate::ui::panes::utils::render_array_elements` will break unless I re-export.

pub(crate) use formatting::*;
pub(crate) use memory::*;
pub(crate) use rendering::*;
