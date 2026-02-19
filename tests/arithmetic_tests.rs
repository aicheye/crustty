use crustty::interpreter::engine::Interpreter;
use crustty::parser::parse::Parser;
use std::fs;
use std::path::Path;

#[test]
fn test_arithmetic_coercion() {
    let path = Path::new("examples/arithmetic_test.c");
    let source = fs::read_to_string(path).expect("Failed to read example file");

    let mut parser = Parser::new(&source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 100 * 1024 * 1024);

    // In actual usage, run() is called.
    interpreter.run().expect("Execution failed");

    // Check outputs
    // Note: MockTerminal output handling might be slightly finicky with newlines.
    // Based on `get_output()` it flat_maps split('\n').

    let output = interpreter.terminal().get_output();
    let joined_output = output
        .iter()
        .map(|(s, _)| s.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    println!("Interpreter output:\n{}", joined_output);

    // Assert expected outputs
    assert!(joined_output.contains("Char + Int: 107"));
    assert!(joined_output.contains("Int - Char: -87"));
    assert!(joined_output.contains("Char * Char: 194"));
    assert!(joined_output.contains("Comparison Char == Int: OK"));
    assert!(joined_output.contains("Comparison Int == Char: OK"));
    assert!(joined_output.contains("Char / Int: 48"));
}

#[test]
fn test_pointer_arithmetic() {
    let source = r#"
    int main() {
        int arr[5];
        int *p = arr;     // p points to arr[0]
        int *p2 = p + 2;  // p2 points to arr[2]

        // Write to p2
        *p2 = 42;

        // Read from array index
        if (arr[2] == 42) {
             printf("Pointer Write OK\n");
        } else {
             printf("Pointer Write FAIL: %d\n", arr[2]);
        }

        // Pointer difference
        int diff = p2 - p;

        printf("Diff elems: %d\n", diff);

        // Commutativity
        int *p3 = 2 + p;
        *p3 = 84;
        if (arr[2] == 84) {
             printf("Commutative Add OK\n");
        }

        // Char + Pointer
        char c = 1;

        return 0;
    }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 100 * 1024 * 1024);
    interpreter.run().expect("Execution failed");

    let output = interpreter.terminal().get_output();
    let joined = output
        .iter()
        .map(|(s, _)| s.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    println!("Pointer Output:\n{}", joined);

    assert!(joined.contains("Pointer Write OK"));
    assert!(joined.contains("Commutative Add OK"));
    // Verify diff behavior
    // Since I implemented element-diff
    assert!(joined.contains("Diff elems: 2"));
}
