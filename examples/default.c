/*
 * Comprehensive Test File for Crustty C Interpreter
 * Tests all compatible language features
 */

#include <stdio.h>
#include <stdlib.h>

// ================== STRUCT DEFINITIONS ==================

struct Point {
  int x;
  int y;
};

struct Rectangle {
  struct Point topLeft;
  struct Point bottomRight;
  int area;
};

struct Node {
  int value;
  struct Node *next;
};

// ================== FUNCTION DEFINITIONS ==================

// Basic arithmetic function
int add(int a, int b) { return a + b; }

// Function with multiple parameters and local variables
int multiply_and_add(int x, int y, int z) {
  int product = x * y;
  int result = product + z;
  return result;
}

// Recursive function - factorial
int factorial(int n) {
  if (n <= 1) {
    return 1;
  }
  return n * factorial(n - 1);
}

// Function using pointers
void swap(int *a, int *b) {
  int temp = *a;
  *a = *b;
  *b = temp;
  return;
}

// Function returning struct
struct Point make_point(int x, int y) {
  struct Point p;
  p.x = x;
  p.y = y;
  return p;
}

// Function with pointer to struct parameter
int point_distance_squared(struct Point *p1, struct Point *p2) {
  int dx = p2->x - p1->x;
  int dy = p2->y - p1->y;
  return dx * dx + dy * dy;
}

// Function using sizeof
int get_struct_size(void) { return sizeof(struct Point); }

// ================== MAIN FUNCTION ==================

int main(void) {
  // ====== BASIC VARIABLE DECLARATIONS ======
  int a = 10;
  int b = 3;
  int c;
  char ch = 65; // ASCII 'A'

  // ====== ARITHMETIC OPERATORS ======
  c = a + b; // 13
  c = a - b; // 7
  c = a * b; // 30
  c = a / b; // 3
  c = a % b; // 1

  // ====== COMPARISON OPERATORS ======
  int cmp1 = (a == b); // 0
  int cmp2 = (a != b); // 1
  int cmp3 = (a < b);  // 0
  int cmp4 = (a <= b); // 0
  int cmp5 = (a > b);  // 1
  int cmp6 = (a >= b); // 1

  // ====== LOGICAL OPERATORS ======
  int log1 = (a > 0) && (b > 0); // 1
  int log2 = (a < 0) || (b > 0); // 1
  int log3 = !(a == b);          // 1

  // ====== BITWISE OPERATORS ======
  int bit1 = a & b;  // 10 & 3 = 2
  int bit2 = a | b;  // 10 | 3 = 11
  int bit3 = a ^ b;  // 10 ^ 3 = 9
  int bit4 = ~a;     // ~10
  int bit5 = a << 2; // 10 << 2 = 40
  int bit6 = a >> 1; // 10 >> 1 = 5

  // ====== UNARY OPERATORS ======
  int neg = -a;       // -10
  int pre_inc = ++a;  // a becomes 11, pre_inc = 11
  int post_inc = a++; // post_inc = 11, a becomes 12
  int pre_dec = --a;  // a becomes 11, pre_dec = 11
  int post_dec = a--; // post_dec = 11, a becomes 10

  // ====== COMPOUND ASSIGNMENT ======
  int compound = 5;
  compound += 3; // 8
  compound -= 2; // 6
  compound *= 4; // 24
  compound /= 3; // 8
  compound %= 5; // 3

  // ====== TERNARY OPERATOR ======
  int max_val = (a > b) ? a : b;        // 10
  int abs_val = (neg < 0) ? -neg : neg; // 10

  // ====== ARRAYS ======
  int arr[5];
  arr[0] = 1;
  arr[1] = 2;
  arr[2] = 3;
  arr[3] = 4;
  arr[4] = 5;

  int sum = arr[0] + arr[1] + arr[2] + arr[3] + arr[4]; // 15

  // Array with initializer (element by element)
  int arr2[3];
  arr2[0] = 10;
  arr2[1] = 20;
  arr2[2] = 30;

  // ====== POINTERS ======
  int val = 42;
  int *ptr = &val;
  int deref = *ptr; // 42
  *ptr = 100;       // val becomes 100

  char *newChar = malloc(sizeof(char));
  *newChar = 'A';

  // Pointer to pointer
  int **pptr = &ptr;
  int deref2 = **pptr; // 100

  // ====== STRUCTS (STACK) ======
  struct Point p1;
  p1.x = 5;
  p1.y = 10;

  struct Point p2;
  p2.x = 15;
  p2.y = 20;

  int px = p1.x + p2.x; // 20
  int py = p1.y + p2.y; // 30

  // Nested struct
  struct Rectangle rect;
  rect.topLeft.x = 0;
  rect.topLeft.y = 0;
  rect.bottomRight.x = 10;
  rect.bottomRight.y = 5;
  rect.area = (rect.bottomRight.x - rect.topLeft.x) *
              (rect.bottomRight.y - rect.topLeft.y); // 50

  // ====== STRUCTS (HEAP) ======
  struct Point *heap_point = (struct Point *)malloc(sizeof(struct Point));
  heap_point->x = 100;
  heap_point->y = 200;
  int hx = heap_point->x; // 100
  int hy = heap_point->y; // 200
  free(heap_point);

  // ====== LINKED LIST (HEAP) ======
  struct Node *head = (struct Node *)malloc(sizeof(struct Node));
  head->value = 1;
  head->next = (struct Node *)malloc(sizeof(struct Node));
  head->next->value = 2;
  head->next->next = (struct Node *)malloc(sizeof(struct Node));
  head->next->next->value = 3;
  head->next->next->next = NULL;

  // Traverse linked list
  int list_sum = 0;
  struct Node *current = head;
  while (current != NULL) {
    list_sum += current->value; // Should be 6
    current = current->next;
  }

  // Free linked list
  struct Node *temp;
  while (head != NULL) {
    temp = head;
    head = head->next;
    free(temp);
  }

  // ====== CONTROL FLOW: IF/ELSE ======
  int if_result;
  if (a > 5) {
    if_result = 1;
  } else {
    if_result = 0;
  }

  // Nested if
  int nested_if;
  if (a > 0) {
    if (b > 0) {
      nested_if = 1;
    } else {
      nested_if = 2;
    }
  } else {
    nested_if = 0;
  }

  // ====== CONTROL FLOW: SWITCH =====
  switch (a) {
    case 10:
      printf("a is 10\n");
      break;
    default:
      printf("should not run\n");
      break;
  }

  switch (a) {
    case 10:
      printf("a is 10\n");
    default:
      printf("should run\n");
      break;
  }

  // ====== CONTROL FLOW: WHILE ======
  int i = 0;
  int while_sum = 0;
  while (i < 5) {
    while_sum += i;
    i++;
  } // while_sum = 0+1+2+3+4 = 10

  // ====== CONTROL FLOW: DO-WHILE ======
  int j = 0;
  int do_sum = 0;
  do {
    do_sum += j;
    j++;
  } while (j < 5); // do_sum = 0+1+2+3+4 = 10

  // ====== CONTROL FLOW: FOR ======
  int for_sum = 0;
  for (int k = 0; k < 5; k++) {
    for_sum += k;
  } // for_sum = 0+1+2+3+4 = 10

  // Nested for loops
  int matrix_sum = 0;
  for (int row = 0; row < 3; row++) {
    for (int col = 0; col < 3; col++) {
      matrix_sum += row * 3 + col;
    }
  } // 0+1+2+3+4+5+6+7+8 = 36

  // ====== FUNCTION CALLS ======
  int add_result = add(5, 7);                   // 12
  int multi_result = multiply_and_add(2, 3, 4); // 10
  int fact_result = factorial(5);               // 120

  // ====== POINTER PARAMETERS ======
  int swap_a = 10;
  int swap_b = 20;
  swap(&swap_a, &swap_b);
  // Now swap_a = 20, swap_b = 10

  // ====== STRUCT RETURN VALUES ======
  struct Point returned_point = make_point(42, 84);
  int ret_x = returned_point.x; // 42
  int ret_y = returned_point.y; // 84

  // ====== POINTER TO STRUCT PARAMETER ======
  struct Point pt1;
  pt1.x = 0;
  pt1.y = 0;
  struct Point pt2;
  pt2.x = 3;
  pt2.y = 4;
  int dist_sq = point_distance_squared(&pt1, &pt2); // 25

  // ====== SIZEOF ======
  int point_size = sizeof(struct Point);
  int int_size = sizeof(int);
  int char_size = sizeof(char);
  int ptr_size = sizeof(int *);
  int func_size = get_struct_size();

  // ====== TYPE CASTING ======
  int int_val = 65;
  char cast_char = (char)int_val;   // 'A'
  int back_to_int = (int)cast_char; // 65

  // Pointer casting (already demonstrated with malloc)
  int *int_ptr = (int *)malloc(sizeof(int));
  *int_ptr = 999;
  free(int_ptr);

  // ====== COMPLEX EXPRESSIONS ======
  int complex1 = (a + b) * (a - b) / 2;
  int complex2 = a > 0 && b > 0 ? a + b : a - b;
  int complex3 = (a << 2) | (b & 255);

  // ====== CHAINED OPERATIONS ======
  int chain = 1;
  chain += 2;
  chain *= 3;
  chain -= 1;
  chain /= 2; // ((1+2)*3-1)/2 = 4

  // ====== MULTIPLE MALLOC/FREE ======
  int *dyn_ptr1 = (int *)malloc(sizeof(int));
  int *dyn_ptr2 = (int *)malloc(sizeof(int));
  int *dyn_ptr3 = (int *)malloc(sizeof(int));
  *dyn_ptr1 = 10;
  *dyn_ptr2 = 20;
  *dyn_ptr3 = 30;
  int dyn_sum = *dyn_ptr1 + *dyn_ptr2 + *dyn_ptr3; // 60
  free(dyn_ptr1);
  free(dyn_ptr2);
  free(dyn_ptr3);

  // ====== POINTER ARITHMETIC (via array indexing) ======
  int parr[5];
  parr[0] = 100;
  parr[1] = 200;
  parr[2] = 300;
  parr[3] = 400;
  parr[4] = 500;
  int first = parr[0];
  int last = parr[4];

  // ====== HEAP ARRAY ======
  int *heap_arr = (int *)malloc(5 * sizeof(int));
  for (int i = 0; i < 5; i++) {
    heap_arr[i] = i * 10;
  }
  int heap_val = heap_arr[2]; // 20
  free(heap_arr);

  // ====== NULL POINTER CHECKS ======
  struct Node *null_ptr = NULL;
  int is_null = (null_ptr == NULL) ? 1 : 0; // 1

  // ====== FINAL OUTPUT ======
  printf("Test complete!\n");
  printf("Factorial of 5: %d\n", fact_result);
  printf("List sum: %d\n", list_sum);
  printf("Distance squared: %d\n", dist_sq);

  return 0;
}
