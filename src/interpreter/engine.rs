//! Core execution engine for the C interpreter
//!
//! This module defines the [`Interpreter`] struct and provides the main execution logic
//! for running C programs from their parsed AST representation.
//!
//! # Responsibilities
//!
//! - Interpreter initialization and configuration
//! - Main execution loop (`run()` and `step()`)
//! - Snapshot management for time-travel debugging
//! - State queries (getters for stack, heap, terminal output, etc.)
//! - Helper functions for value conversion and location tracking
//!
//! # Related Modules
//!
//! The interpreter functionality is distributed across multiple modules:
//! - [`super::statements`]: Statement execution implementation
//! - [`super::expressions`]: Expression evaluation implementation
//! - [`super::builtins`]: Built-in function implementations
//! - [`super::memory_ops`]: Memory operations and struct field access
//! - [`super::type_system`]: Type inference and compatibility

use crate::interpreter::constants::STACK_ADDRESS_START;
use crate::interpreter::errors::RuntimeError;
use crate::memory::{heap::Heap, stack::Stack, value::Value};
use crate::parser::ast::{StructDef as AstStructDef, *};
use crate::snapshot::{MockTerminal, Snapshot, SnapshotManager};
use rustc_hash::FxHashMap;

/// The main interpreter that executes a C program
pub struct Interpreter {
    /// Call stack
    pub(crate) stack: Stack,

    /// Heap memory
    pub(crate) heap: Heap,

    /// Mock terminal for printf output
    pub(crate) terminal: MockTerminal,

    /// Current source location being executed
    pub(crate) current_location: SourceLocation,

    /// Snapshot manager for reverse execution
    pub(crate) snapshot_manager: SnapshotManager,

    /// Current position in execution history (for stepping backward/forward)
    pub(crate) history_position: usize,

    /// Current execution depth (for step over functionality)
    pub(crate) execution_depth: usize,

    /// Struct definitions (name -> StructDef)
    pub(crate) struct_defs: FxHashMap<String, AstStructDef>,

    /// Function definitions (name -> FunctionDef)
    pub(crate) function_defs: FxHashMap<String, FunctionDef>,

    /// Whether execution has finished
    pub(crate) finished: bool,

    /// Whether a break statement was encountered (for loops and switches)
    pub(crate) should_break: bool,

    /// Whether a continue statement was encountered (for loops)
    pub(crate) should_continue: bool,

    /// Mapping from stack addresses to (frame_depth, variable_name)
    pub(crate) stack_address_map: FxHashMap<u64, (usize, String)>,

    /// Next available stack address
    pub(crate) next_stack_address: u64,

    /// Return value from the last function call
    pub(crate) return_value: Option<Value>,

    /// Mapping from heap pointer addresses to their types
    pub(crate) pointer_types: FxHashMap<u64, Type>,

    /// Cache for struct field info: (struct_name, field_name) -> (offset, type)
    pub(crate) field_info_cache: FxHashMap<(String, String), (usize, Type)>,

    /// Last runtime error that occurred during execution (if any)
    pub(crate) last_runtime_error: Option<RuntimeError>,
}

impl Interpreter {
    /// Create a new interpreter with the parsed program
    pub fn new(program: Program, snapshot_memory_limit: usize) -> Self {
        let mut struct_defs = FxHashMap::default();
        let mut function_defs = FxHashMap::default();

        // Index structs and functions for fast lookup
        for node in &program.nodes {
            match node {
                AstNode::StructDef { name, fields, .. } => {
                    struct_defs.insert(
                        name.clone(),
                        AstStructDef {
                            name: name.clone(),
                            fields: fields.clone(),
                        },
                    );
                }
                AstNode::FunctionDef {
                    name,
                    params,
                    body,
                    return_type,
                    location,
                } => {
                    function_defs.insert(
                        name.clone(),
                        FunctionDef {
                            params: params.clone(),
                            body: body.clone(),
                            return_type: return_type.clone(),
                            location: *location,
                        },
                    );
                }
                _ => {}
            }
        }

        Interpreter {
            stack: Stack::new(),
            heap: Heap::default(),
            terminal: MockTerminal::new(),
            current_location: SourceLocation::new(1, 1),
            snapshot_manager: SnapshotManager::new(snapshot_memory_limit),
            history_position: 0,
            execution_depth: 0,
            struct_defs,
            function_defs,
            finished: false,
            should_break: false,
            should_continue: false,
            stack_address_map: FxHashMap::default(),
            next_stack_address: STACK_ADDRESS_START,
            return_value: None,
            pointer_types: FxHashMap::default(),
            field_info_cache: FxHashMap::default(),
            last_runtime_error: None,
        }
    }

    /// Run the program from start to finish
    pub fn run(&mut self) -> Result<(), RuntimeError> {
        // Find main function
        let main_fn = self
            .function_defs
            .get("main")
            .ok_or(RuntimeError::NoMainFunction)?
            .clone();

        // Take initial snapshot
        self.take_snapshot()?;

        // Push initial stack frame for main
        self.stack.push_frame("main".to_string(), None);

        // Execute main function body
        self.current_location = main_fn.location;
        self.take_snapshot()?;

        for stmt in &main_fn.body {
            match self.execute_statement(stmt) {
                Ok(needs_snapshot) => {
                    if !self.finished && needs_snapshot {
                        if let Err(e) = self.take_snapshot() {
                            self.last_runtime_error = Some(e.clone());
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    let _ = self.take_snapshot();
                    self.last_runtime_error = Some(e.clone());
                    return Err(e);
                }
            }
        }

        self.finished = true;
        Ok(())
    }

    /// Execute a single statement
    /// Returns true if a snapshot should be taken after this statement
    pub(crate) fn execute_statement(&mut self, stmt: &AstNode) -> Result<bool, RuntimeError> {
        // Update current location
        if let Some(loc) = Self::get_location(stmt) {
            self.current_location = loc;
        }

        match stmt {
            AstNode::VarDecl {
                name,
                var_type,
                init,
                location,
            } => {
                self.execute_var_decl(name, var_type, init.as_deref(), *location)?;
                Ok(true)
            }

            AstNode::Assignment { lhs, rhs, location } => {
                self.execute_assignment(lhs, rhs, *location)?;
                Ok(true)
            }

            AstNode::CompoundAssignment {
                lhs,
                op,
                rhs,
                location,
            } => {
                self.execute_compound_assignment(lhs, op, rhs, *location)?;
                Ok(true)
            }

            AstNode::Return { expr, location } => {
                self.execute_return(expr.as_deref(), *location)?;
                Ok(false)
            }

            AstNode::If {
                condition,
                then_branch,
                else_branch,
                location,
            } => {
                self.execute_if(
                    condition,
                    then_branch,
                    else_branch.as_ref().map(|v| v.as_slice()),
                    *location,
                )?;
                Ok(false)
            }

            AstNode::While {
                condition,
                body,
                location,
            } => {
                self.execute_while(condition, body, *location)?;
                Ok(false)
            }

            AstNode::DoWhile {
                body,
                condition,
                location,
            } => {
                self.execute_do_while(body, condition, *location)?;
                Ok(false)
            }

            AstNode::For {
                init,
                condition,
                increment,
                body,
                location,
            } => {
                self.execute_for(
                    init.as_deref(),
                    condition.as_deref(),
                    increment.as_deref(),
                    body,
                    *location,
                )?;
                Ok(false)
            }

            AstNode::FunctionCall {
                name,
                args,
                location,
            } => {
                self.execute_function_call(name, args, *location)?;
                Ok(true)
            }

            AstNode::ExpressionStatement { expr, .. } => {
                self.evaluate_expr(expr)?;
                Ok(true)
            }

            AstNode::Break { .. } => {
                self.should_break = true;
                Ok(true)
            }

            AstNode::Continue { .. } => {
                self.should_continue = true;
                Ok(true)
            }

            AstNode::Switch {
                expr,
                cases,
                location,
            } => {
                self.execute_switch(expr, cases, *location)?;
                Ok(false)
            }

            _ => Err(RuntimeError::UnsupportedOperation {
                message: format!("Unexpected statement type: {:?}", stmt),
                location: self.current_location,
            }),
        }
    }

    /// Take a snapshot of the current execution state
    pub(crate) fn take_snapshot(&mut self) -> Result<(), RuntimeError> {
        let snapshot = Snapshot {
            stack: self.stack.clone(),
            heap: self.heap.clone(),
            terminal: self.terminal.clone(),
            current_statement_index: self.history_position,
            source_location: self.current_location,
            return_value: self.return_value.clone(),
            pointer_types: self.pointer_types.clone(),
            stack_address_map: self.stack_address_map.clone(),
            next_stack_address: self.next_stack_address,
            execution_depth: self.execution_depth,
        };

        self.snapshot_manager
            .push(snapshot)
            .map_err(|_| RuntimeError::SnapshotLimitExceeded {
                current: self.snapshot_manager.memory_usage(),
                limit: self.snapshot_manager.memory_limit(),
            })?;

        self.history_position += 1;
        Ok(())
    }

    /// Restore execution state from a snapshot
    fn restore_snapshot(&mut self, snapshot: &Snapshot) {
        self.stack = snapshot.stack.clone();
        self.heap = snapshot.heap.clone();
        self.terminal = snapshot.terminal.clone();
        self.current_location = snapshot.source_location;
        self.history_position = snapshot.current_statement_index;
        self.return_value = snapshot.return_value.clone();
        self.pointer_types = snapshot.pointer_types.clone();
        self.stack_address_map = snapshot.stack_address_map.clone();
        self.next_stack_address = snapshot.next_stack_address;
        self.execution_depth = snapshot.execution_depth;
    }

    /// Step backward in execution (restore previous snapshot)
    pub fn step_backward(&mut self) -> Result<(), RuntimeError> {
        if self.history_position == 0 {
            return Err(RuntimeError::HistoryOperationFailed {
                message: "Already at the beginning of execution".to_string(),
                location: self.current_location,
            });
        }

        self.history_position -= 1;

        if let Some(snapshot) = self.snapshot_manager.get(self.history_position) {
            let snapshot = snapshot.clone();
            self.restore_snapshot(&snapshot);
            Ok(())
        } else {
            Err(RuntimeError::HistoryOperationFailed {
                message: "Snapshot not found in history".to_string(),
                location: self.current_location,
            })
        }
    }

    /// Step forward in execution (restore next snapshot if available)
    pub fn step_forward(&mut self) -> Result<(), RuntimeError> {
        if let Some(snapshot) = self.snapshot_manager.get(self.history_position + 1) {
            self.history_position += 1;
            let snapshot = snapshot.clone();
            self.restore_snapshot(&snapshot);
            Ok(())
        } else if let Some(ref error) = self.last_runtime_error {
            Err(error.clone())
        } else {
            Err(RuntimeError::HistoryOperationFailed {
                message: "Reached end of execution".to_string(),
                location: self.current_location,
            })
        }
    }

    /// Step over: advance until execution depth returns to current level or lower
    pub fn step_over(&mut self) -> Result<(), RuntimeError> {
        let starting_depth = self.execution_depth;

        loop {
            if let Some(snapshot) = self.snapshot_manager.get(self.history_position + 1) {
                self.history_position += 1;
                let snapshot = snapshot.clone();
                self.restore_snapshot(&snapshot);

                if self.execution_depth <= starting_depth {
                    return Ok(());
                }
            } else {
                return Err(RuntimeError::HistoryOperationFailed {
                    message: "Reached end of execution".to_string(),
                    location: self.current_location,
                });
            }
        }
    }

    /// Step backward over: rewind until execution depth returns to current level or lower
    pub fn step_back_over(&mut self) -> Result<(), RuntimeError> {
        let starting_depth = self.execution_depth;

        loop {
            self.step_backward()?;

            if self.execution_depth <= starting_depth {
                return Ok(());
            }
        }
    }

    // ========== Getter methods for UI ==========

    pub fn current_location(&self) -> SourceLocation {
        self.current_location
    }

    pub fn stack(&self) -> &Stack {
        &self.stack
    }

    pub fn heap(&self) -> &Heap {
        &self.heap
    }

    pub fn return_value(&self) -> Option<&Value> {
        self.return_value.as_ref()
    }

    pub fn terminal(&self) -> &MockTerminal {
        &self.terminal
    }

    pub fn pointer_types(&self) -> &FxHashMap<u64, Type> {
        &self.pointer_types
    }

    pub fn history_position(&self) -> usize {
        self.history_position
    }

    pub fn total_snapshots(&self) -> usize {
        self.snapshot_manager.len()
    }

    pub fn struct_defs(&self) -> &FxHashMap<String, AstStructDef> {
        &self.struct_defs
    }

    pub fn function_defs(&self) -> &FxHashMap<String, FunctionDef> {
        &self.function_defs
    }

    /// Rewind to the beginning of execution history
    pub fn rewind_to_start(&mut self) -> Result<(), RuntimeError> {
        if self.snapshot_manager.is_empty() {
            return Err(RuntimeError::HistoryOperationFailed {
                message: "No snapshots available".to_string(),
                location: self.current_location,
            });
        }

        self.history_position = 0;
        if let Some(snapshot) = self.snapshot_manager.get(0).cloned() {
            self.restore_snapshot(&snapshot);
            Ok(())
        } else {
            Err(RuntimeError::HistoryOperationFailed {
                message: "Failed to restore initial snapshot".to_string(),
                location: self.current_location,
            })
        }
    }

    /// Get source location from an AST node
    #[inline]
    pub(crate) fn get_location(node: &AstNode) -> Option<SourceLocation> {
        match node {
            AstNode::IntLiteral(_, loc) => Some(*loc),
            AstNode::StringLiteral(_, loc) => Some(*loc),
            AstNode::Null { location } => Some(*location),
            AstNode::Variable(_, loc) => Some(*loc),
            AstNode::BinaryOp { location, .. } => Some(*location),
            AstNode::UnaryOp { location, .. } => Some(*location),
            AstNode::TernaryOp { location, .. } => Some(*location),
            AstNode::FunctionCall { location, .. } => Some(*location),
            AstNode::VarDecl { location, .. } => Some(*location),
            AstNode::Assignment { location, .. } => Some(*location),
            AstNode::CompoundAssignment { location, .. } => Some(*location),
            AstNode::Return { location, .. } => Some(*location),
            AstNode::If { location, .. } => Some(*location),
            AstNode::While { location, .. } => Some(*location),
            AstNode::DoWhile { location, .. } => Some(*location),
            AstNode::For { location, .. } => Some(*location),
            AstNode::Switch { location, .. } => Some(*location),
            AstNode::Break { location } => Some(*location),
            AstNode::Continue { location } => Some(*location),
            AstNode::Goto { location, .. } => Some(*location),
            AstNode::Label { location, .. } => Some(*location),
            AstNode::ExpressionStatement { location, .. } => Some(*location),
            AstNode::ArrayAccess { location, .. } => Some(*location),
            AstNode::MemberAccess { location, .. } => Some(*location),
            AstNode::PointerMemberAccess { location, .. } => Some(*location),
            AstNode::Cast { location, .. } => Some(*location),
            AstNode::SizeofType { location, .. } => Some(*location),
            AstNode::SizeofExpr { location, .. } => Some(*location),
            _ => None,
        }
    }

    /// Coerce a value to match a target type
    #[inline]
    pub(crate) fn coerce_value_to_type(
        &self,
        value: Value,
        target_type: &Type,
        _location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        if target_type.pointer_depth > 0 || !target_type.array_dims.is_empty() {
            return Ok(value);
        }

        match (&target_type.base, &value) {
            (BaseType::Char, Value::Int(n)) => Ok(Value::Char((*n & 0xFF) as i8)),
            (BaseType::Char, Value::Char(_)) => Ok(value),
            (BaseType::Int, Value::Char(c)) => Ok(Value::Int(*c as i32)),
            (BaseType::Int, Value::Int(_)) => Ok(value),
            _ => Ok(value),
        }
    }

    /// Convert a value to a boolean (for conditionals)
    #[inline]
    pub(crate) fn value_to_bool(
        val: &Value,
        location: SourceLocation,
    ) -> Result<bool, RuntimeError> {
        match val {
            Value::Int(n) => Ok(*n != 0),
            Value::Char(c) => Ok(*c != 0),
            Value::Pointer(_) => Ok(true),
            Value::Null => Ok(false),
            _ => Err(RuntimeError::TypeError {
                expected: "int or pointer".to_string(),
                got: format!("{:?}", val),
                location,
            }),
        }
    }
}

/// Helper struct for function definitions
#[derive(Clone, Debug)]
pub struct FunctionDef {
    pub params: Vec<Param>,
    pub body: Vec<AstNode>,
    pub return_type: Type,
    pub(crate) location: SourceLocation,
}
