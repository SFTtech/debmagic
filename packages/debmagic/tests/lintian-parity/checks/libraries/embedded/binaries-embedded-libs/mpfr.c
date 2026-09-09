#include <stdio.h>
#include "hardening-trigger.h"

/*
 * There's not much you can do with MPFR without having the allocation
 * code ...
 */
static const char alloc_failure[]
    = "MPFR: Can't allocate memory";

int
main(void)
{
    printf("%s\n", alloc_failure);
}
