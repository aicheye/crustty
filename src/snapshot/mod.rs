// Snapshot management for reverse execution

use crate::memory::{heap::Heap, stack::Stack, value::Value};
use crate::parser::ast::{SourceLocation, Type};
use rustc_hash::FxHashMap;

/// Mock terminal for capturing printf output
#[derive(Debug, Clone)]
pub struct MockTerminal {
    pub lines: Vec<TerminalLine>,
}

impl MockTerminal {
    pub fn new() -> Self {
        MockTerminal { lines: Vec::new() }
    }

    /// Print without newline
    pub fn print(&mut self, text: String, location: SourceLocation) {
        if let Some(last) = self.lines.last_mut() {
            if last.location.line == location.line {
                last.text.push_str(&text);
                return;
            }
        }
        self.lines.push(TerminalLine { text, location });
    }

    /// Get all lines as a vector of strings
    pub fn get_output(&self) -> Vec<String> {
        self.lines
            .iter()
            .flat_map(|tl| {
                // Split by newlines to handle multiple prints from same source line
                let mut result: Vec<String> = tl.text.split('\n').map(|s| s.to_string()).collect();
                // Remove trailing empty string if text ended with newline
                if result.last().is_some_and(|s| s.is_empty()) {
                    result.pop();
                }
                result
            })
            .collect()
    }
}

impl Default for MockTerminal {
    fn default() -> Self {
        Self::new()
    }
}

/// A line of terminal output with source location tracking
#[derive(Debug, Clone)]
pub struct TerminalLine {
    pub text: String,
    pub location: SourceLocation,
}

/// Snapshot of execution state
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub stack: Stack,
    pub heap: Heap,
    pub terminal: MockTerminal,
    pub current_statement_index: usize, // Index into statement list
    pub source_location: SourceLocation,
    pub return_value: Option<Value>,
    pub pointer_types: FxHashMap<u64, Type>,
    pub stack_address_map: FxHashMap<u64, (usize, String)>,
    pub next_stack_address: u64,
    pub execution_depth: usize,
}

impl Snapshot {
    /// Estimate the memory usage of this snapshot in bytes
    pub fn estimated_size(&self) -> usize {
        // This is a rough estimate
        // Stack: assume 100 bytes per frame on average
        let stack_size = self.stack.depth() * 100;

        // Heap: sum of all allocations
        let heap_size = self.heap.total_allocated();

        // Terminal: assume 50 bytes per line on average
        let terminal_size = self.terminal.lines.len() * 50;

        stack_size + heap_size + terminal_size
    }
}

/// Manages execution history for reverse execution
#[derive(Debug)]
pub struct SnapshotManager {
    snapshots: Vec<Snapshot>,
    max_memory: usize,
    current_memory: usize,
}

impl SnapshotManager {
    pub fn new(max_memory: usize) -> Self {
        SnapshotManager {
            snapshots: Vec::new(),
            max_memory,
            current_memory: 0,
        }
    }

    /// Add a snapshot to history
    pub fn push(&mut self, snapshot: Snapshot) -> Result<(), String> {
        let snapshot_size = snapshot.estimated_size();

        if self.current_memory + snapshot_size > self.max_memory {
            return Err(format!(
                "Snapshot memory limit exceeded: {} + {} > {}",
                self.current_memory, snapshot_size, self.max_memory
            ));
        }

        self.current_memory += snapshot_size;
        self.snapshots.push(snapshot);
        Ok(())
    }

    /// Get a snapshot by index
    pub fn get(&self, index: usize) -> Option<&Snapshot> {
        self.snapshots.get(index)
    }

    /// Get the number of snapshots
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Get current memory usage
    pub fn memory_usage(&self) -> usize {
        self.current_memory
    }

    /// Get max memory limit
    pub fn memory_limit(&self) -> usize {
        self.max_memory
    }
}
