#include <stdio.h>
#include "hardening-trigger.h"

static const char no_magic_files[]
    = "could not find any magic files!";

int
main(void)
{
    printf("%s\n", no_magic_files);
}
