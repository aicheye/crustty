//! Loop statement execution (`while`, `do-while`, `for`).
//!
//! Adds `impl Interpreter` methods for the three loop forms supported by the
//! C subset. `break` and `continue` are propagated via `LoopBodyResult` so
//! the loop driver can react without inspecting `control_flow` directly.
//!
//! `goto` and `return` inside a loop body are handled by returning
//! `LoopBodyResult::Exit`, which causes the loop to unwind immediately and
//! let the outer execution context propagate the control-flow signal.

use crate::interpreter::engine::{ControlFlow, Interpreter};
use crate::interpreter::errors::RuntimeError;
use crate::parser::ast::{AstNode, SourceLocation};

/// Result returned by [`Interpreter::execute_loop_body`] to signal how the body ended.
pub(crate) enum LoopBodyResult {
    /// Body completed normally or via `continue` — the loop should iterate again.
    Continue,
    /// `break` was encountered — the loop should exit cleanly.
    Break,
    /// `return`, `goto`, or another non-loop control flow was triggered —
    /// the loop driver should unwind and propagate `self.control_flow` to the caller.
    Exit,
}

impl Interpreter {
    /// Executes all statements in `body` inside a fresh scope.
    ///
    /// Returns [`LoopBodyResult::Continue`] if the body ran to completion or hit
    /// `continue`, [`LoopBodyResult::Break`] on `break`, and
    /// [`LoopBodyResult::Exit`] for any other control-flow signal (`return`, `goto`).
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

    /// Executes a `while (condition) { body }` loop.
    ///
    /// The condition is evaluated before each iteration. A snapshot is taken at
    /// `location` both when the condition is true (before executing the body) and
    /// when it first becomes false (loop exit point).
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

    /// Executes a `do { body } while (condition)` loop.
    ///
    /// The body always runs at least once; the condition is checked after each
    /// iteration.
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

    /// Executes a `for (init; condition; increment) { body }` loop.
    ///
    /// `init`, `condition`, and `increment` are all optional, matching C semantics.
    /// A missing condition is treated as always-true. The initializer and loop
    /// variable share a single scope that is exited when the loop ends.
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
