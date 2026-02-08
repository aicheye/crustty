// Test file with runtime error (uninitialized read)
int main() {
    int x;  // Uninitialized variable
    printf("x = %d\n", x);  // Line 4 - should cause uninitialized read error
    return 0;
}
