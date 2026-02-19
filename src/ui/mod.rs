//! Terminal user interface built on [ratatui](https://github.com/ratatui-org/ratatui).
//!
//! The UI is organized into three layers:
//!
//! - **[`app`]** — application state, keyboard event loop, pane focus, scanf input mode
//! - **[`panes`]** — stateless render functions for each visible pane (source, stack,
//!   heap, terminal, status bar)
//! - **[`theme`]** — centralized color palette used by all panes
//!
//! The entry point for consumers is [`App`]: construct it with an [`Interpreter`] and
//! call [`App::run`] to start the event loop.
//!
//! [`Interpreter`]: crate::interpreter::engine::Interpreter
//! [`App::run`]: app::App::run

pub mod app;
pub mod panes;
pub mod theme;

pub use app::App;
