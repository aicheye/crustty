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
- **Built-ins**: `printf` and `scanf` (with format specifiers), `malloc`, `free`, `sizeof`

### TUI Interface

The terminal interface provides multiple panes:

- **Source Code**: Syntax-highlighted C code with execution line indicator
- **Stack**: Call stack with local variables and their values
- **Heap**: Dynamic memory allocations with type information
- **Terminal**: Output from `printf` and input prompts from `scanf`
- **Status Bar**: Keybindings and execution state

### Keybindings

- `n` / `Space`: Step forward
- `b`: Step backward
- `c`: Continue execution
- `r`: Restart program
- `q`: Quit
- `esc`: Exit input mode (in `scanf` input prompt)
- Arrow keys: Navigate through stack/heap panes

## Quick Start

Precompiled binaries for the following platforms will be available in the releases section of the GitHub repository:

- Windows (x86_64 and ARM)
- macOS (x86_64 and ARM)
- Linux (x86_64 and ARM)

Download the appropriate binary for your platform, rename it to `crustty` (or `crustty.exe` on Windows), and add the directory containing the binary to your PATH.

## Usage

```bash
crustty <source.c | example_name>
```

Examples:

```bash
# Run the bundled comprehensive example (features demo)
crustty default

# Run your own C file
crustty path/to/your/file.c
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

```markdown
src/
├── main.rs                     # Entry point and CLI argument handling
├── lib.rs                      # Library root and public API
│
├── parser/                     # C parser (source → AST)
│   ├── mod.rs                  # Module overview
│   ├── lexer.rs                # Tokenizer: source text → Token stream
│   ├── parse.rs                # Parser entry point (Parser struct)
│   ├── declarations.rs         # Struct/function declaration parsing
│   ├── statements.rs           # Statement parsing
│   ├── expressions.rs          # Expression parsing with precedence climbing
│   └── ast.rs                  # AST node type definitions
│
├── interpreter/                # C interpreter (AST → execution)
│   ├── mod.rs                  # Module overview and execution model docs
│   ├── engine.rs               # Interpreter struct, run(), rewind(), snapshots
│   ├── statements.rs           # Statement dispatch (if, while, decl, …)
│   ├── expressions.rs          # Expression evaluation (largest file)
│   ├── builtins.rs             # Built-in functions (printf, scanf, malloc, free)
│   ├── type_system.rs          # Type inference helpers
│   ├── loops.rs                # while / do-while / for loop execution
│   ├── jumps.rs                # return / switch execution
│   ├── heap_serial.rs          # Value ↔ heap byte serialization
│   ├── errors.rs               # RuntimeError enum
│   ├── constants.rs            # Address-space constants
│   └── ops/                    # Operator implementations (impl Interpreter)
│       ├── mod.rs              # Submodule declarations
│       ├── binary.rs           # Arithmetic, comparison, bitwise, compound-assign
│       ├── unary.rs            # Negation, NOT, increment/decrement
│       ├── assign.rs           # Assignment to lvalues
│       ├── access.rs           # Reading lvalues and resolving addresses
│       └── structs.rs          # Struct initialization and field-offset helpers
│
├── memory/                     # Runtime memory model
│   ├── mod.rs                  # sizeof, pointer arithmetic helpers
│   ├── stack.rs                # Call frames and local variables
│   ├── heap.rs                 # First-fit heap allocator
│   └── value.rs                # Value enum (Int, Char, Pointer, Struct, …)
│
├── snapshot/                   # Time-travel debugging
│   └── mod.rs                  # Snapshot, SnapshotManager, MockTerminal
│
└── ui/                         # Terminal UI (ratatui + crossterm)
    ├── mod.rs                  # Module re-exports
    ├── app.rs                  # App struct, event loop, pane focus, scanf input
    ├── theme.rs                # Color palette (DEFAULT_THEME)
    └── panes/                  # Stateless pane render functions
        ├── mod.rs              # Re-exports for all pane modules
        ├── source.rs           # Syntax-highlighted source code pane
        ├── stack.rs            # Call stack visualization pane
        ├── heap.rs             # Heap block visualization pane
        ├── terminal.rs         # printf / scanf terminal output pane
        ├── status.rs           # Status bar (keybindings, step counter)
        └── utils/              # Shared rendering helpers
            ├── mod.rs          # Re-exports from submodules
            ├── formatting.rs   # Value/address formatting
            ├── memory.rs       # Stack/heap data extraction helpers
            └── rendering.rs    # Low-level ratatui span/line builders
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

The interpreter code is split across focused submodules: `engine.rs` owns the core
loop and public API; operator evaluation lives under `ops/`; loop control flow in
`loops.rs`; jump-style control flow (`return`, `switch`) in `jumps.rs`; and heap
serialization in `heap_serial.rs`.

### Memory Model

- **Stack**: Local variables, function parameters, return addresses
  - Address space: `0x0000_0004` and up (sequential variable IDs per frame)
- **Heap**: Dynamic allocations via `malloc`
  - Address space: `0x7fff_0000` and up
  - First-fit allocation strategy

The two regions occupy non-overlapping address ranges so the TUI can distinguish stack and heap pointers without type annotation, and pointer arithmetic can be range-checked cheaply.

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
