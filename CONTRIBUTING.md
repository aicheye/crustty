# Contributing to CRusTTY

Thank you for your interest in contributing. CRusTTY is a pedagogical C
interpreter — contributions that improve correctness, add educational value, or
make the tool easier to understand are especially welcome.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Architecture Overview](#architecture-overview)
- [Coding Conventions](#coding-conventions)
- [Testing](#testing)
- [Areas Open for Contribution](#areas-open-for-contribution)
- [Submitting a Pull Request](#submitting-a-pull-request)

---

## Code of Conduct

Be respectful and constructive. Criticism of code is welcome; criticism of
people is not.

---

## Getting Started

### Prerequisites

- Rust toolchain (stable, 1.70 or later). Install via [rustup](https://rustup.rs/).
- A terminal with Unicode and true-color support (for running the TUI manually).

### Clone and build

```bash
git clone https://github.com/aicheye/crustty.git
cd crustty
cargo build
```

### Run the bundled example

```bash
./target/debug/crustty default
```

### Run your own C file

```bash
./target/debug/crustty path/to/file.c
```

---

## Development Workflow

### Pre-commit hooks

The repository ships pre-commit hooks that run automatically on every commit:

```bash
cargo fmt          # auto-format
cargo clippy -- -D warnings
cargo test
cargo check
cargo doc --no-deps --all-features
```

Make sure all four pass before pushing. Install the hooks with:

```bash
pre-commit install
```

Alternatively, you can run them manually at any time:

```bash
pre-commit run --all-files
```

### Useful commands

```bash
# Run all tests
cargo test

# Run only integration tests
cargo test --test '*'

# Run only unit tests
cargo test --lib

# Show stdout during tests (useful for debugging interpreter output)
cargo test -- --nocapture

# Generate and open API documentation
cargo doc --open
```

---

## Architecture Overview

Understanding the execution pipeline helps you find the right place for a change.

```markdown
Source text
    │
    ▼
Lexer (src/parser/lexer.rs)
    │  Token stream
    ▼
Parser (src/parser/)
    │  Program (AST)
    ▼
Interpreter (src/interpreter/)
    │  Snapshot list
    ▼
TUI (src/ui/)
```

### Key modules

| Module | Purpose |
|--------|---------|
| `parser/lexer.rs` | Converts source text to a `Token` stream. Preprocessor directives are silently skipped. |
| `parser/ast.rs` | All AST node types (`AstNode`, `BinOp`, `Type`, …). |
| `parser/parse.rs` | `Parser` struct — entry point for parsing a full `Program`. |
| `interpreter/engine.rs` | `Interpreter` struct — `run()`, `step_forward()`, `step_backward()`, snapshot management. |
| `interpreter/expressions.rs` | Expression evaluation (~38 KB, the hot path). |
| `interpreter/ops/` | Operator helpers split by class: `binary`, `unary`, `assign`, `access`, `structs`. |
| `interpreter/loops.rs` | `while`, `do-while`, `for` execution. |
| `interpreter/jumps.rs` | `return` and `switch` execution. |
| `interpreter/builtins.rs` | `printf`, `scanf`, `malloc`, `free`, `sizeof`. |
| `memory/` | `Value` enum, stack frames (`stack.rs`), heap allocator (`heap.rs`). |
| `snapshot/mod.rs` | `Snapshot` (full state clone), `SnapshotManager` (history with memory cap). |
| `ui/app.rs` | `App` — ratatui event loop, keyboard handling, pane focus, scanf input mode. |
| `ui/panes/` | One stateless render function per pane. |
| `ui/theme.rs` | Central color palette (`DEFAULT_THEME`). |

### Adding a new built-in function

1. Add a handler method `fn builtin_<name>` in `src/interpreter/builtins.rs`.
2. Dispatch to it from `Interpreter::call_builtin` in `engine.rs` (or wherever
   built-in dispatch lives).
3. Document the format specifiers / arguments in the `builtins.rs` module doc
   (`//!` comment at the top of the file).
4. Add an integration test in `tests/integration_test.rs`.

### Adding a new C language feature

1. **Lexer** — add any new tokens to `Token` in `lexer.rs`.
2. **Parser** — add grammar rules in the appropriate sub-parser
   (`declarations.rs`, `statements.rs`, or `expressions.rs`).
3. **AST** — add a new `AstNode` variant (and any supporting types) in `ast.rs`.
4. **Interpreter** — handle the new variant in `engine.rs` /
   `interpreter/statements.rs` / `interpreter/expressions.rs`.
5. **Tests** — add a regression test in `tests/`.

---

## Coding Conventions

- **Hash maps**: Use `FxHashMap` / `FxHashSet` from `rustc-hash` everywhere.
  Never use `std::collections::HashMap`.
- **Line width**: 100 characters maximum (`rustfmt.toml` enforces this).
- **Clippy**: All warnings are errors (`-D warnings`). Fix them, don't suppress
  them unless there is a documented reason.
- **`pub(crate)` by default**: Interpreter internals should be `pub(crate)`, not
  `pub`. Only types that form part of the library's external API should be `pub`.
- **Doc comments**: Every public type and every public/`pub(crate)` function
  should have a `///` doc comment explaining what it does. Module-level `//!`
  comments explain the purpose of the module and list its main types/functions.
- **No `unwrap()` in production paths**: Return a `RuntimeError` instead. Reserve
  `unwrap()` / `expect()` for cases that are provably unreachable or in tests.
- **Snapshots are cheap to take, expensive to store**: Prefer capturing more
  snapshots for better TUI granularity; the `SnapshotManager` will evict old
  ones if the memory limit is hit.

---

## Testing

Tests live in two places:

| Location | Purpose |
|----------|---------|
| `tests/integration_test.rs` | End-to-end tests: parse + execute a C snippet, assert on output / error |
| `tests/arithmetic_tests.rs` | Focused arithmetic and type-coercion tests |
| `src/**/*.rs` (inline `#[cfg(test)]`) | Unit tests for individual functions |

### Writing an integration test

```rust
#[test]
fn test_my_feature() {
    let source = r#"
        int main() {
            // your C code here
            return 0;
        }
    "#;

    let mut interpreter = run_program(source).expect("should not fail");

    let output: Vec<_> = interpreter
        .terminal()
        .get_output()
        .into_iter()
        .filter_map(|(s, kind)| {
            (kind == crustty::snapshot::TerminalLineKind::Output).then_some(s)
        })
        .collect();

    assert_eq!(output, vec!["expected output"]);
}
```

The `run_program` helper is defined in `tests/integration_test.rs` — import it
or copy the pattern for new test files.

---

## Areas Open for Contribution

The following items from `TRACKING.md` are good starting points:

### Language / interpreter

- **Nested pointer member access** (`ptr->nested.field`) — currently incomplete.
- **Backward `goto`** — forward-only `goto` is supported; backward jumps are not.
- **`printf` width/precision modifiers** — `%5d`, `%.2f`, etc.
- **Additional built-ins** — `memset`, `memcpy`, `strcpy`, `strlen`.
- **Strict type checking** for assignments and function call arguments.

### TUI / UX

- **Pagination** for the Stack and Heap panes — currently all items are rendered
  at once, which can be slow for large programs.
- **Auto-scroll** to the active source line when stepping backward.
- **`memset`/`memcpy` visualization** in the Heap pane.

### Unit / integration tests

- **Edge-case coverage**: zero-size `malloc`, stack overflow detection,
  deep/cyclic struct references.
- **Full walkthrough test** for `examples/default.c`.

### Performance / architecture

- **Copy-on-write snapshots**: Instead of cloning the full stack and heap on
  every statement, track diffs and reconstruct on demand.
- **Snapshot save/load**: Serialize an execution trace to disk for later replay.

---

## Submitting a Pull Request

1. Fork the repository and create a branch from `master`.
2. Make your changes. Keep commits focused — one logical change per commit.
3. Ensure `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` all pass.
4. Open a pull request against `master`. Describe:
   - **What** the change does
   - **Why** it is needed (link to an issue if one exists)
   - **How** you tested it (C snippets, test cases, or manual TUI verification)
5. Address review feedback and push additional commits to the same branch.

---

For questions or discussion, open a GitHub issue.
