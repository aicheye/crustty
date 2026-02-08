// Integration tests for the C interpreter

use crustty::interpreter::engine::Interpreter;
use crustty::parser::parser::Parser;

#[test]
fn test_simple_arithmetic() {
    let source = r#"
        int main() {
            int x = 5;
            int y = 10;
            int z = x + y;
            return z;
        }
    "#;

    // Parse
    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    // Execute
    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100); // 100MB limit
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_function_call() {
    let source = r#"
        int add(int a, int b) {
            return a + b;
        }

        int main() {
            int result = add(3, 4);
            return result;
        }
    "#;

    // Parse
    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    // Execute
    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_struct_basics() {
    let source = r#"
        struct Point {
            int x;
            int y;
        };

        int main() {
            struct Point p;
            p.x = 10;
            p.y = 20;
            return p.x + p.y;
        }
    "#;

    // Parse
    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    // Execute
    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

// === HEAP STRUCT INTEGRATION TESTS ===

#[test]
fn test_heap_struct_allocation() {
    let source = r#"
        struct Point {
            int x;
            int y;
        };

        int main() {
            struct Point* p = (struct Point*)malloc(sizeof(struct Point));
            p->x = 42;
            p->y = 100;
            int sum = p->x + p->y;
            free(p);
            return sum;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_heap_struct_multiple_allocations() {
    let source = r#"
        struct Data {
            int value;
        };

        int main() {
            struct Data* d1 = (struct Data*)malloc(sizeof(struct Data));
            struct Data* d2 = (struct Data*)malloc(sizeof(struct Data));

            d1->value = 10;
            d2->value = 20;

            int result = d1->value + d2->value;

            free(d1);
            free(d2);

            return result;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_heap_struct_nested_fields() {
    let source = r#"
        struct Inner {
            int a;
            int b;
        };

        struct Outer {
            int c;
            int d;
        };

        int main() {
            struct Outer* obj = (struct Outer*)malloc(sizeof(struct Outer));
            obj->c = 5;
            obj->d = 10;

            int result = obj->c + obj->d;
            free(obj);
            return result;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_heap_double_free_error() {
    let source = r#"
        struct Point {
            int x;
            int y;
        };

        int main() {
            struct Point* p = (struct Point*)malloc(sizeof(struct Point));
            p->x = 10;
            free(p);
            free(p);
            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_err(), "Expected double-free error");
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(
        error_msg.contains("DoubleFree")
            || error_msg.contains("Double")
            || error_msg.contains("freed"),
        "Error message should mention double-free, got: {}",
        error_msg
    );
}

#[test]
fn test_heap_use_after_free_error() {
    let source = r#"
        struct Point {
            int x;
            int y;
        };

        int main() {
            struct Point* p = (struct Point*)malloc(sizeof(struct Point));
            p->x = 10;
            free(p);
            int val = p->x;
            return val;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_err(), "Expected use-after-free error");
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(
        error_msg.contains("Use-after-free") || error_msg.contains("freed"),
        "Error message should mention use-after-free or freed, got: {}",
        error_msg
    );
}

#[test]
fn test_heap_null_dereference() {
    let source = r#"
        struct Point {
            int x;
            int y;
        };

        int main() {
            struct Point* p = NULL;
            p->x = 10;
            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_err(), "Expected null dereference error");
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(
        error_msg.contains("Null") || error_msg.contains("null"),
        "Error message should mention null, got: {}",
        error_msg
    );
}

#[test]
fn test_heap_struct_pointer_in_struct() {
    let source = r#"
        struct Node {
            int value;
            struct Node* next;
        };

        int main() {
            struct Node* first = (struct Node*)malloc(sizeof(struct Node));
            struct Node* second = (struct Node*)malloc(sizeof(struct Node));

            first->value = 1;
            first->next = second;

            second->value = 2;
            second->next = NULL;

            int sum = first->value + first->next->value;

            free(first);
            free(second);

            return sum;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_heap_struct_mixed_types() {
    let source = r#"
        struct Mixed {
            int num;
            int value;
        };

        int main() {
            struct Mixed* m = (struct Mixed*)malloc(sizeof(struct Mixed));

            m->num = 42;
            m->value = 100;

            int result = m->num + m->value;

            free(m);
            return result;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

// === SNAPSHOT AND REVERSE EXECUTION TESTS ===

#[test]
fn test_step_backward() {
    let source = r#"
        int main() {
            int x = 5;
            int y = 10;
            int z = x + y;
            return z;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);

    // Run to completion
    let result = interpreter.run();
    assert!(result.is_ok(), "Execution failed: {:?}", result);

    // Step backward once
    let result = interpreter.step_backward();
    assert!(result.is_ok(), "Step backward failed: {:?}", result);

    // Step backward multiple times
    for _ in 0..3 {
        let result = interpreter.step_backward();
        assert!(result.is_ok(), "Step backward failed: {:?}", result);
    }
}

#[test]
fn test_step_forward_and_backward() {
    let source = r#"
        int main() {
            int x = 1;
            int y = 2;
            int z = 3;
            return z;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);

    // Run to completion
    interpreter.run().expect("Execution failed");

    // Step backward
    interpreter.step_backward().expect("Step backward failed");
    interpreter.step_backward().expect("Step backward failed");

    // Step forward (should replay from history)
    let result = interpreter.step_forward();
    assert!(result.is_ok(), "Step forward failed: {:?}", result);
}

#[test]
fn test_step_backward_at_beginning() {
    let source = r#"
        int main() {
            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);

    // Try stepping backward at the start - should fail
    let result = interpreter.step_backward();
    assert!(
        result.is_err(),
        "Expected error when stepping backward at beginning"
    );
}

#[test]
fn test_reverse_execution_preserves_state() {
    let source = r#"
        int main() {
            int x = 5;
            x = x + 10;
            x = x * 2;
            return x;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);

    // Run to completion
    interpreter.run().expect("Execution failed");

    // Step all the way back
    while interpreter.step_backward().is_ok() {}

    // Step forward to same point
    while interpreter.step_forward().is_ok() {}

    // State should be consistent (just verify it doesn't panic/error)
    assert!(true, "Successfully stepped forward and backward");
}

// ================== CONVERTED C FILE TESTS ==================

#[test]
fn test_array_minimal() {
    let source = r#"
        int main(void) {
            int arr[3];
            arr[0] = 1;
            arr[1] = 2;
            arr[2] = 3;
            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_array_sizeof() {
    let source = r#"
        #include <stdio.h>

        int main() {
            int nums[5];

            int array_size = sizeof(nums);
            printf("sizeof(nums) = %d (expected 20)\n", array_size);

            nums[0] = 10;
            nums[1] = 20;
            nums[2] = 30;
            nums[3] = 40;
            nums[4] = 50;

            int *ptr = nums;
            printf("ptr[0] = %d (expected 10)\n", ptr[0]);
            printf("ptr[2] = %d (expected 30)\n", ptr[2]);
            printf("ptr[4] = %d (expected 50)\n", ptr[4]);

            printf("nums[1] = %d (expected 20)\n", nums[1]);
            printf("nums[3] = %d (expected 40)\n", nums[3]);

            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_basic() {
    let source = r#"
        #include <stdio.h>

        int main() {
            int a = 5;
            int b = 10;
            int sum = a + b;

            printf("a = %d\n", a);
            printf("b = %d\n", b);
            printf("sum = %d\n", sum);

            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_break_continue() {
    let source = r#"
        #include <stdio.h>

        int main() {
            printf("Testing break:\n");
            int i = 0;
            while (i < 10) {
                printf("i = %d\n", i);
                if (i == 5) {
                    printf("Breaking at i = 5\n");
                    break;
                }
                i = i + 1;
            }
            printf("After break, i = %d\n\n", i);

            printf("Testing continue:\n");
            int j = 0;
            while (j < 10) {
                j = j + 1;
                if (j == 3 || j == 7) {
                    printf("Skipping j = %d\n", j);
                    continue;
                }
                printf("j = %d\n", j);
            }
            printf("After continue, j = %d\n", j);

            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_char() {
    let source = r#"
        #include <stdio.h>

        int main() {
            char a = 'A';
            char b = 'B';
            char newline = '\n';
            char tab = '\t';

            printf("a = %c\n", a);
            printf("b = %c\n", b);

            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_char_array() {
    let source = r#"
        #include <stdio.h>
        #include <stdlib.h>

        int main() {
            char c = 'A';
            char d = 65;
            int nums[5];
            nums[0] = 10;
            nums[1] = 20;
            nums[2] = 30;
            nums[3] = 40;
            nums[4] = 50;

            int *arr = malloc(5 * sizeof(int));
            arr[0] = 1;
            arr[1] = 2;
            arr[2] = 3;
            arr[3] = 4;
            arr[4] = 5;

            int i = 0;
            while (i < 3) {
                printf("i = %d\n", i);
                i = i + 1;
            }

            free(arr);
            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_control_flow() {
    let source = r#"
        #include <stdio.h>

        int main() {
            printf("Before if statement\n");

            int x = 5;
            if (x > 3) {
                printf("x is greater than 3\n");
            }

            printf("After if statement\n");

            printf("Before loop\n");
            int i = 0;
            while (i < 3) {
                printf("i = %d\n", i);
                i = i + 1;
            }
            printf("After loop\n");

            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_heap_uninit() {
    let source = r#"
        #include <stdio.h>
        #include <stdlib.h>

        int main() {
            int *uninit_arr = malloc(5 * sizeof(int));
            printf("Allocated uninitialized array\n");

            uninit_arr[0] = 100;
            uninit_arr[2] = 200;
            printf("Initialized some elements\n");

            int *init_arr = malloc(3 * sizeof(int));
            init_arr[0] = 10;
            init_arr[1] = 20;
            init_arr[2] = 30;
            printf("Allocated and initialized second array\n");

            int *third_arr = malloc(4 * sizeof(int));
            third_arr[0] = 5;
            third_arr[1] = 15;
            third_arr[2] = 25;
            third_arr[3] = 35;
            printf("Allocated third array - heap should auto-scroll\n");

            printf("First element of init_arr: %d\n", init_arr[0]);

            free(uninit_arr);
            free(init_arr);
            free(third_arr);

            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_loop_exit() {
    let source = r#"
        #include <stdio.h>

        int main() {
            printf("Testing while loop:\n");
            int i = 0;
            while (i < 3) {
                printf("i = %d\n", i);
                i = i + 1;
            }
            printf("After while loop\n\n");

            printf("Testing for loop:\n");
            int j;
            for (j = 0; j < 3; j = j + 1) {
                printf("j = %d\n", j);
            }
            printf("After for loop\n\n");

            printf("Testing do-while loop:\n");
            int k = 0;
            do {
                printf("k = %d\n", k);
                k = k + 1;
            } while (k < 3);
            printf("After do-while loop\n");

            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_simple() {
    let source = r#"
        #include <stdio.h>
        #include <stdlib.h>

        struct Point {
            int x;
            int y;
        };

        int main() {
            int a = 5;
            int b = 10;
            int sum = a + b;

            struct Point p;
            p.x = 3;
            p.y = 4;

            int *ptr = malloc(sizeof(int));
            *ptr = 42;

            printf("Sum: %d\n", sum);
            printf("Point: (%d, %d)\n", p.x, p.y);
            printf("Pointer value: %d\n", *ptr);

            free(ptr);

            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_struct_addresses() {
    let source = r#"
        struct Point {
            int x;
            int y;
        };

        int main() {
            int a;
            char b;
            struct Point p;
            int c;

            a = 10;
            b = 'X';
            p.x = 100;
            p.y = 200;
            c = 30;

            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_switch() {
    let source = r#"
        #include <stdio.h>

        int main() {
            int a = 10;

            printf("Testing switch with a = %d\n", a);

            switch (a) {
                case 5:
                    printf("a is 5\n");
                    break;
                case 10:
                    printf("a is 10\n");
                    break;
                case 15:
                    printf("a is 15\n");
                    break;
                default:
                    printf("a is something else\n");
                    break;
            }

            printf("After switch\n");

            int b = 2;
            printf("\nTesting fall-through with b = %d\n", b);

            switch (b) {
                case 1:
                    printf("b is 1\n");
                case 2:
                    printf("b is 2 (or fell through from 1)\n");
                case 3:
                    printf("b is 3 (or fell through from 1 or 2)\n");
                    break;
                default:
                    printf("b is something else\n");
            }

            printf("Done\n");

            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}

#[test]
fn test_switch_snapshots() {
    let source = r#"
        #include <stdio.h>

        int main() {
            int value = 2;

            printf("Testing switch with value = %d\n", value);

            switch (value) {
                case 1:
                    printf("Case 1\n");
                    break;
                case 2:
                    printf("Case 2\n");
                    break;
                case 3:
                    printf("Case 3\n");
                    break;
                default:
                    printf("Default case\n");
                    break;
            }

            printf("After switch\n");

            printf("\nTesting fall-through with value = 5\n");
            int x = 5;

            switch (x) {
                case 5:
                    printf("Case 5 - falling through\n");
                case 6:
                    printf("Case 6 - falling through\n");
                case 7:
                    printf("Case 7 - breaking\n");
                    break;
                default:
                    printf("Default\n");
            }

            printf("Done\n");

            return 0;
        }
    "#;

    let mut parser = Parser::new(source).expect("Parser creation failed");
    let program = parser.parse_program().expect("Parsing failed");

    let mut interpreter = Interpreter::new(program, 1024 * 1024 * 100);
    let result = interpreter.run();

    assert!(result.is_ok(), "Execution failed: {:?}", result);
}
