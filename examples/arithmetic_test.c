int main() {
    char c = 'a'; // 97
    int i = 10;

    // Test 1: Char + Int
    int result1 = c + i;
    printf("Char + Int: %d\n", result1); // Expected: 107

    // Test 2: Int - Char
    int result2 = i - c;
    printf("Int - Char: %d\n", result2); // Expected: -87

    // Test 3: Char * Char
    char c2 = 2;
    int result3 = c * c2;
    printf("Char * Char: %d\n", result3); // Expected: 194

    // Test 4: Comparison
    if (c == 97) {
        printf("Comparison Char == Int: OK\n");
    } else {
        printf("Comparison Char == Int: FAIL\n");
    }

    if (97 == c) {
        printf("Comparison Int == Char: OK\n");
    } else {
        printf("Comparison Int == Char: FAIL\n");
    }

     // Test 5: Division
    int result4 = c / 2;
    printf("Char / Int: %d\n", result4); // Expected: 48

    return 0;
}
