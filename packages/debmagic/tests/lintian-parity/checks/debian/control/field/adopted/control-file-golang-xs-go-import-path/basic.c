#include <stdio.h>

int
main(void)
{
    char t[10];
    printf("Hello world!\n");
    /* Bad choice for reading from stdin, but it forces a stack
       protector, so meh.
     */
    gets (t);
}
