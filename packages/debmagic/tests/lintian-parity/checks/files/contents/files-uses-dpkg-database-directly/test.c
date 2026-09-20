#include <stdio.h>
#include <string.h>

#define VAR_LIB_DPKG "/var/lib/dpkg"

int
main(void)
{
    printf("/var/lib/dpkg\n");
    printf("%s\n", VAR_LIB_DPKG);

    return 0;
}
