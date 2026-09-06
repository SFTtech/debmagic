#include <stdio.h>
#include "hardening-trigger.h"

/*
 * zlib asks derivative works to include this string, so it's the signature
 * that we use to detect embedded copies.
 */
static const char deflate_copyright[]
    = "deflate 1.2.3.3 Copyright 1995-2006 Jean-loup Gailly";

int
main(void)
{
    printf("%s\n", deflate_copyright);
}
