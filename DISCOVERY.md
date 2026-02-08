# DISCOVERY.md

## Purpose
This document records all clarifying questions and answers that define the project specification. These decisions are binding and inform all architectural and implementation choices.

---

## C Language Subset

**Q1: What is the minimal viable C subset?**
**A:** Keywords: break, case, continue, default, do, else, for, goto, if, int, nullptr, return, sizeof, structs, switch, void, while. Arrays (must be supported), recursion (must be supported), malloc/free and heap management (must be supported).

**Q2: Preprocessor support?**
**A:** No. Skip #include directives when encountered. Provide built-in declarations for printf, malloc, free.

**Q3: Which undefined behaviors must we detect?**
**A:** Uninitialized reads, null dereferences, buffer overruns must cause errors. Goal is to understand C, not mimic RAM/CPU.

**Q4: malloc/free in scope?**
**A:** YES (covered in Q1).

**Additional operators (from test file analysis):**
- Compound assignment: `+=`, `-=`, `*=`, etc.
- Logical operators: `&&`, `||`, `!`
- Bitwise operators: `&`, `|`, `^`, `~`, `<<`, `>>`
- Comparison operators: `<`, `>`, `<=`, `>=`, `==`, `!=`
- Arithmetic operators: `+`, `-`, `*`, `/`, `%`
- Unary operators: `+`, `-`, `!`, `~`, `&`, `*` (deref), `++`, `--`
- Member access: `->`, `.`
- Ternary: `? :`
- Cast: `(Type*)` for void pointers only

**Additional features:**
- Comments: Both `//` and `/* */` styles
- Declaration with initialization: `int x = 0;`
- Return by value for structs
- `const` qualifier (modifying const is an error)
- `NULL` as a keyword/constant (not `nullptr` - corrected from Q1)

---

## Memory Model

**Q5: Byte-addressable with pointer arithmetic?**
**A:** Yes, must include faithful pointer arithmetic (scaled by type size).

**Q6: Track uninitialized memory?**
**A:** Yes, track uninitialized access and error on attempts to access it.

**Q7: Stack frame structure?**
**A:** Dynamic frames (not fixed-size).

**Q27: Exact type sizes?**
**A:**
- `sizeof(int)` = 4 bytes
- `sizeof(char)` = 1 byte
- `sizeof(void*)` = 8 bytes (all pointer types)
- `sizeof(struct)` = sum of fields (no padding/alignment)

**Q28: Pointer arithmetic semantics?**
**A:** Implement faithfully (scaled arithmetic). `ptr + 1` advances by `sizeof(*ptr)` bytes.

**Q31: Uninitialized memory tracking granularity?**
**A:** Per-field tracking. After `malloc(sizeof(struct Foo))`, individual fields are tracked separately.

**Q37: Implementation detail for Q31?**
**A:** Per-field tracking requires shadow metadata per struct member.

**Q38: String literal storage?**
**A:** Inline in AST (no memory representation). No separate data section.

**Q39: Array declaration syntax?**
**A:**
- `int arr[10];` ✓ (fixed size)
- `int arr[N];` ✗ (VLA not supported)
- `int arr[] = {1, 2, 3};` ✓ (inferred from initializer)
- `int mat[5][10];` ✓ (multidimensional)

**Q40: Global variables?**
**A:** No.

**Q41: Cast operators?**
**A:** Yes, but only for void-to-type pointers: `(int*)malloc(...)`, `(char*)ptr`.

---

## Reverse Execution

**Q8: Maximum step-back depth?**
**A:** Entire execution history.

**Q9: Step granularity?**
**A:** Statement-level. One C statement = one step.

**Q10: I/O support and reversal?**
**A:** Yes, I/O supported but to a mock terminal (not real stdout/stdin). Mock terminal can be erased on TUI for reverse execution.

**Q21: Un-malloc mechanics?**
**A:** Free memory on step-back, restore on step-forward.

**Q22: Terminal output reversal?**
**A:** When stepping back a line, delete all terminal output from that source line. When stepping forward, if there's a printf, re-execute it (not replay from recording).

**Q23: Heap restoration mechanics?**
**A:** Use tombstones for freed blocks. Tombstones are not visible to users.

**Q24: "Delete terminal output from that line" means?**
**A:** Delete output produced by the current *source line* being stepped back from (not terminal lines).

**Q25: Statement granularity detail?**
**A:** Entire statement is one step. `for (int i = 0; i < 10; i++)` header is one step. Function call with argument evaluation is one step.

**Q26: Memory budget for full history?**
**A:** Set a hard limit. Error if snapshot history exceeds the limit.

**Q36: Snapshot strategy?**
**A:** Hard limit on total snapshot memory. Error when exceeded (not copy-on-write optimization initially).

---

## Execution Model

**Q11: AST or bytecode?**
**A:** AST interpretation (no bytecode, no compilation pass).

**Q12: Branch history tracking?**
**A:** Not needed. Just show current state (current line, stack, heap).

**Q29: sizeof evaluation?**
**A:** Treat it like a function call (runtime evaluation, not parse-time).

**Q30: Integer overflow behavior?**
**A:** Error on overflow. Do not distinguish signed vs unsigned (all treated as signed for overflow purposes).

**Q42: Ternary operator?**
**A:** Yes, supported.

---

## Terminal UI

**Q13: Minimum visible state?**
**A:** Stack frames, heap, mock terminal I/O, current source line + context lines.

**Q14: Minimum terminal size?**
**A:** Yes, assume a minimum terminal size.

**Q15: TUI libraries?**
**A:** Anything needed to make it look nice.

**Navigation:**
- Arrow keys left/right: switch between panes (code → stack → heap → terminal)
- Arrow keys up/down: scroll within the current pane

---

## Built-in Functions

**Q19: How are printf/malloc/free/sizeof provided?**
**A:** Magic functions implemented in Rust.

**Q32: printf format specifier support?**
**A:** Support `%d`, `%s`, `%c`, `%n` only. Validate format strings.

**Q33: malloc failure behavior?**
**A:** malloc never fails. If the program runs out of memory, show an error and quit.

---

## Test Program

**Q18: Reference test suite?**
**A:** `listmatrix.c` must run correctly.

**Q34: What should main() do?**
**A:** Happy path - just demonstrate matrix operations to visualize memory.

**Q43: When to write main()?**
**A:** Now. (Completed - added to listmatrix.c)

---

## Contradictions Resolved

**C1: Preprocessor**
Initially answered "no preprocessor" but test file has #include directives.
**Resolution:** Skip #include when parsing, provide built-in declarations.

**C2: nullptr vs NULL**
Initially listed "nullptr" as keyword but test uses "NULL" (standard C).
**Resolution:** Support NULL (not nullptr). Define as keyword/constant.

**C3: Missing main()**
Test file had no main() function.
**Resolution:** main() is required. Written and added to listmatrix.c.

**C4: const qualifier**
Not in initial keyword list but used in test file.
**Resolution:** Support const. Modifying const variables is an error.

---

## Out of Scope (Explicitly)

- Preprocessor (#include, #define, etc.)
- Global variables
- Static storage (`static` keyword)
- Variable-length arrays (VLAs)
- Function pointers
- `typedef`, `enum`, `union`
- Comma operator (`,` outside function calls)
- Full cast support (only void pointer casts)
- Variadic functions (except built-in printf)
- Alignment and padding in structs
- Platform-dependent behavior
- Optimization for performance
- Real I/O (only mock terminal)

---

## Design Principles Applied

From CLAUDE.md:
- **Clarity over fidelity:** Simplified memory model (no padding), deterministic execution (malloc never fails)
- **Explicit state:** All execution state is data structures (snapshots, tombstones)
- **Determinism:** Same input = same execution path
- **Errors are first-class:** Detect UB (uninitialized reads, null deref, overflow, buffer overruns)

---

## Implementation Constraints

From CLAUDE.md:
- Single-threaded
- No JIT
- No LLVM
- `unsafe` Rust discouraged (must be justified)
- Interior mutability must be explained

---

End of discovery phase.
