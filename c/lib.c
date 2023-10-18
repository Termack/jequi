// gcc -fPIC -shared -o target/debug/jequi_go.so c/lib.c
#include <stdio.h>
void __attribute__((destructor)) calledLast();
    
void HandleRequest(void* req, void* resp) {
    printf("blaaabla\n");
}

// This function is assigned to execute after
// main using __attribute__((destructor))
void calledLast()
{
    printf("\nI am called last\n");
}