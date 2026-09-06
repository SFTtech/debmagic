#include <stdio.h>
#include "hardening-trigger.h"

/*
 * The tcd_decode error message appears to be unique enough to be used to
 * detect embedded or statically-linked copies of libopenjpeg.
 */
static const char tcd_error[]
    = "tcd_decode: incomplete bistream";

int
main(void)
{
    printf("%s\n", tcd_error);
}
