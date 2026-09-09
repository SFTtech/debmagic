#include <stdio.h>
#include "hardening-trigger.h"

/*
 * The PCRE_UTF8 message is unique enough to be used to
 * detect embedded or statically-linked copies of pcre.
 */
static const char pcre_utf8[]
    = "this version of PCRE is not compiled with PCRE_UTF8 support";

int
main(void)
{
    printf("%s\n", pcre_utf8);
}
