//! Jump-style control-flow statement execution (`return` and `switch`).
//!
//! Adds `impl Interpreter` methods for statements that transfer control
//! non-linearly within a function. Loop-based control flow (`while`, `for`,
//! `do-while`, `break`, `continue`) lives in [`crate::interpreter::loops`].

use crate::interpreter::engine::{ControlFlow, Interpreter};
use crate::interpreter::errors::RuntimeError;
use crate::memory::value::Value;
use crate::parser::ast::{AstNode, CaseNode, SourceLocation};

impl Interpreter {
    /// Executes a `return` statement, capturing a snapshot at the return site.
    ///
    /// If an expression is provided its value is stored in `self.return_value`;
    /// otherwise `return_value` is cleared (void return). Sets `control_flow` to
    /// [`ControlFlow::Return`] so callers unwind the statement loop.
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

        self.snapshot_at(location)?;
        self.control_flow = ControlFlow::Return;
        Ok(())
    }

    /// Executes a `switch` statement.
    ///
    /// Evaluates the switch expression, then scans the case list for a matching
    /// value (using [`Self::values_equal`]). Falls through to the `default` case
    /// if present and no value matched. Executes cases sequentially with
    /// fall-through semantics until a `break` (or end of case list) is reached.
    pub(crate) fn execute_switch(
        &mut self,
        expr: &AstNode,
        cases: &[CaseNode],
        location: SourceLocation,
    ) -> Result<(), RuntimeError> {
        self.snapshot_at(location)?;

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

                    if !matches!(self.control_flow, ControlFlow::Normal) {
                        if matches!(self.control_flow, ControlFlow::Break) {
                            self.control_flow = ControlFlow::Normal;
                            self.exit_scope();
                            return Ok(());
                        }

                        // finished, goto_target, should_continue -> propagate
                        self.exit_scope();
                        return Ok(());
                    }

                    if needs_snapshot {
                        self.take_snapshot()?;
                    }
                }
            }
            self.exit_scope();
        }

        Ok(())
    }

    /// Returns `true` if two [`Value`]s compare equal by C semantics.
    ///
    /// `NULL` compares equal to `Pointer(0)` and to itself.
    /// Values of incompatible types (e.g. `Int` vs `Pointer`) are never equal.
    pub(crate) fn values_equal(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Pointer(a), Value::Pointer(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Null, Value::Pointer(0))
            | (Value::Pointer(0), Value::Null) => true,
            _ => false,
        }
    }
}
