use crate::interpreter::engine::{ControlFlow, Interpreter};
use crate::interpreter::errors::RuntimeError;
use crate::memory::value::Value;
use crate::parser::ast::{AstNode, CaseNode, SourceLocation};

impl Interpreter {
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
}
