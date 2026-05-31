#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void helper(int x) {
    int y = x * 2;
    printf("Result: %d\n", y);
}

int main(int argc, char *argv[]) {
    char *msg = "Hello from heretek!";
    int nums[] = {10, 20, 30, 40, 50};
    void *heap_ptr = malloc(256);
    memset(heap_ptr, 'A', 256);
    strcpy((char *)heap_ptr, msg);

    for (int i = 0; i < 5; i++) {
        helper(nums[i]);
    }

    free(heap_ptr);
    return 0;
}
