#include <stdio.h>
#include <string.h>

void
report(char *string)
{
    char buf[80];
    int len;

    strcpy(buf, string);
    fprintf(stdout, "Hello world from %s!\n%n", buf, &len);
}

int
main(int argc, char *argv[])
{
    report(argv[0]);
}
