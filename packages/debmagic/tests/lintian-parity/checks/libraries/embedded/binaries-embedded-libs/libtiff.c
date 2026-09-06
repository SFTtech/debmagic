#include <stdio.h>
#include "hardening-trigger.h"

/*
 * The PixarLog error message is unique enough to be used to
 * detect embedded or statically-linked copies of libtiff.
 */
static const char pixarlog_error[]
    = "No space for PixarLog state block";

int
main(void)
{
    printf("%s\n", pixarlog_error);
}
