//! Statement execution implementation
//!
//! This module handles the execution of all C statement types, including:
//!
//! - Variable declarations and initializations
//! - Control flow (if/else, while, for, do-while, switch/case)
//! - Function calls and returns
//! - Blocks and compound statements
//! - Continue and break statements
//!
//! # Implementation
//!
//! All statement execution methods are implemented as `pub(crate)` methods
//! on the [`Interpreter`] struct, allowing them to access and modify the
//! interpreter's state (stack, heap, terminal, etc.).
//!
//! # Control Flow
//!
//! - Loop constructs set `break_encountered` and `continue_encountered` flags
//! - Return statements set `return_value` and signal function exit
//! - Switch statements support fallthrough behavior matching C semantics

use crate::interpreter::engine::Interpreter;
use crate::interpreter::errors::RuntimeError;
use crate::memory::{sizeof_type, value::Value};
use crate::parser::ast::*;

impl Interpreter {
    pub(crate) fn execute_var_decl(
        &mut self,
        name: &str,
        var_type: &Type,
        init: Option<&AstNode>,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        // Check that we have a stack frame
        if self.stack.current_frame().is_none() {
            return Err(RuntimeError::NoStackFrame { location });
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
                // Initialize array with uninitialized values
                let size = var_type.array_dims[0].unwrap_or(0);
                let default_element = Value::Uninitialized;
                let elements: Vec<Value> = (0..size).map(|_| default_element.clone()).collect();
                (
                    crate::memory::stack::InitState::Uninitialized,
                    Some(Value::Array(elements)),
                )
            } else {
                // For structs, create a default struct value with all fields initialized
                match &var_type.base {
                    BaseType::Struct(_struct_name) => {
                        // Helper function to create default value for a type
                        fn create_default_value<S: std::hash::BuildHasher>(
                            type_: &Type,
                            struct_defs: &std::collections::HashMap<String, StructDef, S>,
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
        let var_size = sizeof_type(var_type, &self.struct_defs) as u64;
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
            if var_type.pointer_depth > 0 {
                if let Some(addr) = val.as_pointer() {
                    if addr != 0 {
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

    pub(crate) fn execute_assignment(
        &mut self,
        lhs: &AstNode,
        rhs: &AstNode,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        let value = self.evaluate_expr(rhs)?;
        self.assign_to_lvalue(lhs, value, location)
    }

    pub(crate) fn execute_compound_assignment(
        &mut self,
        lhs: &AstNode,
        op: &BinOp,
        rhs: &AstNode,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        let rhs_val = self.evaluate_expr(rhs)?;
        let lhs_val = self.evaluate_expr(lhs)?;

        let result_val = match op {
            BinOp::AddAssign => self.checked_add_values(&lhs_val, &rhs_val, location)?,
            BinOp::SubAssign => self.checked_sub_values(&lhs_val, &rhs_val, location)?,
            BinOp::MulAssign => self.checked_mul_values(&lhs_val, &rhs_val, location)?,
            BinOp::DivAssign => self.checked_div_values(&lhs_val, &rhs_val, location)?,
            BinOp::ModAssign => self.checked_mod_values(&lhs_val, &rhs_val, location)?,
            _ => {
                return Err(RuntimeError::UnsupportedOperation {
                    message: format!("Unsupported compound assignment operator: {:?}", op),
                    location,
                });
            }
        };

        self.assign_to_lvalue(lhs, result_val, location)
    }

    pub(crate) fn execute_return(
        &mut self,
        expr: Option<&AstNode>,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        if let Some(ret_expr) = expr {
            let return_val = self.evaluate_expr(ret_expr)?;
            self.return_value = Some(return_val);
        } else {
            self.return_value = None;
        }

        self.current_location = location;
        self.take_snapshot()?;
        self.finished = true;
        Ok(())
    }

    pub(crate) fn execute_if(
        &mut self,
        condition: &AstNode,
        then_branch: &[AstNode],
        else_branch: Option<&[AstNode]>,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        self.current_location = location;
        self.take_snapshot()?;

        let cond_val = self.evaluate_expr(condition)?;
        let cond_bool = Self::value_to_bool(&cond_val, location)?;

        if cond_bool {
            self.enter_scope();
            for stmt in then_branch {
                let needs_snapshot = self.execute_statement(stmt)?;
                if self.finished {
                    self.exit_scope();
                    return Ok(());
                }

                if self.should_break || self.should_continue {
                    self.exit_scope();
                    return Ok(());
                }

                if needs_snapshot {
                    self.take_snapshot()?;
                }
            }
            self.exit_scope();
        } else if let Some(else_stmts) = else_branch {
            self.enter_scope();
            for stmt in else_stmts {
                let needs_snapshot = self.execute_statement(stmt)?;
                if self.finished {
                    self.exit_scope();
                    return Ok(());
                }

                if self.should_break || self.should_continue {
                    self.exit_scope();
                    return Ok(());
                }

                if needs_snapshot {
                    self.take_snapshot()?;
                }
            }
            self.exit_scope();
        }

        Ok(())
    }

    pub(crate) fn execute_while(
        &mut self,
        condition: &AstNode,
        body: &[AstNode],
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        self.execution_depth += 1;
        'outer_loop: loop {
            let cond_val = self.evaluate_expr(condition)?;
            let cond_bool = Self::value_to_bool(&cond_val, location)?;

            if !cond_bool {
                self.current_location = location;
                self.take_snapshot()?;
                break;
            }

            self.current_location = location;
            self.take_snapshot()?;

            self.enter_scope();
            for stmt in body {
                let needs_snapshot = self.execute_statement(stmt)?;
                if self.finished {
                    self.exit_scope();
                    self.execution_depth -= 1;
                    return Ok(());
                }
                if self.should_break {
                    self.exit_scope();
                    self.should_break = false;
                    break 'outer_loop;
                }
                if self.should_continue {
                    self.exit_scope();
                    self.should_continue = false;
                    break;
                }
                if needs_snapshot {
                    self.take_snapshot()?;
                }
            }
            self.exit_scope();
        }
        self.execution_depth -= 1;

        Ok(())
    }

    pub(crate) fn execute_do_while(
        &mut self,
        body: &[AstNode],
        condition: &AstNode,
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        self.execution_depth += 1;
        'outer_loop: loop {
            self.current_location = location;
            self.take_snapshot()?;

            self.enter_scope();
            for stmt in body {
                let needs_snapshot = self.execute_statement(stmt)?;
                if self.finished {
                    self.exit_scope();
                    self.execution_depth -= 1;
                    return Ok(());
                }
                if self.should_break {
                    self.exit_scope();
                    self.should_break = false;
                    break 'outer_loop;
                }
                if self.should_continue {
                    self.exit_scope();
                    self.should_continue = false;
                    break;
                }
                if needs_snapshot {
                    self.take_snapshot()?;
                }
            }
            self.exit_scope();

            let cond_val = self.evaluate_expr(condition)?;
            let cond_bool = Self::value_to_bool(&cond_val, location)?;

            if !cond_bool {
                self.current_location = location;
                self.take_snapshot()?;
                break;
            }
        }
        self.execution_depth -= 1;

        Ok(())
    }

    pub(crate) fn execute_for(
        &mut self,
        init: Option<&AstNode>,
        condition: Option<&AstNode>,
        increment: Option<&AstNode>,
        body: &[AstNode],
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        self.enter_scope(); // Scope for init and loop variable

        if let Some(init_stmt) = init {
            let _needs_snapshot = self.execute_statement(init_stmt)?;
        }

        self.execution_depth += 1;
        'outer_loop: loop {
            if let Some(cond) = condition {
                let cond_val = self.evaluate_expr(cond)?;
                let cond_bool = Self::value_to_bool(&cond_val, location)?;

                if !cond_bool {
                    self.current_location = location;
                    self.take_snapshot()?;
                    break;
                }
            }

            self.current_location = location;
            self.take_snapshot()?;

            self.enter_scope(); // Scope for body
            for stmt in body {
                let needs_snapshot = self.execute_statement(stmt)?;
                if self.finished {
                    self.exit_scope(); // Exit body scope
                    self.exit_scope(); // Exit loop scope
                    self.execution_depth -= 1;
                    return Ok(());
                }
                if self.should_break {
                    self.exit_scope(); // Exit body scope
                    self.should_break = false;
                    break 'outer_loop;
                }
                if self.should_continue {
                    self.exit_scope(); // Exit body scope
                    self.should_continue = false;
                    break;
                }
                if needs_snapshot {
                    self.take_snapshot()?;
                }
            }
            self.exit_scope(); // Exit body scope

            if let Some(inc) = increment {
                self.evaluate_expr(inc)?;
            }
        }
        self.execution_depth -= 1;
        self.exit_scope(); // Exit loop scope

        Ok(())
    }

    pub(crate) fn execute_switch(
        &mut self,
        expr: &AstNode,
        cases: &[CaseNode],
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        use crate::parser::ast::CaseNode;

        self.current_location = location;
        self.take_snapshot()?;

        let switch_val = self.evaluate_expr(expr)?;

        let mut match_index: Option<usize> = None;
        let mut default_index: Option<usize> = None;

        for (i, case) in cases.iter().enumerate() {
            match case {
                CaseNode::Case { value, .. } => {
                    let case_val = self.evaluate_expr(value)?;
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

        let start_index = match_index.or(default_index);

        if let Some(start) = start_index {
            self.enter_scope();
            for case in &cases[start..] {
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
                        self.exit_scope();
                        return Ok(());
                    }

                    if needs_snapshot {
                        self.take_snapshot()?;
                    }

                    if self.should_break {
                        self.should_break = false;
                        self.exit_scope();
                        return Ok(());
                    }
                    if self.should_continue {
                        self.exit_scope();
                        return Ok(());
                    }
                }
            }
            self.exit_scope();
        }

        Ok(())
    }

    pub(crate) fn values_equal(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Pointer(a), Value::Pointer(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Null, Value::Pointer(0)) | (Value::Pointer(0), Value::Null) => true,
            _ => false,
        }
    }

    pub(crate) fn execute_function_call(
        &mut self,
        name: &str,
        args: &[AstNode],
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        match name {
            "printf" => self.builtin_printf(args, location),
            "malloc" => self.builtin_malloc(args, location),
            "free" => self.builtin_free(args, location),
            _ => self.call_user_function(name, args, location),
        }
    }

    pub(crate) fn call_user_function(
        &mut self,
        name: &str,
        args: &[AstNode],
        location: SourceLocation,
    ) -> Result<Value, RuntimeError> {
        self.current_location = location;
        self.take_snapshot()?;

        let func_def = self.function_defs.get(name).cloned().ok_or_else(|| {
            RuntimeError::UndefinedFunction {
                name: name.to_string(),
                location,
            }
        })?;

        if args.len() != func_def.params.len() {
            return Err(RuntimeError::ArgumentCountMismatch {
                function: name.to_string(),
                expected: func_def.params.len(),
                got: args.len(),
                location,
            });
        }

        let mut arg_values = Vec::new();
        for arg in args {
            arg_values.push(self.evaluate_expr(arg)?);
        }

        self.execution_depth += 1;
        self.stack.push_frame(name.to_string(), Some(location));

        for (param, value) in func_def.params.iter().zip(arg_values.iter()) {
            let address = self.next_stack_address;
            self.next_stack_address += 1;

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

        let saved_finished = self.finished;
        let saved_return_value = self.return_value.clone();
        self.finished = false;
        self.return_value = None;

        self.current_location = func_def.location;
        self.take_snapshot()?;

        for stmt in &func_def.body {
            let needs_snapshot = self.execute_statement(stmt)?;
            if self.finished {
                break;
            }
            if needs_snapshot {
                self.take_snapshot()?;
            }
        }

        let return_val = self.return_value.clone().unwrap_or(Value::Int(0));
        self.stack.pop_frame();
        self.execution_depth -= 1;
        self.finished = saved_finished;
        self.return_value = saved_return_value;
        self.current_location = location;

        Ok(return_val)
    }
}
