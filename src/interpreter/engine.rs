// Execution engine for the C interpreter

use crate::interpreter::errors::RuntimeError;
use crate::memory::{heap::Heap, sizeof_type, stack::Stack, value::Value};
use crate::parser::ast::{StructDef as AstStructDef, *};
use crate::snapshot::{MockTerminal, Snapshot, SnapshotManager};
use std::collections::HashMap;

/// The main interpreter that executes a C program
pub struct Interpreter {
    /// Parsed program (functions, structs, etc.)
    /// Currently unused but retained for future error reporting and debugging features
    #[allow(dead_code)]
    program: Program,

    /// Call stack
    stack: Stack,

    /// Heap memory
    heap: Heap,

    /// Mock terminal for printf output
    terminal: MockTerminal,

    /// Current source location being executed
    current_location: SourceLocation,

    /// Snapshot manager for reverse execution
    snapshot_manager: SnapshotManager,

    /// Current position in execution history (for stepping backward/forward)
    history_position: usize,

    /// Struct definitions (name -> StructDef)
    struct_defs: HashMap<String, AstStructDef>,

    /// Function definitions (name -> FunctionDef)
    function_defs: HashMap<String, FunctionDef>,

    /// Whether execution has finished
    finished: bool,

    /// Whether a break statement was encountered (for loops and switches)
    should_break: bool,

    /// Whether a continue statement was encountered (for loops)
    should_continue: bool,

    /// Mapping from stack addresses to (frame_depth, variable_name)
    /// This allows us to implement the address-of operator for stack variables
    stack_address_map: HashMap<u64, (usize, String)>,

    /// Next available stack address (starts at 0x00000001)
    next_stack_address: u64,

    /// Return value from the last function call
    return_value: Option<Value>,

    /// Mapping from heap pointer addresses to their types
    /// This allows us to know what type a malloc'd pointer points to
    /// Populated when we see casts like (struct Point*)malloc(...)
    pointer_types: HashMap<u64, Type>,
}

impl Interpreter {
    /// Create a new interpreter with the parsed program
    pub fn new(program: Program, snapshot_memory_limit: usize) -> Self {
        let mut struct_defs = HashMap::new();
        let mut function_defs = HashMap::new();

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
                            name: name.clone(),
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
            program,
            stack: Stack::new(),
            heap: Heap::default(), // Use default 10MB heap
            terminal: MockTerminal::new(),
            current_location: SourceLocation::new(1, 1),
            snapshot_manager: SnapshotManager::new(snapshot_memory_limit),
            history_position: 0,
            struct_defs,
            function_defs,
            finished: false,
            should_break: false,
            should_continue: false,
            stack_address_map: HashMap::new(),
            next_stack_address: 0x00000004, // Stack addresses start at 4 for alignment
            return_value: None,
            pointer_types: HashMap::new(),
        }
    }

    /// Run the program from start to finish
    pub fn run(&mut self) -> Result<(), RuntimeError> {
        // Find main function
        let main_fn = self
            .function_defs
            .get("main")
            .ok_or_else(|| RuntimeError::Generic {
                message: "No main() function found".to_string(),
                location: SourceLocation::new(1, 1),
            })?
            .clone();

        // Take initial snapshot
        self.take_snapshot()?;

        // Push initial stack frame for main
        self.stack.push_frame("main".to_string(), None);

        // Execute main function body
        self.current_location = main_fn.location;
        self.take_snapshot()?;

        for stmt in &main_fn.body {
            let needs_snapshot = self.execute_statement(stmt)?;
            if !self.finished && needs_snapshot {
                self.take_snapshot()?;
            }
        }

        self.finished = true;
        Ok(())
    }

    /// Execute a single statement
    /// Returns true if a snapshot should be taken after this statement
    fn execute_statement(&mut self, stmt: &AstNode) -> Result<bool, RuntimeError> {
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
                Ok(true) // Leaf statement - needs snapshot
            }

            AstNode::Assignment { lhs, rhs, location } => {
                self.execute_assignment(lhs, rhs, *location)?;
                Ok(true) // Leaf statement - needs snapshot
            }

            AstNode::CompoundAssignment {
                lhs,
                op,
                rhs,
                location,
            } => {
                self.execute_compound_assignment(lhs, op, rhs, *location)?;
                Ok(true) // Leaf statement - needs snapshot
            }

            AstNode::Return { expr, location } => {
                self.execute_return(expr.as_deref(), *location)?;
                Ok(false) // Return already takes a snapshot
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
                Ok(false) // Control structure - already handles snapshots internally
            }

            AstNode::While {
                condition,
                body,
                location,
            } => {
                self.execute_while(condition, body, *location)?;
                Ok(false) // Control structure - already handles snapshots internally
            }

            AstNode::DoWhile {
                body,
                condition,
                location,
            } => {
                self.execute_do_while(body, condition, *location)?;
                Ok(false) // Control structure - already handles snapshots internally
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
                Ok(false) // Control structure - already handles snapshots internally
            }

            AstNode::FunctionCall {
                name,
                args,
                location,
            } => {
                // Function call as statement (ignore return value)
                self.execute_function_call(name, args, *location)?;
                Ok(true) // Leaf statement - needs snapshot
            }

            AstNode::ExpressionStatement { expr, .. } => {
                // Expression as a statement (e.g., assignments, function calls)
                // Evaluate the expression and discard the result
                self.evaluate_expr(expr)?;
                Ok(true) // Leaf statement - needs snapshot
            }

            AstNode::Break { .. } => {
                self.should_break = true;
                Ok(true) // Leaf statement - needs snapshot
            }

            AstNode::Continue { .. } => {
                self.should_continue = true;
                Ok(true) // Leaf statement - needs snapshot
            }

            AstNode::Switch {
                expr,
                cases,
                location,
            } => {
                self.execute_switch(expr, cases, *location)?;
                Ok(false) // Control structure - already handles snapshots internally
            }

            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("Unexpected statement type: {:?}", stmt),
                    location: self.current_location,
                });
            }
        }
    }

    /// Evaluate an expression and return its value
    fn evaluate_expr(&mut self, expr: &AstNode) -> Result<Value, RuntimeError> {
        let location = Self::get_location(expr).unwrap_or(self.current_location);

        match expr {
            AstNode::IntLiteral(n, _) => Ok(Value::Int(*n)),

            AstNode::CharLiteral(c, _) => Ok(Value::Char(*c)),

            AstNode::StringLiteral(s, loc) => {
                // String literals are stored in the heap
                // Allocate memory for the string + null terminator
                let bytes = s.as_bytes();
                let addr =
                    self.heap
                        .allocate(bytes.len() + 1)
                        .map_err(|_| RuntimeError::OutOfMemory {
                            requested: bytes.len() + 1,
                            limit: self.heap.max_size(),
                        })?;

                // Write string bytes to heap
                for (i, &byte) in bytes.iter().enumerate() {
                    self.heap.write_byte(addr + i as u64, byte).map_err(|e| {
                        RuntimeError::Generic {
                            message: e,
                            location: *loc,
                        }
                    })?;
                }
                // Null terminator
                self.heap
                    .write_byte(addr + bytes.len() as u64, 0)
                    .map_err(|e| RuntimeError::Generic {
                        message: e,
                        location: *loc,
                    })?;

                Ok(Value::Pointer(addr))
            }

            AstNode::Null { .. } => Ok(Value::Null),

            AstNode::Variable(name, loc) => {
                // Look up variable in current frame
                let frame = self
                    .stack
                    .current_frame()
                    .ok_or_else(|| RuntimeError::Generic {
                        message: "No stack frame".to_string(),
                        location: *loc,
                    })?;

                let var = frame
                    .get_var(name)
                    .ok_or_else(|| RuntimeError::UndefinedVariable {
                        name: name.clone(),
                        location: *loc,
                    })?;

                // Check if initialized
                if !var.init_state.is_initialized() {
                    return Err(RuntimeError::UninitializedRead {
                        var: name.clone(),
                        location: *loc,
                    });
                }

                // Implement pointer decay for arrays
                // In most contexts, arrays decay to pointers to their first element
                // This does NOT happen in sizeof() or & operator (handled elsewhere)
                if !var.var_type.array_dims.is_empty() {
                    // Array decays to pointer to first element
                    Ok(Value::Pointer(var.address))
                } else {
                    Ok(var.value.clone())
                }
            }

            AstNode::BinaryOp {
                op,
                left,
                right,
                location,
            } => self.evaluate_binary_op(op, left, right, *location),

            AstNode::UnaryOp {
                op,
                operand,
                location,
            } => self.evaluate_unary_op(op, operand, *location),

            AstNode::TernaryOp {
                condition,
                true_expr,
                false_expr,
                location,
            } => {
                let cond_val = self.evaluate_expr(condition)?;
                let cond_int = Self::value_to_bool(&cond_val, *location)?;

                if cond_int {
                    self.evaluate_expr(true_expr)
                } else {
                    self.evaluate_expr(false_expr)
                }
            }

            AstNode::FunctionCall {
                name,
                args,
                location,
            } => self.execute_function_call(name, args, *location),

            AstNode::Cast {
                target_type,
                expr,
                location: _,
            } => {
                let val = self.evaluate_expr(expr)?;

                // If we're casting a pointer to a struct pointer type, track the type
                if let Value::Pointer(addr) = val {
                    if target_type.pointer_depth > 0 {
                        // Store the pointee type (the type being pointed to)
                        let mut pointee_type = target_type.clone();
                        pointee_type.pointer_depth -= 1; // Remove one level of pointer
                        self.pointer_types.insert(addr, pointee_type);
                    }
                }

                Ok(val)
            }

            AstNode::MemberAccess {
                object,
                member,
                location,
            } => {
                // Access a struct field using . operator
                let obj_val = self.evaluate_expr(object)?;

                match obj_val {
                    Value::Struct(fields) => {
                        fields
                            .get(member)
                            .cloned()
                            .ok_or_else(|| RuntimeError::Generic {
                                message: format!("Struct does not have field '{}'", member),
                                location: *location,
                            })
                    }
                    _ => Err(RuntimeError::TypeError {
                        expected: "struct".to_string(),
                        got: format!("{:?}", obj_val),
                        location: *location,
                    }),
                }
            }

            AstNode::PointerMemberAccess {
                object,
                member,
                location,
            } => {
                // Access a struct field using -> operator (pointer dereference + member access)
                let ptr_val = self.evaluate_expr(object)?;

                match ptr_val {
                    Value::Pointer(addr) => {
                        // Check if this is a stack address or heap address
                        if addr < 0x10000000 {
                            // Stack address - look up in map
                            let (frame_depth, var_name) = self
                                .stack_address_map
                                .get(&addr)
                                .ok_or_else(|| RuntimeError::Generic {
                                    message: format!("Invalid stack pointer: 0x{:x}", addr),
                                    location: *location,
                                })?
                                .clone();

                            // Get the variable from the appropriate frame
                            let frame = self.stack.frames().get(frame_depth).ok_or_else(|| {
                                RuntimeError::Generic {
                                    message: "Invalid frame depth".to_string(),
                                    location: *location,
                                }
                            })?;

                            let var = frame.get_var(&var_name).ok_or_else(|| {
                                RuntimeError::UndefinedVariable {
                                    name: var_name.clone(),
                                    location: *location,
                                }
                            })?;

                            // The variable should be a struct
                            match &var.value {
                                Value::Struct(fields) => {
                                    fields.get(member).cloned().ok_or_else(|| {
                                        RuntimeError::Generic {
                                            message: format!(
                                                "Struct does not have field '{}'",
                                                member
                                            ),
                                            location: *location,
                                        }
                                    })
                                }
                                _ => Err(RuntimeError::TypeError {
                                    expected: "struct".to_string(),
                                    got: format!("{:?}", var.value),
                                    location: *location,
                                }),
                            }
                        } else {
                            // Heap address - read struct from heap
                            // Look up the pointer type
                            let pointee_type = self.pointer_types.get(&addr)
                                .ok_or_else(|| RuntimeError::Generic {
                                    message: format!("Unknown type for pointer 0x{:x}. Did you cast the result of malloc?", addr),
                                    location: *location,
                                })?;

                            // Ensure it's a struct type
                            let struct_name = match &pointee_type.base {
                                BaseType::Struct(name) => name.clone(),
                                _ => {
                                    return Err(RuntimeError::TypeError {
                                        expected: "struct pointer".to_string(),
                                        got: format!("{:?}", pointee_type),
                                        location: *location,
                                    });
                                }
                            };

                            // Calculate field offset
                            let offset =
                                self.calculate_field_offset(&struct_name, member, *location)?;

                            // Get field type
                            let field_type =
                                self.get_field_type(&struct_name, member, *location)?;

                            // Deserialize field value from heap
                            self.deserialize_value_from_heap(
                                &field_type,
                                addr + offset as u64,
                                *location,
                            )
                        }
                    }
                    Value::Null => Err(RuntimeError::NullDereference {
                        location: *location,
                    }),
                    _ => Err(RuntimeError::TypeError {
                        expected: "pointer".to_string(),
                        got: format!("{:?}", ptr_val),
                        location: *location,
                    }),
                }
            }

            AstNode::ArrayAccess {
                array,
                index,
                location,
            } => {
                let arr_val = self.evaluate_expr(array)?;
                let idx_val = self.evaluate_expr(index)?;

                let idx = match idx_val {
                    Value::Int(i) => i,
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "int".to_string(),
                            got: format!("{:?}", idx_val),
                            location: *location,
                        });
                    }
                };

                match arr_val {
                    Value::Array(elements) => {
                        if idx < 0 || idx as usize >= elements.len() {
                            return Err(RuntimeError::BufferOverrun {
                                index: idx as usize,
                                size: elements.len(),
                                location: *location,
                            });
                        }
                        Ok(elements[idx as usize].clone())
                    }
                    Value::Pointer(addr) => {
                        if addr == 0 {
                            return Err(RuntimeError::NullDereference {
                                location: *location,
                            });
                        }

                        // Check if this is a stack address
                        if addr < 0x10000000 {
                            // Stack pointer indexing (for decayed arrays)
                            let (frame_depth, var_name) = self
                                .stack_address_map
                                .get(&addr)
                                .ok_or_else(|| RuntimeError::Generic {
                                    message: format!("Invalid stack pointer: 0x{:x}", addr),
                                    location: *location,
                                })?
                                .clone();

                            let frame = self.stack.frames().get(frame_depth).ok_or_else(|| {
                                RuntimeError::Generic {
                                    message: "Invalid frame depth".to_string(),
                                    location: *location,
                                }
                            })?;

                            let var = frame.get_var(&var_name).ok_or_else(|| {
                                RuntimeError::UndefinedVariable {
                                    name: var_name.clone(),
                                    location: *location,
                                }
                            })?;

                            // The variable should contain an array
                            match &var.value {
                                Value::Array(elements) => {
                                    if idx < 0 || idx as usize >= elements.len() {
                                        return Err(RuntimeError::BufferOverrun {
                                            index: idx as usize,
                                            size: elements.len(),
                                            location: *location,
                                        });
                                    }
                                    Ok(elements[idx as usize].clone())
                                }
                                _ => {
                                    // Not an array - treat as single element at index 0
                                    if idx == 0 {
                                        Ok(var.value.clone())
                                    } else {
                                        Err(RuntimeError::Generic {
                                            message: format!(
                                                "Pointer to non-array stack variable, index {} out of bounds",
                                                idx
                                            ),
                                            location: *location,
                                        })
                                    }
                                }
                            }
                        } else {
                            // Heap pointer indexing
                            if let Some(elem_type) = self.pointer_types.get(&addr).cloned() {
                                let elem_size = sizeof_type(&elem_type, &self.struct_defs);

                                let offset = (idx as i64) * (elem_size as i64);
                                let target_addr = if offset >= 0 {
                                    addr + (offset as u64)
                                } else {
                                    addr - ((-offset) as u64)
                                };

                                self.deserialize_value_from_heap(&elem_type, target_addr, *location)
                            } else {
                                Err(RuntimeError::Generic {
                                    message: format!(
                                        "Unknown pointer type for indexing at 0x{:x}",
                                        addr
                                    ),
                                    location: *location,
                                })
                            }
                        }
                    }
                    _ => Err(RuntimeError::TypeError {
                        expected: "array or pointer".to_string(),
                        got: format!("{:?}", arr_val),
                        location: *location,
                    }),
                }
            }

            AstNode::Assignment { lhs, rhs, location } => {
                // Assignment as an expression (returns the assigned value)
                let value = self.evaluate_expr(rhs)?;
                self.assign_to_lvalue(lhs, value.clone(), *location)?;
                Ok(value)
            }

            AstNode::SizeofType {
                target_type,
                location: _,
            } => {
                let size = sizeof_type(target_type, &self.struct_defs);
                Ok(Value::Int(size as i32))
            }

            AstNode::SizeofExpr { expr, location } => {
                // For sizeof(expr), infer the type of the expression
                // Important: sizeof does NOT evaluate the expression, only its type
                let expr_type = self.infer_expr_type(expr)?;
                let size = sizeof_type(&expr_type, &self.struct_defs);
                Ok(Value::Int(size as i32))
            }

            AstNode::CompoundAssignment {
                lhs,
                op,
                rhs,
                location,
            } => {
                // Compound assignment as an expression
                // Evaluate it and return the assigned value
                self.execute_compound_assignment(lhs, op, rhs, *location)?;
                // Return the new value of the lhs
                self.evaluate_expr(lhs)
            }

            _ => Err(RuntimeError::Generic {
                message: format!("Cannot evaluate expression: {:?}", expr),
                location,
            }),
        }
    }

    /// Take a snapshot of the current execution state
    fn take_snapshot(&mut self) -> Result<(), RuntimeError> {
        let snapshot = Snapshot::new(
            self.stack.clone(),
            self.heap.clone(),
            self.terminal.clone(),
            self.history_position,
            self.current_location,
            self.return_value.clone(),
            self.pointer_types.clone(),
            self.stack_address_map.clone(),
            self.next_stack_address,
        );

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
    }

    /// Step backward in execution (restore previous snapshot)
    pub fn step_backward(&mut self) -> Result<(), RuntimeError> {
        if self.history_position == 0 {
            return Err(RuntimeError::Generic {
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
            Err(RuntimeError::Generic {
                message: "Snapshot not found in history".to_string(),
                location: self.current_location,
            })
        }
    }

    /// Step forward in execution (restore next snapshot if available, or execute next statement)
    pub fn step_forward(&mut self) -> Result<(), RuntimeError> {
        // Check if we have a snapshot ahead in history
        if let Some(snapshot) = self.snapshot_manager.get(self.history_position + 1) {
            // Replay from history
            self.history_position += 1;
            let snapshot = snapshot.clone();
            self.restore_snapshot(&snapshot);
            Ok(())
        } else {
            // No snapshot ahead - we need to execute new code
            // This would require tracking which statement to execute next
            // For now, return an error indicating we've reached the end
            Err(RuntimeError::Generic {
                message: "No more snapshots available (execution finished)".to_string(),
                location: self.current_location,
            })
        }
    }

    // ========== Getter methods for UI ==========

    /// Get the current source location
    pub fn current_location(&self) -> SourceLocation {
        self.current_location
    }

    /// Get a reference to the stack
    pub fn stack(&self) -> &Stack {
        &self.stack
    }

    /// Get a reference to the heap
    pub fn heap(&self) -> &Heap {
        &self.heap
    }

    /// Get the current return value (set when at a return statement)
    pub fn return_value(&self) -> Option<&Value> {
        self.return_value.as_ref()
    }

    /// Get a reference to the terminal output
    pub fn terminal(&self) -> &MockTerminal {
        &self.terminal
    }

    /// Get pointer type mappings for heap allocations
    pub fn pointer_types(&self) -> &HashMap<u64, Type> {
        &self.pointer_types
    }

    /// Get the current history position
    pub fn history_position(&self) -> usize {
        self.history_position
    }

    /// Get the total number of snapshots
    pub fn total_snapshots(&self) -> usize {
        self.snapshot_manager.len()
    }

    /// Check if execution has finished
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Get struct definitions (for type information in UI)
    pub fn struct_defs(&self) -> &HashMap<String, AstStructDef> {
        &self.struct_defs
    }

    /// Get function definitions
    pub fn function_defs(&self) -> &HashMap<String, FunctionDef> {
        &self.function_defs
    }

    /// Rewind to the beginning of execution history
    pub fn rewind_to_start(&mut self) -> Result<(), RuntimeError> {
        if self.snapshot_manager.len() == 0 {
            return Err(RuntimeError::Generic {
                message: "No snapshots available".to_string(),
                location: self.current_location.clone(),
            });
        }

        self.history_position = 0;
        // Clone the snapshot to avoid borrow checker issues
        if let Some(snapshot) = self.snapshot_manager.get(0).cloned() {
            self.restore_snapshot(&snapshot);
            Ok(())
        } else {
            Err(RuntimeError::Generic {
                message: "Failed to restore initial snapshot".to_string(),
                location: self.current_location.clone(),
            })
        }
    }

    /// Get source location from an AST node
    fn get_location(node: &AstNode) -> Option<SourceLocation> {
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
    /// Performs implicit conversions like int -> char
    fn coerce_value_to_type(
        &self,
        value: Value,
        target_type: &Type,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        // If target is a pointer or array, no coercion needed
        if target_type.pointer_depth > 0 || !target_type.array_dims.is_empty() {
            return Ok(value);
        }

        // Perform type coercion based on target base type
        match (&target_type.base, &value) {
            // Char target type
            (BaseType::Char, Value::Int(n)) => {
                // Convert int to char by taking lower 8 bits
                Ok(Value::Char((*n & 0xFF) as i8))
            }
            (BaseType::Char, Value::Char(_)) => Ok(value),

            // Int target type
            (BaseType::Int, Value::Char(c)) => {
                // Convert char to int by zero-extending
                Ok(Value::Int(*c as i32))
            }
            (BaseType::Int, Value::Int(_)) => Ok(value),

            // No coercion needed for matching types
            _ => Ok(value),
        }
    }

    /// Convert a value to a boolean (for conditionals)
    fn value_to_bool(val: &Value, location: SourceLocation) -> Result<bool, RuntimeError> {
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
/// Stores complete function metadata for execution and future type checking
#[derive(Clone, Debug)]
pub struct FunctionDef {
    /// Function name (retained for stack traces and error messages)
    #[allow(dead_code)]
    pub name: String,
    /// Function parameters
    pub params: Vec<Param>,
    /// Function body statements
    pub body: Vec<AstNode>,
    /// Return type (retained for future type checking of return values)
    pub return_type: Type,
    /// Function definition location
    location: SourceLocation,
}

// Statement execution methods
impl Interpreter {
    fn execute_var_decl(
        &mut self,
        name: &str,
        var_type: &Type,
        init: Option<&AstNode>,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Check that we have a stack frame
        if self.stack.current_frame().is_none() {
            return Err(RuntimeError::Generic {
                message: "No stack frame".to_string(),
                location,
            });
        }

        // Determine initialization state and value
        let (init_state, value) = if let Some(init_expr) = init {
            // Evaluate the initializer
            let val = self.evaluate_expr(init_expr)?;
            // Coerce the value to match the declared type
            let coerced_val = self.coerce_value_to_type(val, var_type, location)?;
            (
                crate::memory::stack::InitState::Initialized,
                Some(coerced_val),
            )
        } else {
            // No initializer
            // Check if this is an array type first
            if !var_type.array_dims.is_empty() {
                // Initialize array with default values
                let size = var_type.array_dims[0].unwrap_or(0);
                let default_element = match &var_type.base {
                    BaseType::Int => Value::Int(0),
                    BaseType::Char => Value::Char(0),
                    BaseType::Void => Value::Uninitialized,
                    BaseType::Struct(_) => Value::Struct(std::collections::HashMap::new()),
                };
                let elements: Vec<Value> = (0..size).map(|_| default_element.clone()).collect();
                (
                    crate::memory::stack::InitState::Initialized,
                    Some(Value::Array(elements)),
                )
            } else {
                // For structs, create a default struct value with all fields initialized
                match &var_type.base {
                    BaseType::Struct(struct_name) => {
                        // Helper function to create default value for a type
                        fn create_default_value(
                            type_: &Type,
                            struct_defs: &std::collections::HashMap<String, StructDef>,
                        ) -> Value {
                            if !type_.array_dims.is_empty() {
                                let size = type_.array_dims[0].unwrap_or(0);
                                let element_type = Type {
                                    base: type_.base.clone(),
                                    is_const: type_.is_const,
                                    pointer_depth: type_.pointer_depth,
                                    array_dims: type_.array_dims[1..].to_vec(),
                                };
                                let default_element =
                                    create_default_value(&element_type, struct_defs);
                                let elements: Vec<Value> =
                                    (0..size).map(|_| default_element.clone()).collect();
                                return Value::Array(elements);
                            }

                            if type_.pointer_depth > 0 {
                                return Value::Pointer(0); // Null pointer
                            }

                            match &type_.base {
                                BaseType::Int => Value::Int(0),
                                BaseType::Char => Value::Char(0),
                                BaseType::Void => Value::Uninitialized,
                                BaseType::Struct(name) => {
                                    let mut fields = std::collections::HashMap::new();
                                    if let Some(def) = struct_defs.get(name) {
                                        for field in &def.fields {
                                            fields.insert(
                                                field.name.clone(),
                                                create_default_value(
                                                    &field.field_type,
                                                    struct_defs,
                                                ),
                                            );
                                        }
                                    }
                                    Value::Struct(fields)
                                }
                            }
                        }

                        let default_struct = create_default_value(var_type, &self.struct_defs);

                        // Mark as initialized
                        (
                            crate::memory::stack::InitState::Initialized,
                            Some(default_struct),
                        )
                    }
                    _ => {
                        // For other types, leave uninitialized
                        (crate::memory::stack::InitState::Uninitialized, None)
                    }
                }
            }
        };

        // Allocate a virtual address for this variable
        let address = self.next_stack_address;
        let var_size = sizeof_type(&var_type, &self.struct_defs) as u64;
        self.next_stack_address += var_size;

        // Store in address map
        let frame_depth = self.stack.depth() - 1;
        self.stack_address_map
            .insert(address, (frame_depth, name.to_string()));

        // Now declare the variable
        let frame = self.stack.current_frame_mut().unwrap();
        frame.declare_var(name.to_string(), var_type.clone(), init_state, address);

        // Set the value if we have one
        if let Some(val) = value {
            let var = frame.get_var_mut(name).unwrap();
            var.value = val.clone();

            // If this is a pointer variable with an initializer, track its type
            // This allows us to know what type a malloc'd pointer points to
            if var_type.pointer_depth > 0 {
                if let Some(addr) = val.as_pointer() {
                    if addr != 0 {
                        // Remove one level of indirection to get the pointed-to type
                        let pointed_to_type = Type {
                            base: var_type.base.clone(),
                            is_const: var_type.is_const,
                            pointer_depth: var_type.pointer_depth - 1,
                            array_dims: var_type.array_dims.clone(),
                        };
                        self.pointer_types.insert(addr, pointed_to_type);
                    }
                }
            }
        }

        Ok(())
    }

    fn execute_assignment(
        &mut self,
        lhs: &AstNode,
        rhs: &AstNode,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        let value = self.evaluate_expr(rhs)?;
        self.assign_to_lvalue(lhs, value, location)
    }

    fn execute_compound_assignment(
        &mut self,
        lhs: &AstNode,
        op: &BinOp,
        rhs: &AstNode,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Evaluate the right-hand side
        let rhs_val = self.evaluate_expr(rhs)?;

        // Get the current value of the left-hand side
        let lhs_val = self.evaluate_expr(lhs)?;

        // Apply the compound operation
        let result_val = match op {
            BinOp::AddAssign => self.checked_add_values(&lhs_val, &rhs_val, location)?,
            BinOp::SubAssign => self.checked_sub_values(&lhs_val, &rhs_val, location)?,
            BinOp::MulAssign => self.checked_mul_values(&lhs_val, &rhs_val, location)?,
            BinOp::DivAssign => self.checked_div_values(&lhs_val, &rhs_val, location)?,
            BinOp::ModAssign => self.checked_mod_values(&lhs_val, &rhs_val, location)?,
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!("Unsupported compound assignment operator: {:?}", op),
                    location,
                });
            }
        };

        // Assign the result back to the left-hand side
        self.assign_to_lvalue(lhs, result_val, location)
    }

    fn execute_return(
        &mut self,
        expr: Option<&AstNode>,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Evaluate the return expression
        if let Some(ret_expr) = expr {
            let return_val = self.evaluate_expr(ret_expr)?;
            self.return_value = Some(return_val);
        } else {
            self.return_value = None;
        }

        // Snapshot at return statement
        self.current_location = location;
        self.take_snapshot()?;

        // Mark execution as finished
        self.finished = true;
        Ok(())
    }

    fn execute_if(
        &mut self,
        condition: &AstNode,
        then_branch: &[AstNode],
        else_branch: Option<&[AstNode]>,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Take snapshot at if statement to show control structure entry
        self.current_location = location;
        self.take_snapshot()?;

        let cond_val = self.evaluate_expr(condition)?;
        let cond_bool = Self::value_to_bool(&cond_val, location)?;

        if cond_bool {
            for stmt in then_branch {
                let needs_snapshot = self.execute_statement(stmt)?;
                if self.finished {
                    return Ok(());
                }
                // Take snapshot after each statement for time-travel debugging
                if needs_snapshot {
                    self.take_snapshot()?;
                }
            }
        } else if let Some(else_stmts) = else_branch {
            for stmt in else_stmts {
                let needs_snapshot = self.execute_statement(stmt)?;
                if self.finished {
                    return Ok(());
                }
                // Take snapshot after each statement for time-travel debugging
                if needs_snapshot {
                    self.take_snapshot()?;
                }
            }
        }

        Ok(())
    }

    fn execute_while(
        &mut self,
        condition: &AstNode,
        body: &[AstNode],
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        loop {
            // Check condition first
            let cond_val = self.evaluate_expr(condition)?;
            let cond_bool = Self::value_to_bool(&cond_val, location)?;

            if !cond_bool {
                // Take snapshot at loop exit to show condition failed
                self.current_location = location;
                self.take_snapshot()?;
                break;
            }

            // Take snapshot when entering the loop body
            self.current_location = location;
            self.take_snapshot()?;

            for stmt in body {
                let needs_snapshot = self.execute_statement(stmt)?;
                if self.finished {
                    return Ok(());
                }
                // Check for break or continue
                if self.should_break {
                    self.should_break = false; // Reset the flag
                    return Ok(()); // Exit the loop
                }
                if self.should_continue {
                    self.should_continue = false; // Reset the flag
                    break; // Continue to next iteration
                }
                // Take snapshot after each statement for time-travel debugging
                if needs_snapshot {
                    self.take_snapshot()?;
                }
            }
        }

        Ok(())
    }

    fn execute_do_while(
        &mut self,
        body: &[AstNode],
        condition: &AstNode,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        loop {
            // Snapshot at start of iteration
            self.current_location = location;
            self.take_snapshot()?;

            for stmt in body {
                let needs_snapshot = self.execute_statement(stmt)?;
                if self.finished {
                    return Ok(());
                }
                // Check for break or continue
                if self.should_break {
                    self.should_break = false; // Reset the flag
                    return Ok(()); // Exit the loop
                }
                if self.should_continue {
                    self.should_continue = false; // Reset the flag
                    break; // Continue to next iteration
                }
                // Take snapshot after each statement for time-travel debugging
                if needs_snapshot {
                    self.take_snapshot()?;
                }
            }

            let cond_val = self.evaluate_expr(condition)?;
            let cond_bool = Self::value_to_bool(&cond_val, location)?;

            if !cond_bool {
                // Take snapshot at loop exit to show condition failed
                self.current_location = location;
                self.take_snapshot()?;
                break;
            }
        }

        Ok(())
    }

    fn execute_for(
        &mut self,
        init: Option<&AstNode>,
        condition: Option<&AstNode>,
        increment: Option<&AstNode>,
        body: &[AstNode],
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Execute initializer
        if let Some(init_stmt) = init {
            let _needs_snapshot = self.execute_statement(init_stmt)?;
            // Note: for loop init doesn't need a snapshot, the loop header will take one
        }

        loop {
            // Check condition first
            if let Some(cond) = condition {
                let cond_val = self.evaluate_expr(cond)?;
                let cond_bool = Self::value_to_bool(&cond_val, location)?;

                if !cond_bool {
                    // Take snapshot at loop exit to show condition failed
                    self.current_location = location;
                    self.take_snapshot()?;
                    break;
                }
            }

            // Take snapshot when entering the loop body
            self.current_location = location;
            self.take_snapshot()?;

            // Execute body
            for stmt in body {
                let needs_snapshot = self.execute_statement(stmt)?;
                if self.finished {
                    return Ok(());
                }
                // Check for break or continue
                if self.should_break {
                    self.should_break = false; // Reset the flag
                    return Ok(()); // Exit the loop
                }
                if self.should_continue {
                    self.should_continue = false; // Reset the flag
                    break; // Continue to next iteration
                }
                // Take snapshot after each statement for time-travel debugging
                if needs_snapshot {
                    self.take_snapshot()?;
                }
            }

            // Execute increment
            if let Some(inc) = increment {
                self.evaluate_expr(inc)?;
            }
        }

        Ok(())
    }

    fn execute_switch(
        &mut self,
        expr: &AstNode,
        cases: &[CaseNode],
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        use crate::parser::ast::CaseNode;

        // Take snapshot at switch entry
        self.current_location = location;
        self.take_snapshot()?;

        // Evaluate the switch expression
        let switch_val = self.evaluate_expr(expr)?;

        // Find the matching case (or default)
        let mut match_index: Option<usize> = None;
        let mut default_index: Option<usize> = None;

        for (i, case) in cases.iter().enumerate() {
            match case {
                CaseNode::Case { value, .. } => {
                    let case_val = self.evaluate_expr(value)?;
                    // Compare values for equality
                    if self.values_equal(&switch_val, &case_val) {
                        match_index = Some(i);
                        break;
                    }
                }
                CaseNode::Default { .. } => {
                    default_index = Some(i);
                }
            }
        }

        // Determine where to start execution
        let start_index = match_index.or(default_index);

        if let Some(start) = start_index {
            // Execute from the matching case onwards (fall-through behavior)
            for case in &cases[start..] {
                // Take snapshot when entering each case (including first matched case)
                let case_location = match case {
                    CaseNode::Case { location, .. } => *location,
                    CaseNode::Default { location, .. } => *location,
                };
                self.current_location = case_location;
                self.take_snapshot()?;

                let statements = match case {
                    CaseNode::Case { statements, .. } => statements,
                    CaseNode::Default { statements, .. } => statements,
                };

                for stmt in statements {
                    let needs_snapshot = self.execute_statement(stmt)?;
                    if self.finished {
                        return Ok(());
                    }

                    // Take snapshot after statement if needed (before checking control flow)
                    if needs_snapshot {
                        self.take_snapshot()?;
                    }

                    // Check for break - exit switch
                    if self.should_break {
                        self.should_break = false; // Reset the flag
                        return Ok(());
                    }
                    // Continue shouldn't be used in switch outside of loops
                    // but if it happens, let it propagate up
                    if self.should_continue {
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }

    /// Compare two values for equality (used by switch statement)
    fn values_equal(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Pointer(a), Value::Pointer(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Null, Value::Pointer(0)) | (Value::Pointer(0), Value::Null) => true,
            _ => false,
        }
    }

    fn execute_function_call(
        &mut self,
        name: &str,
        args: &[AstNode],
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        // Check if it's a built-in function
        match name {
            "printf" => self.builtin_printf(args, location),
            "malloc" => self.builtin_malloc(args, location),
            "free" => self.builtin_free(args, location),
            _ => {
                // User-defined function
                self.call_user_function(name, args, location)
            }
        }
    }

    fn call_user_function(
        &mut self,
        name: &str,
        args: &[AstNode],
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        // Snapshot at call site (before call)
        self.current_location = location;
        self.take_snapshot()?;

        // Look up the function definition
        let func_def = self.function_defs.get(name).cloned().ok_or_else(|| {
            RuntimeError::UndefinedFunction {
                name: name.to_string(),
                location,
            }
        })?;

        // Check argument count
        if args.len() != func_def.params.len() {
            return Err(RuntimeError::Generic {
                message: format!(
                    "Function {} expects {} arguments, got {}",
                    name,
                    func_def.params.len(),
                    args.len()
                ),
                location,
            });
        }

        // Evaluate all arguments
        let mut arg_values = Vec::new();
        for arg in args {
            arg_values.push(self.evaluate_expr(arg)?);
        }

        // Push a new stack frame
        self.stack.push_frame(name.to_string(), Some(location));

        // Bind parameters to arguments
        for (param, value) in func_def.params.iter().zip(arg_values.iter()) {
            // Allocate a virtual address for this parameter
            let address = self.next_stack_address;
            self.next_stack_address += 1;

            // Store in address map
            let frame_depth = self.stack.depth() - 1;
            self.stack_address_map
                .insert(address, (frame_depth, param.name.clone()));

            let frame = self.stack.current_frame_mut().unwrap();
            frame.declare_var(
                param.name.clone(),
                param.param_type.clone(),
                crate::memory::stack::InitState::Initialized,
                address,
            );

            let var = frame.get_var_mut(&param.name).unwrap();
            var.value = value.clone();
        }

        // Execute function body
        let saved_finished = self.finished;
        let saved_return_value = self.return_value.clone();
        self.finished = false;
        self.return_value = None;

        // Take a snapshot at the function entry (signature)
        self.current_location = func_def.location;
        self.take_snapshot()?;

        for stmt in &func_def.body {
            let needs_snapshot = self.execute_statement(stmt)?;

            // Check if we hit a return statement
            if self.finished {
                break;
            }

            // Take snapshot after each statement (for time-travel debugging)
            if needs_snapshot {
                self.take_snapshot()?;
            }
        }

        // Capture the return value
        let return_val = self.return_value.clone().unwrap_or(Value::Int(0));

        // Pop the stack frame
        self.stack.pop_frame();

        // Restore previous state
        self.finished = saved_finished;
        self.return_value = saved_return_value;

        // Snapshot at call site (after return) removed to avoid duplicate since caller loop will snapshot
        self.current_location = location;
        // self.take_snapshot()?;

        // Return the value
        Ok(return_val)
    }

    // Built-in functions
    fn builtin_printf(
        &mut self,
        args: &[AstNode],
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        if args.is_empty() {
            return Err(RuntimeError::InvalidPrintfFormat {
                message: "printf requires at least one argument".to_string(),
                location,
            });
        }

        // First argument must be a string literal (format string)
        let format_str = match &args[0] {
            AstNode::StringLiteral(s, _) => s.clone(),
            _ => {
                return Err(RuntimeError::InvalidPrintfFormat {
                    message: "printf format must be a string literal".to_string(),
                    location,
                });
            }
        };

        // Evaluate remaining arguments
        let mut arg_values = Vec::new();
        for arg in &args[1..] {
            arg_values.push(self.evaluate_expr(arg)?);
        }

        // Parse format string and generate output
        let output = self.format_printf(&format_str, &arg_values, location)?;

        // Add to mock terminal
        self.terminal.print(output, self.current_location);

        // printf returns the number of characters printed
        // For simplicity, return 0
        Ok(Value::Int(0))
    }

    fn format_printf(
        &self,
        format: &str,
        args: &[Value],
        location: SourceLocation,
    ) -> Result<String, RuntimeError> {
        let mut output = String::new();
        let mut chars = format.chars().peekable();
        let mut arg_index = 0;

        while let Some(ch) = chars.next() {
            if ch == '%' {
                if let Some(&next_ch) = chars.peek() {
                    chars.next(); // consume the format specifier

                    match next_ch {
                        '%' => output.push('%'),
                        'd' => {
                            if arg_index >= args.len() {
                                return Err(RuntimeError::InvalidPrintfFormat {
                                    message: "Not enough arguments for format string".to_string(),
                                    location,
                                });
                            }
                            match &args[arg_index] {
                                Value::Int(n) => output.push_str(&n.to_string()),
                                _ => {
                                    return Err(RuntimeError::InvalidPrintfFormat {
                                        message: format!(
                                            "%d expects int, got {:?}",
                                            args[arg_index]
                                        ),
                                        location,
                                    });
                                }
                            }
                            arg_index += 1;
                        }
                        'c' => {
                            if arg_index >= args.len() {
                                return Err(RuntimeError::InvalidPrintfFormat {
                                    message: "Not enough arguments for format string".to_string(),
                                    location,
                                });
                            }
                            match &args[arg_index] {
                                Value::Char(c) => output.push(*c as u8 as char),
                                Value::Int(n) => output.push((*n as u8) as char),
                                _ => {
                                    return Err(RuntimeError::InvalidPrintfFormat {
                                        message: format!(
                                            "%c expects char or int, got {:?}",
                                            args[arg_index]
                                        ),
                                        location,
                                    });
                                }
                            }
                            arg_index += 1;
                        }
                        's' => {
                            if arg_index >= args.len() {
                                return Err(RuntimeError::InvalidPrintfFormat {
                                    message: "Not enough arguments for format string".to_string(),
                                    location,
                                });
                            }
                            match &args[arg_index] {
                                Value::Pointer(addr) => {
                                    // Read string from heap
                                    let string = self.read_string_from_heap(*addr, location)?;
                                    output.push_str(&string);
                                }
                                _ => {
                                    return Err(RuntimeError::InvalidPrintfFormat {
                                        message: format!(
                                            "%s expects pointer, got {:?}",
                                            args[arg_index]
                                        ),
                                        location,
                                    });
                                }
                            }
                            arg_index += 1;
                        }
                        'n' => {
                            // %n writes the number of characters printed so far
                            // This is complex and requires write-back to a pointer
                            return Err(RuntimeError::Generic {
                                message: "%n format specifier not yet implemented".to_string(),
                                location,
                            });
                        }
                        _ => {
                            return Err(RuntimeError::InvalidPrintfFormat {
                                message: format!("Unsupported format specifier: %{}", next_ch),
                                location,
                            });
                        }
                    }
                } else {
                    output.push('%');
                }
            } else if ch == '\\' {
                // Handle escape sequences
                if let Some(&next_ch) = chars.peek() {
                    chars.next();
                    match next_ch {
                        'n' => output.push('\n'),
                        't' => output.push('\t'),
                        'r' => output.push('\r'),
                        '\\' => output.push('\\'),
                        '"' => output.push('"'),
                        _ => {
                            output.push('\\');
                            output.push(next_ch);
                        }
                    }
                } else {
                    output.push('\\');
                }
            } else {
                output.push(ch);
            }
        }

        Ok(output)
    }

    fn read_string_from_heap(
        &self,
        addr: u64,
        location: SourceLocation,
    ) -> Result<String, RuntimeError> {
        let mut bytes = Vec::new();
        let mut current_addr = addr;

        // Read until null terminator
        loop {
            let byte = self
                .heap
                .read_byte(current_addr)
                .map_err(|e| RuntimeError::Generic {
                    message: e,
                    location,
                })?;

            if byte == 0 {
                break;
            }

            bytes.push(byte);
            current_addr += 1;

            // Safety limit
            if bytes.len() > 10000 {
                return Err(RuntimeError::Generic {
                    message: "String too long or missing null terminator".to_string(),
                    location,
                });
            }
        }

        String::from_utf8(bytes).map_err(|_| RuntimeError::Generic {
            message: "Invalid UTF-8 in string".to_string(),
            location,
        })
    }

    fn builtin_malloc(
        &mut self,
        args: &[AstNode],
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        if args.len() != 1 {
            return Err(RuntimeError::Generic {
                message: "malloc requires exactly one argument".to_string(),
                location,
            });
        }

        let size_val = self.evaluate_expr(&args[0])?;
        let size = match size_val {
            Value::Int(n) if n > 0 => n as usize,
            Value::Int(n) => {
                return Err(RuntimeError::Generic {
                    message: format!("malloc size must be positive, got {}", n),
                    location,
                });
            }
            _ => {
                return Err(RuntimeError::TypeError {
                    expected: "int".to_string(),
                    got: format!("{:?}", size_val),
                    location,
                });
            }
        };

        let addr = self
            .heap
            .allocate(size)
            .map_err(|_| RuntimeError::OutOfMemory {
                requested: size,
                limit: self.heap.max_size(),
            })?;

        Ok(Value::Pointer(addr))
    }

    fn builtin_free(
        &mut self,
        args: &[AstNode],
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        if args.len() != 1 {
            return Err(RuntimeError::Generic {
                message: "free requires exactly one argument".to_string(),
                location,
            });
        }

        let ptr_val = self.evaluate_expr(&args[0])?;
        let addr = match ptr_val {
            Value::Pointer(a) => a,
            Value::Null => {
                // free(NULL) is a no-op in C
                return Ok(Value::Int(0));
            }
            _ => {
                return Err(RuntimeError::TypeError {
                    expected: "pointer".to_string(),
                    got: format!("{:?}", ptr_val),
                    location,
                });
            }
        };

        self.heap.free(addr).map_err(|e| {
            if e.contains("Double free") {
                RuntimeError::DoubleFree {
                    address: addr,
                    location,
                }
            } else {
                RuntimeError::InvalidFree {
                    address: addr,
                    location,
                }
            }
        })?;

        Ok(Value::Int(0))
    }
}

// Expression evaluation methods
impl Interpreter {
    fn evaluate_binary_op(
        &mut self,
        op: &BinOp,
        left: &AstNode,
        right: &AstNode,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        use BinOp::*;

        match op {
            // Compound assignment operators need special handling
            AddAssign | SubAssign | MulAssign | DivAssign | ModAssign => {
                // These modify the LHS, so we need to handle them as assignments
                let right_val = self.evaluate_expr(right)?;

                // Get current value of LHS
                let left_val = self.evaluate_expr(left)?;

                // Perform the operation
                let result = match op {
                    AddAssign => self.checked_add_values(&left_val, &right_val, location)?,
                    SubAssign => self.checked_sub_values(&left_val, &right_val, location)?,
                    MulAssign => self.checked_mul_values(&left_val, &right_val, location)?,
                    DivAssign => self.checked_div_values(&left_val, &right_val, location)?,
                    ModAssign => self.checked_mod_values(&left_val, &right_val, location)?,
                    _ => unreachable!(),
                };

                // Assign back to LHS
                self.assign_to_lvalue(left, result.clone(), location)?;
                Ok(result)
            }

            // Regular binary operators
            _ => {
                let left_val = self.evaluate_expr(left)?;
                let right_val = self.evaluate_expr(right)?;

                match op {
                    // Arithmetic
                    Add => self.checked_add_values(&left_val, &right_val, location),
                    Sub => self.checked_sub_values(&left_val, &right_val, location),
                    Mul => self.checked_mul_values(&left_val, &right_val, location),
                    Div => self.checked_div_values(&left_val, &right_val, location),
                    Mod => self.checked_mod_values(&left_val, &right_val, location),

                    // Comparison
                    Eq => self.compare_values(&left_val, &right_val, |a, b| a == b, location),
                    Ne => self.compare_values(&left_val, &right_val, |a, b| a != b, location),
                    Lt => self.compare_values(&left_val, &right_val, |a, b| a < b, location),
                    Le => self.compare_values(&left_val, &right_val, |a, b| a <= b, location),
                    Gt => self.compare_values(&left_val, &right_val, |a, b| a > b, location),
                    Ge => self.compare_values(&left_val, &right_val, |a, b| a >= b, location),

                    // Logical
                    And => {
                        let left_bool = Self::value_to_bool(&left_val, location)?;
                        if !left_bool {
                            Ok(Value::Int(0))
                        } else {
                            let right_bool = Self::value_to_bool(&right_val, location)?;
                            Ok(Value::Int(if right_bool { 1 } else { 0 }))
                        }
                    }
                    Or => {
                        let left_bool = Self::value_to_bool(&left_val, location)?;
                        if left_bool {
                            Ok(Value::Int(1))
                        } else {
                            let right_bool = Self::value_to_bool(&right_val, location)?;
                            Ok(Value::Int(if right_bool { 1 } else { 0 }))
                        }
                    }

                    // Bitwise
                    BitAnd | BitOr | BitXor | BitShl | BitShr => {
                        self.bitwise_op(&left_val, &right_val, op, location)
                    }

                    _ => unreachable!("Compound assignment should be handled above"),
                }
            }
        }
    }

    fn evaluate_unary_op(
        &mut self,
        op: &UnOp,
        operand: &AstNode,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        use UnOp::*;

        match op {
            Neg => {
                let val = self.evaluate_expr(operand)?;
                match val {
                    Value::Int(n) => n
                        .checked_neg()
                        .ok_or(RuntimeError::IntegerOverflow {
                            operation: format!("-{}", n),
                            location,
                        })
                        .map(Value::Int),
                    _ => Err(RuntimeError::TypeError {
                        expected: "int".to_string(),
                        got: format!("{:?}", val),
                        location,
                    }),
                }
            }

            Not => {
                let val = self.evaluate_expr(operand)?;
                let b = Self::value_to_bool(&val, location)?;
                Ok(Value::Int(if b { 0 } else { 1 }))
            }

            BitNot => {
                let val = self.evaluate_expr(operand)?;
                match val {
                    Value::Int(n) => Ok(Value::Int(!n)),
                    _ => Err(RuntimeError::TypeError {
                        expected: "int".to_string(),
                        got: format!("{:?}", val),
                        location,
                    }),
                }
            }

            PreInc | PreDec | PostInc | PostDec => {
                // These modify the operand
                let current_val = self.evaluate_expr(operand)?;
                let one = Value::Int(1);

                let new_val = match op {
                    PreInc | PostInc => self.checked_add_values(&current_val, &one, location)?,
                    PreDec | PostDec => self.checked_sub_values(&current_val, &one, location)?,
                    _ => unreachable!(),
                };

                // Assign the new value
                self.assign_to_lvalue(operand, new_val.clone(), location)?;

                // Return value depends on pre vs post
                match op {
                    PreInc | PreDec => Ok(new_val),
                    PostInc | PostDec => Ok(current_val),
                    _ => unreachable!(),
                }
            }

            Deref => {
                // Dereference a pointer
                let val = self.evaluate_expr(operand)?;
                match val {
                    Value::Pointer(addr) => {
                        // Check if this is a stack address
                        if addr < 0x10000000 {
                            // Stack address - look up in map
                            let (frame_depth, var_name) = self
                                .stack_address_map
                                .get(&addr)
                                .ok_or_else(|| RuntimeError::Generic {
                                    message: format!("Invalid stack pointer: 0x{:x}", addr),
                                    location,
                                })?
                                .clone();

                            // Get the variable from the appropriate frame
                            let frame = self.stack.frames().get(frame_depth).ok_or_else(|| {
                                RuntimeError::Generic {
                                    message: "Invalid frame depth".to_string(),
                                    location,
                                }
                            })?;

                            let var = frame.get_var(&var_name).ok_or_else(|| {
                                RuntimeError::UndefinedVariable {
                                    name: var_name.clone(),
                                    location,
                                }
                            })?;

                            // Return the value
                            Ok(var.value.clone())
                        } else {
                            // Heap address
                            // Look up the type this pointer points to
                            let pointee_type = self.pointer_types.get(&addr).cloned();

                            if let Some(ptr_type) = pointee_type {
                                // We know the type, deserialize from heap
                                self.deserialize_value_from_heap(&ptr_type, addr, location)
                            } else {
                                // Try to infer type from size - assume int for now
                                // This is a fallback for simple cases
                                let bytes = self.heap.read_bytes_at(addr, 4).map_err(|e| {
                                    RuntimeError::Generic {
                                        message: format!(
                                            "Failed to read from address 0x{:x}: {}",
                                            addr, e
                                        ),
                                        location,
                                    }
                                })?;

                                if bytes.len() == 4 {
                                    let int_val = i32::from_le_bytes([
                                        bytes[0], bytes[1], bytes[2], bytes[3],
                                    ]);
                                    Ok(Value::Int(int_val))
                                } else {
                                    Err(RuntimeError::Generic {
                                        message: format!(
                                            "Cannot dereference pointer at 0x{:x}: type unknown. Did you cast the result of malloc?",
                                            addr
                                        ),
                                        location,
                                    })
                                }
                            }
                        }
                    }
                    Value::Null => Err(RuntimeError::NullDereference { location }),
                    _ => Err(RuntimeError::TypeError {
                        expected: "pointer".to_string(),
                        got: format!("{:?}", val),
                        location,
                    }),
                }
            }

            AddrOf => {
                // Take address of a variable
                match operand {
                    AstNode::Variable(name, _) => {
                        // Get the variable from the current frame
                        let frame =
                            self.stack
                                .current_frame()
                                .ok_or_else(|| RuntimeError::Generic {
                                    message: "No stack frame".to_string(),
                                    location,
                                })?;

                        let var = frame.get_var(name).ok_or_else(|| RuntimeError::Generic {
                            message: format!("Undefined variable: {}", name),
                            location,
                        })?;

                        // Use the pre-allocated address
                        Ok(Value::Pointer(var.address))
                    }
                    _ => Err(RuntimeError::Generic {
                        message: "Address-of operator only supports variables currently"
                            .to_string(),
                        location,
                    }),
                }
            }
        }
    }

    // Helper methods for arithmetic with overflow checking
    fn checked_add_values(
        &self,
        left: &Value,
        right: &Value,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => a
                .checked_add(*b)
                .ok_or(RuntimeError::IntegerOverflow {
                    operation: format!("{} + {}", a, b),
                    location,
                })
                .map(Value::Int),
            (Value::Pointer(addr), Value::Int(offset)) => {
                // Pointer arithmetic
                // TODO: Need type info for proper scaling
                Ok(Value::Pointer((*addr as i64 + *offset as i64) as u64))
            }
            _ => Err(RuntimeError::TypeError {
                expected: "int or pointer".to_string(),
                got: format!("{:?} + {:?}", left, right),
                location,
            }),
        }
    }

    fn checked_sub_values(
        &self,
        left: &Value,
        right: &Value,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => a
                .checked_sub(*b)
                .ok_or(RuntimeError::IntegerOverflow {
                    operation: format!("{} - {}", a, b),
                    location,
                })
                .map(Value::Int),
            (Value::Pointer(addr), Value::Int(offset)) => {
                // Pointer arithmetic
                Ok(Value::Pointer((*addr as i64 - *offset as i64) as u64))
            }
            _ => Err(RuntimeError::TypeError {
                expected: "int or pointer".to_string(),
                got: format!("{:?} - {:?}", left, right),
                location,
            }),
        }
    }

    fn checked_mul_values(
        &self,
        left: &Value,
        right: &Value,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => a
                .checked_mul(*b)
                .ok_or(RuntimeError::IntegerOverflow {
                    operation: format!("{} * {}", a, b),
                    location,
                })
                .map(Value::Int),
            _ => Err(RuntimeError::TypeError {
                expected: "int".to_string(),
                got: format!("{:?} * {:?}", left, right),
                location,
            }),
        }
    }

    fn checked_div_values(
        &self,
        left: &Value,
        right: &Value,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    return Err(RuntimeError::Generic {
                        message: "Division by zero".to_string(),
                        location,
                    });
                }
                a.checked_div(*b)
                    .ok_or(RuntimeError::IntegerOverflow {
                        operation: format!("{} / {}", a, b),
                        location,
                    })
                    .map(Value::Int)
            }
            _ => Err(RuntimeError::TypeError {
                expected: "int".to_string(),
                got: format!("{:?} / {:?}", left, right),
                location,
            }),
        }
    }

    fn checked_mod_values(
        &self,
        left: &Value,
        right: &Value,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    return Err(RuntimeError::Generic {
                        message: "Modulo by zero".to_string(),
                        location,
                    });
                }
                a.checked_rem(*b)
                    .ok_or(RuntimeError::IntegerOverflow {
                        operation: format!("{} % {}", a, b),
                        location,
                    })
                    .map(Value::Int)
            }
            _ => Err(RuntimeError::TypeError {
                expected: "int".to_string(),
                got: format!("{:?} % {:?}", left, right),
                location,
            }),
        }
    }

    fn compare_values<F>(
        &self,
        left: &Value,
        right: &Value,
        cmp: F,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError>
    where
        F: Fn(i32, i32) -> bool,
    {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(if cmp(*a, *b) { 1 } else { 0 })),
            (Value::Char(a), Value::Char(b)) => {
                Ok(Value::Int(if cmp(*a as i32, *b as i32) { 1 } else { 0 }))
            }
            (Value::Pointer(a), Value::Pointer(b)) => {
                Ok(Value::Int(if cmp(*a as i32, *b as i32) { 1 } else { 0 }))
            }
            (Value::Pointer(a), Value::Null) | (Value::Null, Value::Pointer(a)) => {
                Ok(Value::Int(if cmp(*a as i32, 0) { 1 } else { 0 }))
            }
            (Value::Null, Value::Null) => Ok(Value::Int(if cmp(0, 0) { 1 } else { 0 })),
            _ => Err(RuntimeError::TypeError {
                expected: "comparable types".to_string(),
                got: format!("{:?} vs {:?}", left, right),
                location,
            }),
        }
    }

    fn bitwise_op(
        &self,
        left: &Value,
        right: &Value,
        op: &BinOp,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => {
                let result = match op {
                    BinOp::BitAnd => a & b,
                    BinOp::BitOr => a | b,
                    BinOp::BitXor => a ^ b,
                    BinOp::BitShl => a << b,
                    BinOp::BitShr => a >> b,
                    _ => unreachable!(),
                };
                Ok(Value::Int(result))
            }
            _ => Err(RuntimeError::TypeError {
                expected: "int".to_string(),
                got: format!("{:?} op {:?}", left, right),
                location,
            }),
        }
    }

    /// Assign a value to an l-value (variable, array element, struct field, etc.)
    fn assign_to_lvalue(
        &mut self,
        lvalue: &AstNode,
        value: Value,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        match lvalue {
            AstNode::Variable(name, _) => {
                // Assign to a variable in the current frame
                let frame =
                    self.stack
                        .current_frame_mut()
                        .ok_or_else(|| RuntimeError::Generic {
                            message: "No stack frame".to_string(),
                            location,
                        })?;

                let var =
                    frame
                        .get_var_mut(name)
                        .ok_or_else(|| RuntimeError::UndefinedVariable {
                            name: name.clone(),
                            location,
                        })?;

                // Check if const
                if var.is_const {
                    return Err(RuntimeError::ConstModification {
                        var: name.clone(),
                        location,
                    });
                }

                var.value = value;
                var.init_state = crate::memory::stack::InitState::Initialized;
                Ok(())
            }

            AstNode::MemberAccess {
                object,
                member,
                location: _,
            } => {
                // Assign to a struct field using . operator
                // Recursive read-modify-write approach to handle nested access
                let mut struct_val = self.evaluate_expr(object)?;

                match &mut struct_val {
                    Value::Struct(fields) => {
                        fields.insert(member.clone(), value);
                    }
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "struct".to_string(),
                            got: format!("{:?}", struct_val),
                            location,
                        });
                    }
                }

                // Assign the modified struct back to the object
                self.assign_to_lvalue(object, struct_val, location)?;
                Ok(())
            }

            AstNode::PointerMemberAccess {
                object,
                member,
                location: _,
            } => {
                // Assign to a struct field using -> operator
                let ptr_val = self.evaluate_expr(object)?;

                match ptr_val {
                    Value::Pointer(addr) => {
                        if addr < 0x10000000 {
                            // Stack address
                            let (frame_depth, var_name) = self
                                .stack_address_map
                                .get(&addr)
                                .ok_or_else(|| RuntimeError::Generic {
                                    message: format!("Invalid stack pointer: 0x{:x}", addr),
                                    location,
                                })?
                                .clone();

                            // Get mutable access to the frame
                            // This is tricky with Rust's borrow checker
                            // We need to use an index-based approach
                            let frames_len = self.stack.frames().len();
                            if frame_depth >= frames_len {
                                return Err(RuntimeError::Generic {
                                    message: "Invalid frame depth".to_string(),
                                    location,
                                });
                            }

                            // Get mutable reference to the specific frame
                            let frame = self.stack.frame_mut(frame_depth).ok_or_else(|| {
                                RuntimeError::Generic {
                                    message: "Invalid frame depth".to_string(),
                                    location,
                                }
                            })?;

                            let var = frame.get_var_mut(&var_name).ok_or_else(|| {
                                RuntimeError::UndefinedVariable {
                                    name: var_name.clone(),
                                    location,
                                }
                            })?;

                            match &mut var.value {
                                Value::Struct(fields) => {
                                    fields.insert(member.clone(), value);
                                    Ok(())
                                }
                                _ => Err(RuntimeError::TypeError {
                                    expected: "struct".to_string(),
                                    got: format!("{:?}", var.value),
                                    location,
                                }),
                            }
                        } else {
                            // Heap address - write struct field to heap
                            // Look up the pointer type
                            let pointee_type = self.pointer_types.get(&addr)
                                .ok_or_else(|| RuntimeError::Generic {
                                    message: format!("Unknown type for pointer 0x{:x}. Did you cast the result of malloc?", addr),
                                    location,
                                })?
                                .clone(); // Clone to avoid borrow checker issues

                            // Ensure it's a struct type
                            let struct_name = match &pointee_type.base {
                                BaseType::Struct(name) => name.clone(),
                                _ => {
                                    return Err(RuntimeError::TypeError {
                                        expected: "struct pointer".to_string(),
                                        got: format!("{:?}", pointee_type),
                                        location,
                                    });
                                }
                            };

                            // Calculate field offset
                            let offset =
                                self.calculate_field_offset(&struct_name, &member, location)?;

                            // Get field type
                            let field_type =
                                self.get_field_type(&struct_name, &member, location)?;

                            // Serialize field value to heap
                            self.serialize_value_to_heap(
                                &value,
                                &field_type,
                                addr + offset as u64,
                                location,
                            )?;
                            Ok(())
                        }
                    }
                    Value::Null => Err(RuntimeError::NullDereference { location }),
                    _ => Err(RuntimeError::TypeError {
                        expected: "pointer".to_string(),
                        got: format!("{:?}", ptr_val),
                        location,
                    }),
                }
            }

            AstNode::UnaryOp {
                op: UnOp::Deref,
                operand,
                location: _,
            } => {
                // Dereference assignment: *ptr = value
                let ptr_val = self.evaluate_expr(operand)?;

                match ptr_val {
                    Value::Pointer(addr) => {
                        // Check if this is a stack address
                        if addr < 0x10000000 {
                            // Stack address - look up in map
                            let (frame_depth, var_name) = self
                                .stack_address_map
                                .get(&addr)
                                .ok_or_else(|| RuntimeError::Generic {
                                    message: format!("Invalid stack pointer: 0x{:x}", addr),
                                    location,
                                })?
                                .clone();

                            // Get mutable access to the frame
                            let frames_len = self.stack.frames().len();
                            if frame_depth >= frames_len {
                                return Err(RuntimeError::Generic {
                                    message: "Invalid frame depth".to_string(),
                                    location,
                                });
                            }

                            let frame = self.stack.frame_mut(frame_depth).ok_or_else(|| {
                                RuntimeError::Generic {
                                    message: "Invalid frame depth".to_string(),
                                    location,
                                }
                            })?;

                            let var = frame.get_var_mut(&var_name).ok_or_else(|| {
                                RuntimeError::UndefinedVariable {
                                    name: var_name.clone(),
                                    location,
                                }
                            })?;

                            // Update the variable's value
                            var.value = value;
                            var.init_state = crate::memory::stack::InitState::Initialized;
                            Ok(())
                        } else {
                            // Heap address
                            // Determine the type of value we're writing
                            // For now, handle basic types (int, char, pointer)
                            match &value {
                                Value::Int(n) => {
                                    let bytes = n.to_le_bytes();
                                    for (i, &byte) in bytes.iter().enumerate() {
                                        self.heap.write_byte(addr + i as u64, byte).map_err(
                                            |e| RuntimeError::Generic {
                                                message: e,
                                                location,
                                            },
                                        )?;
                                    }
                                    Ok(())
                                }
                                Value::Char(c) => {
                                    self.heap.write_byte(addr, *c as u8).map_err(|e| {
                                        RuntimeError::Generic {
                                            message: e,
                                            location,
                                        }
                                    })?;
                                    Ok(())
                                }
                                Value::Pointer(ptr_addr) => {
                                    let bytes = ptr_addr.to_le_bytes();
                                    for (i, &byte) in bytes.iter().enumerate() {
                                        self.heap.write_byte(addr + i as u64, byte).map_err(
                                            |e| RuntimeError::Generic {
                                                message: e,
                                                location,
                                            },
                                        )?;
                                    }
                                    Ok(())
                                }
                                Value::Null => {
                                    let bytes = 0u64.to_le_bytes();
                                    for (i, &byte) in bytes.iter().enumerate() {
                                        self.heap.write_byte(addr + i as u64, byte).map_err(
                                            |e| RuntimeError::Generic {
                                                message: e,
                                                location,
                                            },
                                        )?;
                                    }
                                    Ok(())
                                }
                                _ => Err(RuntimeError::Generic {
                                    message: format!(
                                        "Cannot assign value of type {:?} through pointer dereference",
                                        value
                                    ),
                                    location,
                                }),
                            }
                        }
                    }
                    Value::Null => Err(RuntimeError::NullDereference { location }),
                    _ => Err(RuntimeError::TypeError {
                        expected: "pointer".to_string(),
                        got: format!("{:?}", ptr_val),
                        location,
                    }),
                }
            }

            AstNode::ArrayAccess {
                array,
                index,
                location: _,
            } => {
                // Assign to an array element: arr[idx] = value
                // Recursive read-modify-write approach
                let idx_val = self.evaluate_expr(index)?;
                let idx = match idx_val {
                    Value::Int(i) => i,
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "int".to_string(),
                            got: format!("{:?}", idx_val),
                            location,
                        });
                    }
                };

                let mut array_val = self.evaluate_expr(array)?;

                // Track whether we need to write back the modified value
                // (true for Value::Array read-modify-write, false for Value::Pointer in-place modification)
                let needs_writeback = matches!(array_val, Value::Array(_));

                match &mut array_val {
                    Value::Array(elements) => {
                        if idx < 0 || idx as usize >= elements.len() {
                            return Err(RuntimeError::BufferOverrun {
                                index: idx as usize,
                                size: elements.len(),
                                location,
                            });
                        }
                        elements[idx as usize] = value;
                    }
                    Value::Pointer(addr) => {
                        let addr = *addr;
                        if addr == 0 {
                            return Err(RuntimeError::NullDereference { location });
                        }

                        if addr < 0x10000000 {
                            // Stack pointer dereference assignment (for decayed arrays)
                            let (frame_depth, var_name) = self
                                .stack_address_map
                                .get(&addr)
                                .ok_or_else(|| RuntimeError::Generic {
                                    message: format!("Invalid stack pointer: 0x{:x}", addr),
                                    location,
                                })?
                                .clone();

                            let frames_len = self.stack.frames().len();
                            if frame_depth >= frames_len {
                                return Err(RuntimeError::Generic {
                                    message: "Invalid frame depth".to_string(),
                                    location,
                                });
                            }

                            let frame = self.stack.frame_mut(frame_depth).ok_or_else(|| {
                                RuntimeError::Generic {
                                    message: "Invalid frame depth".to_string(),
                                    location,
                                }
                            })?;

                            let var = frame.get_var_mut(&var_name).ok_or_else(|| {
                                RuntimeError::UndefinedVariable {
                                    name: var_name.clone(),
                                    location,
                                }
                            })?;

                            // Handle array indexing for stack arrays
                            match &mut var.value {
                                Value::Array(elements) => {
                                    if idx < 0 || idx as usize >= elements.len() {
                                        return Err(RuntimeError::BufferOverrun {
                                            index: idx as usize,
                                            size: elements.len(),
                                            location,
                                        });
                                    }
                                    elements[idx as usize] = value;
                                    // Mark as initialized if this was the variable's first write
                                    var.init_state = crate::memory::stack::InitState::Initialized;
                                }
                                _ => {
                                    // Not an array - only allow index 0
                                    if idx == 0 {
                                        var.value = value;
                                        var.init_state =
                                            crate::memory::stack::InitState::Initialized;
                                    } else {
                                        return Err(RuntimeError::Generic {
                                            message: format!(
                                                "Pointer to non-array stack variable, index {} out of bounds",
                                                idx
                                            ),
                                            location,
                                        });
                                    }
                                }
                            }
                        } else {
                            // Heap pointer assignment
                            if let Some(elem_type) = self.pointer_types.get(&addr).cloned() {
                                let elem_size = sizeof_type(&elem_type, &self.struct_defs);
                                let offset = (idx as i64) * (elem_size as i64);
                                let target_addr = if offset >= 0 {
                                    addr + (offset as u64)
                                } else {
                                    addr - ((-offset) as u64)
                                };

                                self.serialize_value_to_heap(
                                    &value,
                                    &elem_type,
                                    target_addr,
                                    location,
                                )?;
                            } else {
                                return Err(RuntimeError::Generic {
                                    message: format!(
                                        "Unknown pointer type for indexing at 0x{:x}",
                                        addr
                                    ),
                                    location,
                                });
                            }
                        }
                    }
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "array or pointer".to_string(),
                            got: format!("{:?}", array_val),
                            location,
                        });
                    }
                }

                // Assign the modified array back to the object only if we did read-modify-write
                // For pointers to stack/heap arrays, we modify in place, so no writeback needed
                if needs_writeback {
                    self.assign_to_lvalue(array, array_val, location)?;
                }
                Ok(())
            }

            _ => Err(RuntimeError::Generic {
                message: format!(
                    "Assignment to this l-value type not yet implemented: {:?}",
                    lvalue
                ),
                location,
            }),
        }
    }

    /// Calculate the byte offset of a field within a struct
    /// Uses sequential packing (no padding/alignment)
    fn calculate_field_offset(
        &self,
        struct_name: &str,
        field_name: &str,
        location: SourceLocation,
    ) -> Result<usize, RuntimeError> {
        let struct_def =
            self.struct_defs
                .get(struct_name)
                .ok_or_else(|| RuntimeError::Generic {
                    message: format!("Struct '{}' not defined", struct_name),
                    location,
                })?;

        let mut offset = 0;
        for field in &struct_def.fields {
            if field.name == field_name {
                return Ok(offset);
            }
            offset += sizeof_type(&field.field_type, &self.struct_defs);
        }

        Err(RuntimeError::Generic {
            message: format!(
                "Struct '{}' does not have field '{}'",
                struct_name, field_name
            ),
            location,
        })
    }

    /// Get the type of a specific field within a struct
    fn get_field_type(
        &self,
        struct_name: &str,
        field_name: &str,
        location: SourceLocation,
    ) -> Result<Type, RuntimeError> {
        let struct_def =
            self.struct_defs
                .get(struct_name)
                .ok_or_else(|| RuntimeError::Generic {
                    message: format!("Struct '{}' not defined", struct_name),
                    location,
                })?;

        for field in &struct_def.fields {
            if field.name == field_name {
                return Ok(field.field_type.clone());
            }
        }

        Err(RuntimeError::Generic {
            message: format!(
                "Struct '{}' does not have field '{}'",
                struct_name, field_name
            ),
            location,
        })
    }

    /// Serialize a value to heap bytes (sequential packing, no padding)
    fn serialize_value_to_heap(
        &mut self,
        value: &Value,
        value_type: &Type,
        base_addr: u64,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        match value {
            Value::Int(n) => {
                // Write 4 bytes (little-endian)
                let bytes = n.to_le_bytes();
                for (i, byte) in bytes.iter().enumerate() {
                    self.heap
                        .write_byte(base_addr + i as u64, *byte)
                        .map_err(|e| RuntimeError::Generic {
                            message: format!("Failed to write int to heap: {}", e),
                            location,
                        })?;
                }
                Ok(())
            }
            Value::Char(c) => {
                // Write 1 byte (c is already i8)
                self.heap
                    .write_byte(base_addr, *c as u8)
                    .map_err(|e| RuntimeError::Generic {
                        message: format!("Failed to write char to heap: {}", e),
                        location,
                    })?;
                Ok(())
            }
            Value::Uninitialized => {
                // Don't write anything for uninitialized values
                // The heap already marks bytes as uninitialized by default
                Ok(())
            }
            Value::Pointer(addr) => {
                // Write 8 bytes (little-endian)
                let bytes = addr.to_le_bytes();
                for (i, byte) in bytes.iter().enumerate() {
                    self.heap
                        .write_byte(base_addr + i as u64, *byte)
                        .map_err(|e| RuntimeError::Generic {
                            message: format!("Failed to write pointer to heap: {}", e),
                            location,
                        })?;
                }
                Ok(())
            }
            Value::Null => {
                // Write 8 bytes of zeros
                for i in 0..8 {
                    self.heap
                        .write_byte(base_addr + i, 0)
                        .map_err(|e| RuntimeError::Generic {
                            message: format!("Failed to write null to heap: {}", e),
                            location,
                        })?;
                }
                Ok(())
            }
            Value::Struct(fields) => {
                // Get struct name from type
                let struct_name = match &value_type.base {
                    BaseType::Struct(name) => name,
                    _ => {
                        return Err(RuntimeError::Generic {
                            message: "Expected struct type for struct value".to_string(),
                            location,
                        });
                    }
                };

                // Get struct definition
                let struct_def = self
                    .struct_defs
                    .get(struct_name)
                    .ok_or_else(|| RuntimeError::Generic {
                        message: format!("Struct '{}' not defined", struct_name),
                        location,
                    })?
                    .clone(); // Clone to avoid borrow checker issues

                // Write each field sequentially
                let mut offset = 0;
                for field in &struct_def.fields {
                    if let Some(field_value) = fields.get(&field.name) {
                        self.serialize_value_to_heap(
                            field_value,
                            &field.field_type,
                            base_addr + offset as u64,
                            location,
                        )?;
                    }
                    offset += sizeof_type(&field.field_type, &self.struct_defs);
                }
                Ok(())
            }
            Value::Array(elements) => {
                // Get element type
                let elem_type = match &value_type.base {
                    BaseType::Int => Type {
                        base: BaseType::Int,
                        pointer_depth: 0,
                        is_const: false,
                        array_dims: Vec::new(),
                    },
                    BaseType::Char => Type {
                        base: BaseType::Char,
                        pointer_depth: 0,
                        is_const: false,
                        array_dims: Vec::new(),
                    },
                    BaseType::Struct(name) => Type {
                        base: BaseType::Struct(name.clone()),
                        pointer_depth: 0,
                        is_const: false,
                        array_dims: Vec::new(),
                    },
                    _ => {
                        return Err(RuntimeError::Generic {
                            message: format!(
                                "Unsupported array element type: {:?}",
                                value_type.base
                            ),
                            location,
                        });
                    }
                };

                let elem_size = sizeof_type(&elem_type, &self.struct_defs);
                for (i, elem) in elements.iter().enumerate() {
                    self.serialize_value_to_heap(
                        elem,
                        &elem_type,
                        base_addr + (i * elem_size) as u64,
                        location,
                    )?;
                }
                Ok(())
            }
        }
    }

    /// Deserialize a value from heap bytes
    fn deserialize_value_from_heap(
        &self,
        value_type: &Type,
        base_addr: u64,
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match &value_type.base {
            BaseType::Int if value_type.pointer_depth == 0 => {
                // Read 4 bytes (little-endian)
                let mut bytes = [0u8; 4];
                for i in 0..4 {
                    bytes[i] = self.heap.read_byte(base_addr + i as u64).map_err(|e| {
                        RuntimeError::Generic {
                            message: format!("Failed to read int from heap: {}", e),
                            location,
                        }
                    })?;
                }
                Ok(Value::Int(i32::from_le_bytes(bytes)))
            }
            BaseType::Char if value_type.pointer_depth == 0 => {
                // Read 1 byte
                let byte = self
                    .heap
                    .read_byte(base_addr)
                    .map_err(|e| RuntimeError::Generic {
                        message: format!("Failed to read char from heap: {}", e),
                        location,
                    })?;
                Ok(Value::Char(byte as i8))
            }
            _ if value_type.pointer_depth > 0 => {
                // Read 8 bytes (pointer)
                let mut bytes = [0u8; 8];
                for i in 0..8 {
                    bytes[i] = self.heap.read_byte(base_addr + i as u64).map_err(|e| {
                        RuntimeError::Generic {
                            message: format!("Failed to read pointer from heap: {}", e),
                            location,
                        }
                    })?;
                }
                let addr = u64::from_le_bytes(bytes);
                if addr == 0 {
                    Ok(Value::Null)
                } else {
                    Ok(Value::Pointer(addr))
                }
            }
            BaseType::Struct(struct_name) if value_type.pointer_depth == 0 => {
                // Read struct fields
                let struct_def = self
                    .struct_defs
                    .get(struct_name)
                    .ok_or_else(|| RuntimeError::Generic {
                        message: format!("Struct '{}' not defined", struct_name),
                        location,
                    })?
                    .clone(); // Clone to avoid borrow checker issues

                let mut fields = HashMap::new();
                let mut offset = 0;
                for field in &struct_def.fields {
                    let field_value = self.deserialize_value_from_heap(
                        &field.field_type,
                        base_addr + offset as u64,
                        location,
                    )?;
                    fields.insert(field.name.clone(), field_value);
                    offset += sizeof_type(&field.field_type, &self.struct_defs);
                }
                Ok(Value::Struct(fields))
            }
            _ => Err(RuntimeError::Generic {
                message: format!(
                    "Deserialization not yet implemented for type: {:?}",
                    value_type
                ),
                location,
            }),
        }
    }

    /// Infer the type of an expression
    /// This is needed for sizeof(expr) to work properly
    fn infer_expr_type(&self, expr: &AstNode) -> Result<Type, RuntimeError> {
        match expr {
            AstNode::IntLiteral(_, _) => Ok(Type::new(BaseType::Int)),

            AstNode::CharLiteral(_, _) => Ok(Type::new(BaseType::Char)),

            AstNode::StringLiteral(_, _) => {
                // String literals have type char*
                Ok(Type::new(BaseType::Char).with_pointer())
            }

            AstNode::Null { .. } => {
                // NULL has type void*
                Ok(Type::new(BaseType::Void).with_pointer())
            }

            AstNode::Variable(name, location) => {
                // Look up variable type in current frame
                let frame = self
                    .stack
                    .current_frame()
                    .ok_or_else(|| RuntimeError::Generic {
                        message: "No stack frame".to_string(),
                        location: *location,
                    })?;

                let var = frame
                    .get_var(name)
                    .ok_or_else(|| RuntimeError::UndefinedVariable {
                        name: name.clone(),
                        location: *location,
                    })?;

                Ok(var.var_type.clone())
            }

            AstNode::BinaryOp {
                op,
                left,
                right,
                location,
            } => {
                // Most binary ops return int, but pointer arithmetic returns pointer
                match op {
                    BinOp::Add | BinOp::Sub => {
                        // Check if either operand is a pointer
                        let left_type = self.infer_expr_type(left)?;
                        let right_type = self.infer_expr_type(right)?;

                        if left_type.pointer_depth > 0 {
                            Ok(left_type)
                        } else if right_type.pointer_depth > 0 {
                            Ok(right_type)
                        } else {
                            Ok(Type::new(BaseType::Int))
                        }
                    }
                    _ => Ok(Type::new(BaseType::Int)),
                }
            }

            AstNode::UnaryOp {
                op,
                operand,
                location,
            } => {
                match op {
                    UnOp::Deref => {
                        // *ptr: if operand is T*, result is T
                        let operand_type = self.infer_expr_type(operand)?;
                        if operand_type.pointer_depth == 0 {
                            return Err(RuntimeError::TypeError {
                                expected: "pointer".to_string(),
                                got: format!("{:?}", operand_type),
                                location: *location,
                            });
                        }
                        let mut result_type = operand_type;
                        result_type.pointer_depth -= 1;
                        Ok(result_type)
                    }
                    UnOp::AddrOf => {
                        // &var: if operand is T, result is T*
                        let operand_type = self.infer_expr_type(operand)?;
                        let mut result_type = operand_type;
                        result_type.pointer_depth += 1;
                        Ok(result_type)
                    }
                    UnOp::Neg | UnOp::BitNot => Ok(Type::new(BaseType::Int)),
                    UnOp::Not => Ok(Type::new(BaseType::Int)), // logical not returns int (0 or 1)
                    UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec => {
                        // ++/-- returns the type of the operand
                        self.infer_expr_type(operand)
                    }
                }
            }

            AstNode::TernaryOp {
                true_expr,
                false_expr,
                ..
            } => {
                // Ternary operator returns the type of the true branch (simplified)
                // In real C, it's more complex with implicit conversions
                self.infer_expr_type(true_expr)
            }

            AstNode::FunctionCall { name, location, .. } => {
                // Look up function return type
                let func_def =
                    self.function_defs
                        .get(name)
                        .ok_or_else(|| RuntimeError::Generic {
                            message: format!("Function '{}' not found", name),
                            location: *location,
                        })?;
                Ok(func_def.return_type.clone())
            }

            AstNode::ArrayAccess {
                array, location, ..
            } => {
                // arr[i]: if arr is T[], result is T
                let array_type = self.infer_expr_type(array)?;

                if !array_type.array_dims.is_empty() {
                    // Array type - remove one dimension
                    let mut result_type = array_type;
                    result_type.array_dims.remove(0);
                    Ok(result_type)
                } else if array_type.pointer_depth > 0 {
                    // Pointer type - dereference
                    let mut result_type = array_type;
                    result_type.pointer_depth -= 1;
                    Ok(result_type)
                } else {
                    Err(RuntimeError::TypeError {
                        expected: "array or pointer".to_string(),
                        got: format!("{:?}", array_type),
                        location: *location,
                    })
                }
            }

            AstNode::MemberAccess {
                object,
                member,
                location,
            } => {
                // obj.field: get the field type from the struct definition
                let object_type = self.infer_expr_type(object)?;

                let struct_name = match &object_type.base {
                    BaseType::Struct(name) => name,
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "struct".to_string(),
                            got: format!("{:?}", object_type),
                            location: *location,
                        });
                    }
                };

                let field_type = self.get_field_type(struct_name, member, *location)?;
                Ok(field_type)
            }

            AstNode::PointerMemberAccess {
                object,
                member,
                location,
            } => {
                // ptr->field: dereference pointer then get field type
                let pointer_type = self.infer_expr_type(object)?;

                if pointer_type.pointer_depth == 0 {
                    return Err(RuntimeError::TypeError {
                        expected: "pointer".to_string(),
                        got: format!("{:?}", pointer_type),
                        location: *location,
                    });
                }

                let struct_name = match &pointer_type.base {
                    BaseType::Struct(name) => name,
                    _ => {
                        return Err(RuntimeError::TypeError {
                            expected: "struct pointer".to_string(),
                            got: format!("{:?}", pointer_type),
                            location: *location,
                        });
                    }
                };

                let field_type = self.get_field_type(struct_name, member, *location)?;
                Ok(field_type)
            }

            AstNode::Cast { target_type, .. } => {
                // Cast returns the target type
                Ok(target_type.clone())
            }

            AstNode::SizeofType { .. } | AstNode::SizeofExpr { .. } => {
                // sizeof returns int
                Ok(Type::new(BaseType::Int))
            }

            _ => Err(RuntimeError::Generic {
                message: format!("Cannot infer type of expression: {:?}", expr),
                location: SourceLocation::new(1, 1),
            }),
        }
    }
}
