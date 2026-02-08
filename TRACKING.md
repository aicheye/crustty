# Master TODO & Incomplete Features

This document tracks all incomplete features, known limitations, and remaining tasks for the Crustty C Interpreter.

## 🔴 Critical Missing Features / Bugs
- [ ] **Complex Member Access**: Implementation needed for nested field access via pointers (e.g., `ptr->nested.field`).
- [ ] **Pointer Arithmetic**: Full support for mixing pointers and dereferences in arithmetic expressions (e.g., `x + *ptr`).
- [ ] **Type Checking**:
    - [ ] Mixed `char`/`int` arithmetic behavior needs verification.
    - [ ] Strict type checking for assignments and function calls.
- [ ] **Printf Support**:
    - [ ] Current support limited to `%d`, `%s`, `%c`.
    - [ ] Missing width/precision modifiers.
- [ ] **Missing Files**: `listmatrix.c` is referenced in documentation/tests but missing from the repository.

## 🟡 TUI & Usability Improvements
- [ ] **Performance**:
    - [ ] Implement pagination/windowing for Stack and Heap panes (currently renders all items, potentially slow).
    - [ ] Optimize large memory snapshot rendering.
- [ ] **Display Limits**:
    - [ ] Handle deep or cyclic struct references in variables view (verify recursion limits).
- [ ] **Input Handling**:
    - [ ] Verify scrolling behavior when stepping backwards (auto-scroll to active line).

## 🟢 Integration & Polish
- [ ] **CLI/Main**:
    - [ ] Ensure `main.rs` gracefully transitions to TUI.
    - [ ] Better error reporting for runtime errors without TUI mode.
- [ ] **Standard Library**:
    - [ ] Add `memset`, `memcpy`, `strcpy` built-ins.
    - [ ] Support `NULL` macro properly in all contexts.

## 🧪 Testing & Verification
- [ ] **CI Limitations**: Tests requiring TTY/Interactive TUI cannot run in CI.
- [ ] **Integration Scenarios**:
    - [ ] Re-create `listmatrix.c` and verify expected output.
    - [ ] Full run-through of `examples/default.c` checking for memory leaks.
- [ ] **Edge Cases**:
    - [ ] Stack overflow detection (recursion limit).
    - [ ] Zero-size allocations.

## 🧊 Backlog / Future
- [ ] **Parser**: Switch to `nom` or similar if hand-written parser becomes unmaintainable.
- [ ] **Snapshot Optimization**: Implement copy-on-write instead of full cloning for memory efficiency.
- [ ] **Save/Load**: Capability to save execution trace to file.
