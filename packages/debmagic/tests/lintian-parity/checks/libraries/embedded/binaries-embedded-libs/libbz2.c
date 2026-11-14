#include <stdio.h>
#include "hardening-trigger.h"

static const char bzip2_bug[]
    = "This is a bug in bzip2";

int
main(void)
{
    printf("%s\n", bzip2_bug);
}
