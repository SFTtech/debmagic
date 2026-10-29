#include <stdio.h>
#include <string.h>

int
main(int argc, char *argv[])
{
    char t[10];
    printf("Hello world!\n");
    /* forces a stack protector */
    (void) strcpy(t,argv[0]);
    return (int) t[0];
}
