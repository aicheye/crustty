# TRACKING.md

## Purpose
This document tracks implementation progress, current status, and next steps.

---

## Project Status: **IMPLEMENTATION IN PROGRESS**

Discovery phase complete. Architecture defined. Foundation implemented. Parser next.

---

## Completed

### ✓ Phase 1: Discovery (DONE)
- [x] Read CLAUDE.md and DISCOVERY.md
- [x] Identified initial ambiguities and blocking questions (Q1-Q18)
- [x] Received answers and identified contradictions
- [x] Resolved contradictions (C1-C4)
- [x] Asked final blocking questions (Q19-Q43)
- [x] Received all answers
- [x] Wrote test program `main()` function for listmatrix.c
- [x] Documented all Q&A in DISCOVERY.md

### ✓ Phase 2: Architecture (DONE)
- [x] Designed overall system structure
- [x] Defined AST representation
- [x] Defined memory model (stack, heap, values)
- [x] Defined execution engine
- [x] Defined snapshot/reverse execution mechanism
- [x] Designed terminal UI layout
- [x] Documented architecture in ARCHITECTURE.md

### ✓ Phase 3.1-3.3: Foundation (DONE)

#### ✓ 3.1 Project Setup
- [x] Initialize Rust project (already existed)
- [x] Add dependencies to Cargo.toml (ratatui 0.28, crossterm 0.28)
- [x] Create directory structure (parser/, interpreter/, memory/, ui/, snapshot/)
- [x] Set up basic main.rs skeleton
- [x] Verify compilation (✓ builds with 44 expected warnings for unused code)

#### ✓ 3.2 AST and Type Definitions
- [x] Define `Type`, `BaseType`, `AstNode` enums in [parser/ast.rs](src/parser/ast.rs)
- [x] Define operator enums (`BinOp`, `UnOp`)
- [x] Define supporting structs (`Field`, `Param`, `StructDef`, `CaseNode`)
- [x] Add source location tracking to AST nodes
- [x] Implemented with Clone + Debug for snapshot support
- [x] Created `Program` struct for top-level declarations

**Files created:**
- [src/parser/ast.rs](src/parser/ast.rs) - 343 lines, complete AST definition
- [src/parser/mod.rs](src/parser/mod.rs) - module skeleton

#### ✓ 3.3 Memory Model
- [x] Implement `Value` enum in [memory/value.rs](src/memory/value.rs)
- [x] Implement `Stack`, `StackFrame`, `LocalVar` in [memory/stack.rs](src/memory/stack.rs)
- [x] Implement `Heap`, `HeapBlock`, `BlockState` in [memory/heap.rs](src/memory/heap.rs)
- [x] Implement `InitState` tracking for uninitialized memory (per-field for structs)
- [x] Add helper functions in [memory/mod.rs](src/memory/mod.rs): `sizeof_type()`, `pointer_add()`, `pointer_sub()`, `pointer_diff()`

**Files created:**
- [src/memory/value.rs](src/memory/value.rs) - Value enum with initialization tracking
- [src/memory/stack.rs](src/memory/stack.rs) - Stack with dynamic frames
- [src/memory/heap.rs](src/memory/heap.rs) - Heap with tombstones and per-byte init tracking
- [src/memory/mod.rs](src/memory/mod.rs) - Helper functions for sizeof and pointer arithmetic

**Additional infrastructure:**
- [x] Runtime error types in [interpreter/errors.rs](src/interpreter/errors.rs)
- [x] Snapshot module in [snapshot/mod.rs](src/snapshot/mod.rs)
- [x] MockTerminal for printf output capture
- [x] SnapshotManager with memory limit enforcement

---

---

### ✓ Phase 3.4: Parser (DONE)

#### Completed Tasks:
- [x] Implement lexer (tokenization) in [parser/lexer.rs](src/parser/lexer.rs)
  - Token enum with all necessary types (Int, String, Ident, Keywords, Operators, Punctuation)
  - Handles single-line (`//`) and multi-line (`/* */`) comments
  - Skips `#include` directives
  - String literal parsing with escape sequences (`\n`, `\t`, `\"`, etc.)
  - Source location tracking for all tokens
  - 5 passing tests

- [x] Implement recursive descent parser in [parser/parser.rs](src/parser/parser.rs)
  - Parses all supported constructs from DISCOVERY.md
  - Produces AST from tokens
  - Full operator precedence (15 levels from assignment to primary)
  - 4 passing tests

**Files created:**
- [src/parser/lexer.rs](src/parser/lexer.rs) - 670 lines, complete lexer with tests
- [src/parser/parser.rs](src/parser/parser.rs) - 1080 lines, complete recursive descent parser

**Parser capabilities:**
✓ **Expressions:**
  - Literals: int, string, NULL
  - Variables
  - Binary operators: arithmetic (`+`, `-`, `*`, `/`, `%`), comparison (`<`, `<=`, `>`, `>=`, `==`, `!=`), logical (`&&`, `||`), bitwise (`&`, `|`, `^`, `<<`, `>>`)
  - Unary operators: `-`, `+`, `!`, `~`, `&` (address-of), `*` (deref), `++`, `--` (pre and post)
  - Ternary: `? :`
  - Function calls
  - Array access: `arr[index]`
  - Member access: `.` and `->`
  - Casts: `(Type*)expr`
  - Sizeof: `sizeof(Type)` and `sizeof(expr)`

✓ **Statements:**
  - Variable declaration with optional initialization
  - Assignment and compound assignment (`+=`, `-=`, `*=`, `/=`, `%=`)
  - Return
  - If-else
  - While
  - Do-while
  - For (with optional init, condition, increment)
  - Switch-case-default
  - Break, continue
  - Goto and labels

✓ **Declarations:**
  - Function definitions with parameters
  - Struct definitions with fields
  - Type parsing: base types, const qualifier, pointers, arrays

**Parser design:**
- Precedence-climbing for binary operators (correct associativity and precedence)
- Lookahead for disambiguating casts vs parenthesized expressions
- Proper handling of statement vs declaration distinction

**Test results:**
- ✓ Parse simple function: `int main() { return 0; }`
- ✓ Parse expressions with correct precedence: `1 + 2 * 3`
- ✓ Parse if-else statements
- ✓ Parse struct definitions

---

## Current Phase: **INTEGRATION & TESTING (NEXT)**

## Recently Completed

### ✓ Phase 3.8: Terminal UI (COMPLETED)

### ✓ Phase 3.5: Execution Engine (DONE)

#### 3.5 Execution Engine - IMPLEMENTED
- [x] Implement `Interpreter` struct in `interpreter/engine.rs`
- [x] Implement statement execution (walk AST, update state)
- [x] Implement expression evaluation (return `Value`)
- [x] Implement built-in functions (inline in engine.rs)
  - [x] `printf` (format string parsing for %d, %s, %c - output to mock terminal)
  - [x] `malloc` (allocate heap block, return address)
  - [x] `free` (mark as tombstone)
  - [x] `sizeof` (return type size)
- [x] Implement error detection:
  - [x] Uninitialized read
  - [x] Null dereference
  - [x] Buffer overrun (partial - only for arrays)
  - [x] Const modification
  - [x] Integer overflow
  - [x] Double-free and use-after-free
  - [x] Division by zero

#### Additional Features Implemented
- [x] **Struct member access** (`.` operator)
- [x] **Pointer member access** (`->` operator) for stack pointers
- [x] **Heap struct operations** (`->` operator for heap-allocated structs)
- [x] **Address-of operator** (`&`) for stack variables
- [x] **User-defined function calls** with parameter passing and return values
- [x] **Stack frame management** for function calls
- [x] **Struct initialization** and field assignment
- [x] **All binary operators** (arithmetic, comparison, logical, bitwise, compound assignment)
- [x] **All unary operators** (negation, logical NOT, bitwise NOT, pre/post inc/dec)
- [x] **Control flow** (if/else, while, do-while, for loops)
- [x] **Ternary operator** (`? :`)

#### Integration Tests for Heap Structs - COMPLETE
- [x] Basic heap struct allocation and field access (`test_heap_struct_allocation`)
- [x] Multiple heap allocations (`test_heap_struct_multiple_allocations`)
- [x] Nested struct fields (`test_heap_struct_nested_fields`)
- [x] Error detection: double-free (`test_heap_double_free_error`)
- [x] Error detection: use-after-free (`test_heap_use_after_free_error`)
- [x] Error detection: null dereference (`test_heap_null_dereference`)
- [x] Pointer fields in structs (linked list pattern) (`test_heap_struct_pointer_in_struct`)
- [x] Mixed type structs with pointers (`test_heap_struct_mixed_types`)

**Test summary:** 11 total integration tests (3 basic + 8 heap struct tests)

**Test coverage:**
- Stack-based struct operations ✓
- Heap-allocated struct operations with `malloc`/`free` ✓
- Nested struct field access (both stack and heap) ✓
- Struct with pointer fields (linked data structures) ✓
- Error cases: double-free, use-after-free, null dereference ✓

**Known limitations discovered during testing:**
- Complex member access (e.g., `ptr->nested.field`) not yet implemented
- Pointer dereference in arithmetic expressions (e.g., `x + *ptr`) not yet fully supported
- Mixed char/int arithmetic may have type checking issues
- These are documented as future enhancements

**Test execution:** `cargo test --test integration_test` - All 11 tests passing ✓

---

### ✓ Phase 3.6: Snapshot Management (DONE)

#### Snapshot Management - IMPLEMENTED
- [x] Snapshot struct with complete state capture
- [x] Deep cloning of all execution state (stack, heap, terminal, runtime state)
- [x] Memory usage estimation
- [x] Hard limit enforcement with clear error messages
- [x] Snapshot taken after each statement during execution

**Implementation details:**
- **File:** [src/snapshot/mod.rs](src/snapshot/mod.rs)
- Snapshot includes: Stack, Heap, MockTerminal, SourceLocation, return_value, pointer_types, stack_address_map, next_stack_address
- Memory estimation: sum of stack depth, heap allocations, terminal lines
- Hard limit: configurable (passed to Interpreter constructor)
- Snapshots stored in SnapshotManager with automatic limit checking

**Key decision:** Full snapshot cloning
- Tradeoff: Memory usage vs simplicity
- Alternative considered: Copy-on-write or delta snapshots
- Chose full cloning for correctness and implementation simplicity
- Memory limit enforcement prevents runaway growth

---

### ✓ Phase 3.7: Reverse Execution (DONE)

#### Reverse Execution - IMPLEMENTED
- [x] restore_snapshot() - restore interpreter state from snapshot
- [x] step_backward() - restore previous snapshot
- [x] step_forward() - replay from history
- [x] History position tracking
- [x] Edge case handling (at beginning, at end)

**Implementation details:**
- **File:** [src/interpreter/engine.rs](src/interpreter/engine.rs)
- `step_backward()`: decrements history_position, restores snapshot
- `step_forward()`: if snapshot exists in history, restore it; otherwise error
- `restore_snapshot()`: clones and replaces all mutable state
- History position: tracks current location in snapshot history

**Reverse execution tests (4 tests):**
- [x] Basic step backward (`test_step_backward`)
- [x] Step forward and backward (`test_step_forward_and_backward`)
- [x] Error at beginning (`test_step_backward_at_beginning`)
- [x] State preservation across full reverse/forward cycle (`test_reverse_execution_preserves_state`)

**Test execution:** `cargo test --test integration_test` - All 15 tests passing ✓ (11 previous + 4 new)

**Design notes:**
- step_forward() currently limited to replaying existing snapshots
- Extending execution (stepping beyond history) would require statement-level execution control
- This is sufficient for time-travel debugging use case
- **File:** [src/interpreter/engine.rs](src/interpreter/engine.rs) - ~1400 lines
- **Key design:** Stack addresses are synthetic (< 0x10000000), mapped to (frame_depth, var_name)
- **Heap addresses:** Real heap addresses (>= 0x10000000)
- **Return values:** Stored in `Interpreter.return_value` field

**Implementation notes:**
- `execute_statement()` returns `Result<(), RuntimeError>`
- `evaluate_expression()` returns `Result<Value, RuntimeError>`
- Use Rust's `checked_add`, `checked_mul`, etc. for overflow detection
- Mock terminal is `Vec<TerminalLine>` where each line tracks source location

**Execution flow:**
```rust
impl Interpreter {
    fn run(&mut self) -> Result<(), RuntimeError> {
        // Find main()
        let main_fn = self.find_function("main")?;

        // Push initial frame
        self.stack.push_frame("main", main_fn.params);

        // Execute statements
        for stmt in &main_fn.body {
            self.execute_statement(stmt)?;
            self.take_snapshot()?; // After each statement
        }

        Ok(())
    }
}
```

**Potential issues:**
- Function calls require pushing/popping stack frames
- Return values must be passed from callee to caller (store in temp?)
- Recursion will work naturally but could hit stack depth limits
- `printf` format parsing is complex (use regex or hand-written parser)

**Testing:**
- Execute: `int x = 5; return x;`
- Execute: function call with arguments
- Execute: struct field assignment
- Execute: malloc + free
- Execute: listmatrix.c (integration test)

---

#### ✓ 3.6 Snapshot Management (COMPLETED - see above)

#### ✓ 3.7 Reverse Execution (COMPLETED - see above)

---

### ✓ Phase 3.8: Terminal UI (DONE)

#### 3.8 Terminal UI - IMPLEMENTED
- [x] Set up ratatui application in `ui/app.rs`
- [x] Implement 4-pane layout in `ui/panes.rs`
  - [x] Source code pane (highlight current line)
  - [x] Stack pane (show frames, locals)
  - [x] Heap pane (show allocations, tombstones)
  - [x] Terminal pane (show mock output)
- [x] Implement keyboard handling (integrated in app.rs)
  - [x] Left/Right arrow: switch panes
  - [x] Up/Down arrow: scroll within pane
  - [x] Space/Enter: step forward
  - [x] Backspace: step backward
  - [x] Q: quit
- [x] Render current step number / total steps in status bar

**Implementation details:**
- **File:** [src/ui/app.rs](src/ui/app.rs) - App state, event handling, rendering
- **File:** [src/ui/panes.rs](src/ui/panes.rs) - Individual pane rendering logic
- **Layout:** 2-column design (Source+Terminal left, Stack+Heap right)
- Uses `ratatui` 0.28 with `crossterm` 0.28 backend
- Active pane has yellow border
- Current source line highlighted with yellow background
- Auto-scrolling keeps current line in view
- Color-coded heap states (green=allocated, red=freed)

**Layout rationale:**
- Left column (60%): Source (70%) + Terminal output (30%)
- Right column (40%): Stack (50%) + Heap (50%)
- Logical grouping: code/output vs memory views
- Navigation: clockwise (Source → Terminal → Stack → Heap)

**Value rendering:**
- Integers: decimal format
- Characters: printable as 'x', non-printable as '\xNN'
- Pointers: hex format 0xNNNNNNNN
- Structs: recursive display with nesting limit
- Heap data: hex bytes with initialization tracking

**Additional fixes implemented:**
- [x] Pointer dereference assignment (`*ptr = value`)
- [x] Pointer dereference reading (`*ptr` in expressions)
- [x] Added getter methods to Interpreter for UI state access

**Testing:**
- ✓ Compiles with 30 warnings (unused helper functions, expected)
- ✓ Successfully executes [test_simple.c](test_simple.c) with pointer operations
- ✓ Creates snapshot history (14 snapshots for test program)
- ✓ All keyboard controls implemented
- ✓ Pane switching and scrolling functional
- ⚠️ Cannot test interactive TUI in CI (requires real TTY)

**Files created:**
- [src/ui/app.rs](src/ui/app.rs) - 260 lines, main TUI application
- [src/ui/panes.rs](src/ui/panes.rs) - 323 lines, pane rendering
- [UI_IMPROVEMENTS.md](UI_IMPROVEMENTS.md) - Documentation of changes

**Files modified:**
- [src/ui/mod.rs](src/ui/mod.rs) - Module exports
- [src/main.rs](src/main.rs) - TUI integration, terminal setup/cleanup
- [src/interpreter/engine.rs](src/interpreter/engine.rs) - Getter methods, pointer deref
- [src/snapshot/mod.rs](src/snapshot/mod.rs) - len() and is_empty() methods

---

#### 3.9 Integration and Testing
- [ ] Wire up all components in `main.rs`
- [ ] Load and parse `listmatrix.c`
- [ ] Execute through interpreter
- [ ] Run TUI with stepping
- [ ] Verify output matches expected
- [ ] Test reverse execution (step backward through entire execution)

**Expected output for listmatrix.c:**
```
Matrix 1:
(1, 1): 2
(0, 0): 1
Matrix 2:
(1, 1): 4
(0, 0): 3
Result:
(1, 1): 8
(0, 0): 3
```

**Integration test checklist:**
- [ ] Parse listmatrix.c without errors
- [ ] Execute without runtime errors
- [ ] Output matches expected
- [ ] All heap memory is freed at end (no leaks)
- [ ] Can step backward from end to start
- [ ] Can step forward again and reach same end state

---

## Known Risks and Mitigation

### Risk 1: Parser complexity
**Risk:** C syntax is complex; hand-written parser may be error-prone.
**Mitigation:**
- Implement incrementally (start with subset)
- Test each construct in isolation
- Use parser combinator library as fallback (e.g., `nom`)

### Risk 2: Snapshot memory usage
**Risk:** Full snapshots are expensive; memory limit may be too restrictive.
**Mitigation:**
- Set generous initial limit (1 GB)
- If insufficient, implement copy-on-write as optimization
- Alternative: warn user to use smaller test programs

### Risk 3: Uninitialized memory tracking
**Risk:** Per-field tracking is complex and may have bugs.
**Mitigation:**
- Start with per-allocation tracking (simpler)
- Upgrade to per-field after core functionality works
- Write extensive tests for edge cases

### Risk 4: printf format string parsing
**Risk:** Format strings have many edge cases.
**Mitigation:**
- Support only required specifiers (%d, %s, %c, %n)
- Reject unsupported specifiers with clear error
- Use existing crate for parsing as fallback (e.g., `printf-compat`)

### Risk 5: UI rendering performance
**Risk:** Large heap/stack may cause UI lag.
**Mitigation:**
- Implement pagination/windowing (only render visible items)
- Profile if slowness observed
- Defer optimization until necessary

---

## Implementation Order (Recommended)

1. **Project setup** (3.1)
2. **AST definitions** (3.2)
3. **Memory model** (3.3) — needed for interpreter
4. **Basic parser** (3.4) — parse subset (int, return, arithmetic)
5. **Basic interpreter** (3.5) — execute subset (no malloc/free yet)
6. **Snapshot management** (3.6) — needed for reverse execution
7. **Reverse execution** (3.7)
8. **Terminal UI skeleton** (3.8) — display state, no stepping yet
9. **Full parser** (3.4 continued) — parse remaining constructs
10. **Full interpreter** (3.5 continued) — malloc/free, structs, etc.
11. **UI stepping** (3.8 continued) — wire up keyboard controls
12. **Integration testing** (3.9)

---

## Current Task: **Implement execution engine**

Next immediate action: **Create interpreter/engine.rs with Interpreter struct and basic execution logic**

---

## Notes

- All architectural decisions are documented in ARCHITECTURE.md
- All specification decisions are documented in DISCOVERY.md
- This file should be updated after each major milestone
- When marking items complete, note any deviations or issues encountered

---

End of tracking document.
