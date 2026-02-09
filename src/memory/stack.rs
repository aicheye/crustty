#![allow(dead_code)] // Complete API module, not all methods currently used
//! Call stack implementation
//!
//! This module provides the call stack for function execution:
//! - [`Stack`]: The call stack containing frames
//! - [`StackFrame`]: A single function's activation record
//! - [`LocalVar`]: A local variable with type and initialization state
//! - [`InitState`]: Per-field initialization tracking for structs
//!
//! # Initialization Tracking
//!
//! Local variables track initialization state at a granular level:
//! - Simple types (`int`, `char`): single `Initialized` / `Uninitialized` state
//! - Structs: per-field tracking with [`InitState::PartiallyInitialized`]
//!
//! This enables detection of uninitialized reads even for partially-initialized structs.

use super::value::Value;
use crate::parser::ast::{SourceLocation, Type};
use std::collections::HashMap;

/// Initialization state tracking for variables
#[derive(Debug, Clone, PartialEq)]
pub enum InitState {
    Uninitialized,
    PartiallyInitialized(HashMap<String, InitState>), // For structs: field name -> init state
    Initialized,
}

impl InitState {
    /// Check if fully initialized
    pub fn is_initialized(&self) -> bool {
        matches!(self, InitState::Initialized)
    }

    /// Create initialization state for a struct with given fields
    pub fn for_struct(fields: &[String]) -> Self {
        let mut map = HashMap::new();
        for field in fields {
            map.insert(field.clone(), InitState::Uninitialized);
        }
        InitState::PartiallyInitialized(map)
    }

    /// Mark a field as initialized (for structs)
    pub fn mark_field_initialized(&mut self, field: &str) -> Result<(), String> {
        match self {
            InitState::PartiallyInitialized(map) => {
                if let Some(state) = map.get_mut(field) {
                    *state = InitState::Initialized;
                    // Check if all fields are now initialized
                    if map.values().all(|s| s.is_initialized()) {
                        *self = InitState::Initialized;
                    }
                    Ok(())
                } else {
                    Err(format!("Unknown field: {}", field))
                }
            }
            _ => Err("Not a struct".to_string()),
        }
    }

    /// Check if a specific field is initialized
    pub fn is_field_initialized(&self, field: &str) -> bool {
        match self {
            InitState::PartiallyInitialized(map) => {
                map.get(field).is_some_and(|s| s.is_initialized())
            }
            InitState::Initialized => true,
            InitState::Uninitialized => false,
        }
    }
}

/// Local variable on the stack
#[derive(Debug, Clone)]
pub struct LocalVar {
    pub value: Value,
    pub var_type: Type,
    pub is_const: bool,
    pub init_state: InitState,
    pub address: u64, // Virtual address for this variable
}

impl LocalVar {
    pub fn new(var_type: Type, init_state: InitState, address: u64) -> Self {
        LocalVar {
            value: Value::Uninitialized,
            var_type: var_type.clone(),
            is_const: var_type.is_const,
            init_state,
            address,
        }
    }
}

/// Stack frame for a function call
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub function_name: String,
    pub locals: HashMap<String, LocalVar>,
    pub return_location: Option<SourceLocation>, // Where to return to
    pub insertion_order: Vec<String>,            // Track order of variable declarations
    scope_stack: Vec<ScopeData>,
}

#[derive(Debug, Clone)]
struct ScopeData {
    shadowed: Vec<(String, LocalVar)>,
    declared: Vec<String>,
}

impl StackFrame {
    pub fn new(function_name: String, return_location: Option<SourceLocation>) -> Self {
        StackFrame {
            function_name,
            locals: HashMap::new(),
            return_location,
            insertion_order: Vec::new(),
            scope_stack: Vec::new(),
        }
    }

    /// Enter a new scope
    pub fn push_scope(&mut self) {
        self.scope_stack.push(ScopeData {
            shadowed: Vec::new(),
            declared: Vec::new(),
        });
    }

    /// Exit the current scope
    pub fn pop_scope(&mut self) {
        if let Some(scope) = self.scope_stack.pop() {
            // Remove variables declared in this scope
            for name in scope.declared {
                self.locals.remove(&name);
                if let Some(pos) = self.insertion_order.iter().rposition(|x| x == &name) {
                    self.insertion_order.remove(pos);
                }
            }

            // Restore shadowed variables
            for (name, var) in scope.shadowed {
                self.locals.insert(name, var);
            }
        }
    }

    /// Declare a new local variable
    pub fn declare_var(
        &mut self,
        name: String,
        var_type: Type,
        init_state: InitState, // Passed by value
        address: u64,
    ) {
        let new_var = LocalVar::new(var_type, init_state, address);

        // Handle scoping if we are in a nested scope
        if let Some(scope) = self.scope_stack.last_mut() {
            if let Some(old_var) = self.locals.insert(name.clone(), new_var) {
                // If variable existed, track it as shadowed
                scope.shadowed.push((name, old_var));
                // Don't modify insertion_order as name is already there
            } else {
                // New variable in this scope
                scope.declared.push(name.clone());
                self.insertion_order.push(name);
            }
        } else {
            // Top-level function scope
            if !self.locals.contains_key(&name) {
                self.insertion_order.push(name.clone());
            }
            self.locals.insert(name, new_var);
        }
    }

    /// Get a local variable
    pub fn get_var(&self, name: &str) -> Option<&LocalVar> {
        self.locals.get(name)
    }

    /// Get a mutable reference to a local variable
    pub fn get_var_mut(&mut self, name: &str) -> Option<&mut LocalVar> {
        self.locals.get_mut(name)
    }
}

/// The call stack
#[derive(Debug, Clone)]
pub struct Stack {
    frames: Vec<StackFrame>,
}

impl Stack {
    pub fn new() -> Self {
        Stack { frames: Vec::new() }
    }

    /// Push a new stack frame
    pub fn push_frame(&mut self, function_name: String, return_location: Option<SourceLocation>) {
        self.frames
            .push(StackFrame::new(function_name, return_location));
    }

    /// Pop the top stack frame
    pub fn pop_frame(&mut self) -> Option<StackFrame> {
        self.frames.pop()
    }

    /// Get the current (top) frame
    pub fn current_frame(&self) -> Option<&StackFrame> {
        self.frames.last()
    }

    /// Get a mutable reference to the current frame
    pub fn current_frame_mut(&mut self) -> Option<&mut StackFrame> {
        self.frames.last_mut()
    }

    /// Get all frames (for UI display)
    pub fn frames(&self) -> &[StackFrame] {
        &self.frames
    }

    /// Get the depth of the call stack
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Check if stack is empty
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Get a mutable reference to a specific frame by index
    pub fn frame_mut(&mut self, index: usize) -> Option<&mut StackFrame> {
        self.frames.get_mut(index)
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}
