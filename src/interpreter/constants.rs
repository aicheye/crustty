//! Address-space constants for the simulated C memory model.
//!
//! CRusTTY maps C stack and heap variables to distinct, non-overlapping address
//! ranges so that the TUI can distinguish them at a glance and pointer arithmetic
//! can be validated cheaply.
//!
//! | Region | Base address  | Direction |
//! |--------|---------------|-----------|
//! | Stack  | `0x0000_0004` | grows up (sequential variable IDs) |
//! | Heap   | `0x7fff_0000` | grows up (first-fit allocator) |

/// Starting address for heap allocations.
///
/// Heap block addresses start here and grow upward. The large base value keeps
/// heap and stack address ranges visually distinct in the UI and prevents
/// accidental overlap for programs that don't allocate excessively.
pub const HEAP_ADDRESS_START: u64 = 0x7fff_0000;

/// Starting address for stack variable addresses.
///
/// Stack frame variables are assigned addresses beginning at this value, with
/// each new variable bumping the counter upward within its frame.
pub const STACK_ADDRESS_START: u64 = 0x0000_0004;
