#include <stdio.h>
#include "hardening-trigger.h"

/*
 * include the copyright notice from minizip's decompression code
 */
static const char minizip_copyright[]
    = " unzip 1.01 Copyright 1998-2004 Gilles Vollant - http://www.winimage.com/zLibDll";

int
main(void)
{
    printf("%s\n", minizip_copyright);
}
