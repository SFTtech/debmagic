#include <stdio.h>
#include <string.h>

static void
hardening_trigger(char *p, int i, void (*f)(char *))
{
    char test[10];
    memcpy(test, p, i);
    f(test);
    printf("%s", test);
}

int
lib_interface(void)
{
    printf("Hello world!\n");
    hardening_trigger(NULL, 0, NULL);
    return 0;
}
