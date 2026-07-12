# Master TODO & Incomplete Features

This document tracks all incomplete features, known limitations, and remaining tasks for the Crustty C Interpreter.

## 🔴 Critical Missing Features / Bugs

- [ ] **Complex Member Access**: Implementation needed for nested field access via pointers (e.g., `ptr->nested.field`).
- [x] **Pointer Arithmetic**: Full support for mixing pointers and dereferences in arithmetic expressions (e.g., `x + *ptr`).
- [ ] **Type Checking**:
  - [x] Mixed `char`/`int` arithmetic behavior needs verification.
  - [ ] Strict type checking for assignments and function calls.
- [ ] **Printf Support**:
  - [x] Basic specifiers: `%d`, `%u`, `%x`, `%c`, `%s`, `%%`.
  - [ ] Missing width/precision modifiers.
- [ ] **Goto/Label**:
  - [x] Forward gotos within the same function (the `goto cleanup` pattern).
  - [ ] Backward gotos (jumping to a label before the goto statement).
  - [ ] Goto into nested blocks.

## 🟡 TUI & Usability Improvements

- [ ] **Performance**:
  - [ ] Implement pagination/windowing for Stack and Heap panes (currently renders all items, potentially slow).
  - [ ] Optimize large memory snapshot rendering.
- [ ] **Display Limits**:
  - [ ] Handle deep or cyclic struct references in variables view (verify recursion limits).
- [ ] **Input Handling**:
  - [ ] Verify scrolling behavior when stepping backwards (auto-scroll to active line).
  - [x] Shift+Tab (BackTab) for reverse pane cycling.

## 🟢 Integration & Polish

- [ ] **CLI/Main**:
  - [ ] Ensure `main.rs` gracefully transitions to TUI.
  - [ ] Better error reporting for runtime errors without TUI mode.
- [ ] **Standard Library**:
  - [ ] Add `memset`, `memcpy`, `strcpy` built-ins.
  - [ ] Support `NULL` macro properly in all contexts.
- [x] **Error Handling**:
  - [x] Use-after-free errors now produce `RuntimeError::UseAfterFree` with proper address instead of generic `InvalidMemoryOperation`.

## 🧪 Testing & Verification

- [ ] **CI Limitations**: Tests requiring TTY/Interactive TUI cannot run in CI.
- [ ] **Integration Scenarios**:
  - [ ] Full run-through of `examples/default.c` checking for memory leaks.
- [ ] **Edge Cases**:
  - [x] Stack overflow detection (recursion limit) — call depth is capped at
    `MAX_CALL_DEPTH` and surfaces `RuntimeError::StackOverflow`; execution runs
    on a large-stack thread so the cap fires before the native stack overflows.
  - [ ] Zero-size allocations.

## 🧊 Backlog / Future

- [ ] **Parser**: Switch to `nom` or similar if hand-written parser becomes unmaintainable.
- [ ] **Snapshot Optimization**: Implement copy-on-write instead of full cloning for memory efficiency.
- [ ] **Save/Load**: Capability to save execution trace to file.

## Completed Cleanup

- [x] Hardened `sizeof_type` to never panic on malformed types (unknown or
  self-referential structs, unknown array sizes); execution paths validate
  types up front via `Interpreter::ensure_type_complete` and surface a clean
  `RuntimeError::StructNotDefined` / `UnsupportedOperation` instead.
- [x] Capped recursion depth (`MAX_CALL_DEPTH`) and moved execution onto a
  large-stack thread so unbounded recursion reports `RuntimeError::StackOverflow`
  instead of hanging or crashing the process.
- [x] Removed unused `RuntimeError::Generic` variant and `RuntimeError::format()` method.
- [x] Removed unused `Theme.bg` field.
- [x] Removed unused `MockTerminal::println`, `delete_output_from_line`, `clear` methods.
- [x] Removed unused `SnapshotManager::latest`, `count`, `clear` methods.
