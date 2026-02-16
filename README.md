# CRusTTY

A pedagogical C interpreter with time-travel debugging capabilities, built in Rust with a terminal-based UI.

![CRusTTY Screenshot](crustty.png)

## Overview

CRusTTY is an educational tool for understanding C program execution. It provides:

- **Interactive Execution**: Step through C code line by line
- **Time-Travel Debugging**: Step backward and forward through execution history
- **Memory Visualization**: Real-time view of stack and heap memory
- **Terminal Output**: See printf output as your program runs

## Features

### Supported C Subset

- **Data Types**: `int`, `char`, `void`, structs, pointers, arrays
- **Control Flow**: `if/else`, `while`, `for`, `do-while`, `switch/case`
- **Operators**: Arithmetic, logical, bitwise, comparison, ternary
- **Memory**: Stack-based local variables, dynamic heap allocation via `malloc`/`free`
- **Built-ins**: `printf` (with format specifiers), `malloc`, `free`, `sizeof`

### TUI Interface

The terminal interface provides multiple panes:

- **Source Code**: Syntax-highlighted C code with execution line indicator
- **Stack**: Call stack with local variables and their values
- **Heap**: Dynamic memory allocations with type information
- **Terminal**: Output from `printf` and other I/O operations
- **Status Bar**: Keybindings and execution state

### Keybindings

- `n` / `Space`: Step forward
- `b`: Step backward
- `c`: Continue execution
- `r`: Restart program
- `q`: Quit
- Arrow keys: Navigate through stack/heap panes

## Quick Start

Precompiled binaries for the following platforms will be available in the releases section of the GitHub repository:

- Windows (x86_64 and ARM)
- macOS (x86_64 and ARM)
- Linux (x86_64 and ARM)

Download the appropriate binary for your platform, rename it to `crustty` (or `crustty.exe` on Windows), and add the directory containing the binary to your PATH.

## Usage

```bash
crustty <source.c>
```

Example:

```bash
crustty examples/fibonacci.c
```

## Installation From Source

### Prerequisites

- Rust toolchain (1.70 or later)
- Terminal with Unicode and color support

### Building

```bash
cargo build --release
```

The binary will be available at `target/release/crustty`.

## Project Structure

The codebase is organized into focused modules for maintainability:

```
src/
├── main.rs                     # Entry point and CLI
├── lib.rs                      # Library root
│
├── parser/                     # C parser (source → AST)
│   ├── mod.rs                  # Module documentation
│   ├── lexer.rs                # Tokenization
│   ├── parse.rs                # Parser coordinator
│   ├── declarations.rs         # Struct/function declaration parsing
│   ├── statements.rs           # Statement parsing
│   ├── expressions.rs          # Expression parsing with precedence
│   └── ast.rs                  # AST node definitions
│
├── interpreter/                # C interpreter (AST execution)
│   ├── mod.rs                  # Module documentation
│   ├── engine.rs               # Core interpreter and execution loop
│   ├── statements.rs           # Statement execution
│   ├── expressions.rs          # Expression evaluation
│   ├── builtins.rs             # Built-in functions (printf, malloc, free)
│   ├── memory_ops.rs           # Memory operations and struct field access
│   ├── type_system.rs          # Type inference
│   ├── errors.rs               # Runtime error types
│   └── constants.rs            # Address spaces and limits
│
├── memory/                     # Memory management
│   ├── stack.rs                # Call stack implementation
│   ├── heap.rs                 # Heap allocator
│   └── value.rs                # Value representation
│
├── snapshot/                   # Time-travel debugging
│   └── mod.rs                  # Snapshot management
│
└── ui/                         # Terminal UI (ratatui)
    ├── app.rs                  # Application state and event loop
    ├── theme.rs                # Color scheme
    └── panes/                  # TUI pane rendering
        ├── mod.rs              # Pane module organization
        ├── source.rs           # Source code pane
        ├── stack.rs            # Stack visualization pane
        ├── heap.rs             # Heap visualization pane
        ├── terminal.rs         # Terminal output pane
        ├── status.rs           # Status bar
        └── utils.rs            # Shared rendering utilities
```

## Architecture

### Parser

Hand-written recursive descent parser with:
- Precedence climbing for binary operators
- Comprehensive error reporting with source locations
- Full AST representation of program structure

### Interpreter

Executes the AST with:
- Stack-based execution with call frames
- Heap memory allocator with block tracking
- Snapshot system capturing execution state after each statement
- Specific error types for clear diagnostics

### Memory Model

- **Stack**: Local variables, function parameters, return addresses
  - Address space: `0x7fff_0000` and up
  - Grows downward on function calls
- **Heap**: Dynamic allocations via `malloc`
  - Address space: `0x0000_1000` and up
  - First-fit allocation strategy

### Time-Travel Debugging

The interpreter maintains a history of execution snapshots, each containing:
- Complete stack state
- Complete heap state
- Terminal output
- Current source location

This enables stepping backward through execution to any previous point.

## Performance Optimizations

Recent optimizations include:

- **Inline Hints**: Hot-path functions marked with `#[inline]`
- **Field Caching**: Struct field offsets cached to avoid recomputation
- **Fast Hashing**: `FxHashMap` used for non-cryptographic hashing needs

## Development

### Running Tests

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test '*'

# All tests
cargo test
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Check build
cargo check
```

### Configuration Files

- `rustfmt.toml`: Code formatting rules
- `Cargo.toml`: Dependencies and build configuration

## Limitations

This is NOT a production C interpreter:

- Subset of C (no preprocessor, typedefs, unions, enums, function pointers)
- No optimization or JIT compilation
- Limited standard library (only built-in functions)
- No file I/O or external system interaction
- Fixed memory sizes

## License

MIT License. See `LICENSE.md` file for details.
