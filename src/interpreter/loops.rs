use crate::interpreter::engine::{ControlFlow, Interpreter};
use crate::interpreter::errors::RuntimeError;
use crate::parser::ast::{AstNode, SourceLocation};

pub(crate) enum LoopBodyResult {
    Continue,
    Break,
    Exit,
}

impl Interpreter {
    pub(crate) fn execute_loop_body(
        &mut self,
        body: &[AstNode],
    ) -> Result<LoopBodyResult, RuntimeError> {
        self.enter_scope();
        for stmt in body {
            let needs_snapshot = self.execute_statement(stmt)?;
            if !matches!(self.control_flow, ControlFlow::Normal) {
                if matches!(self.control_flow, ControlFlow::Break) {
                    self.control_flow = ControlFlow::Normal;
                    self.exit_scope();
                    return Ok(LoopBodyResult::Break);
                }
                if matches!(self.control_flow, ControlFlow::Continue) {
                    self.control_flow = ControlFlow::Normal;
                    self.exit_scope();
                    return Ok(LoopBodyResult::Continue);
                }

                // Return, Goto, Finished -> Exit
                self.exit_scope();
                return Ok(LoopBodyResult::Exit);
            }
            if needs_snapshot {
                self.take_snapshot()?;
            }
        }
        self.exit_scope();
        Ok(LoopBodyResult::Continue)
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
                self.snapshot_at(location)?;
                break;
            }

            self.snapshot_at(location)?;

            match self.execute_loop_body(body)? {
                LoopBodyResult::Exit => {
                    self.execution_depth -= 1;
                    return Ok(());
                }
                LoopBodyResult::Break => break 'outer_loop,
                LoopBodyResult::Continue => continue,
            }
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
            self.snapshot_at(location)?;

            match self.execute_loop_body(body)? {
                LoopBodyResult::Exit => {
                    self.execution_depth -= 1;
                    return Ok(());
                }
                LoopBodyResult::Break => break 'outer_loop,
                LoopBodyResult::Continue => {}
            }

            let cond_val = self.evaluate_expr(condition)?;
            let cond_bool = Self::value_to_bool(&cond_val, location)?;

            if !cond_bool {
                self.snapshot_at(location)?;
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
                    self.snapshot_at(location)?;
                    break;
                }
            }

            self.snapshot_at(location)?;

            match self.execute_loop_body(body)? {
                LoopBodyResult::Exit => {
                    self.exit_scope(); // Exit loop scope
                    self.execution_depth -= 1;
                    return Ok(());
                }
                LoopBodyResult::Break => break 'outer_loop,
                LoopBodyResult::Continue => {}
            }

            if let Some(inc) = increment {
                self.evaluate_expr(inc)?;
            }
        }
        self.execution_depth -= 1;
        self.exit_scope(); // Exit loop scope

        Ok(())
    }
}
