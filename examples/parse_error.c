// Test file with parse error
int main() {
    int x = 5;
    int y = 10
    // Missing semicolon on line 4 - should cause parse error
    return x + y;
}
