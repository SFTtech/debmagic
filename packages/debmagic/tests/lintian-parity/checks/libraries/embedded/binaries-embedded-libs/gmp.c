#include <stdio.h>
#include "hardening-trigger.h"

/*
 * There's not much you can do with GMP without having the allocation
 * code ...
 */
static const char alloc_failure[]
    = "GNU MP: Cannot allocate memory";

int
main(void)
{
    printf("%s\n", alloc_failure);
}
