#include <stdio.h>
#include "hardening-trigger.h"

static const char domain_error[]
    = "neg**non-integral: DOMAIN error";

int
main(void)
{
    printf("%s\n", domain_error);
}
