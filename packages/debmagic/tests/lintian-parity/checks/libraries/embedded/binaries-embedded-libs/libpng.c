#include <stdio.h>
#include "hardening-trigger.h"

/*
 * The png_zalloc overflow error message is unique enough to be used to
 * detect embedded or statically-linked copies of libpng.
 */
static const char zalloc_error[]
    = "Potential overflow in png_zalloc()";

int
main(void)
{
    printf("%s\n", zalloc_error);
}
