/* scanf demo: enter two numbers to see their sum and product */

int main() {
    int a;
    int b;

    printf("Enter first number: ");
    scanf("%d", &a);

    printf("Enter second number: ");
    scanf("%d", &b);

    printf("Sum:     %d\n", a + b);
    printf("Product: %d\n", a * b);

    while(scanf("%d", &a) == 1) {
        printf("You entered: %d\n", a);
    }

    return 0;
}
