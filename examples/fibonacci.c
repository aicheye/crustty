// Dynamic Programming Fibonacci Implementation
// Uses memoization to cache results and avoid redundant calculations

int fib_dp(int n, int* memo) {
    // Base cases
    if (n <= 1) {
        return n;
    }

    // Check if already computed
    if (memo[n] != -1) {
        return memo[n];
    }

    // Compute and store result
    memo[n] = fib_dp(n - 1, memo) + fib_dp(n - 2, memo);
    return memo[n];
}

void init_memo(int* memo, int size) {
    int i;
    i = 0;
    while (i < size) {
        memo[i] = -1;
        i = i + 1;
    }
}

int main() {
    int memo[50];
    int n;
    int result;

    init_memo(memo, 50);

    printf("Computing Fibonacci numbers using DP:\n");

    n = 0;
    while (n <= 15) {
        result = fib_dp(n, memo);
        printf("fib(%d) = %d\n", n, result);
        n = n + 1;
    }

    printf("\nTesting larger value:\n");
    result = fib_dp(20, memo);
    printf("fib(20) = %d\n", result);

    return 0;
}
