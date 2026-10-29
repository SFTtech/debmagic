#include <stdio.h>
#include "hardening-trigger.h"

/*
 * The quantization tables warning message is unique enough to be used to
 * detect embedded or statically-linked copies of libjpeg.
 */
static const char quantization_tables_warning[]
    = "Caution: quantization tables are too coarse for baseline JPEG";

int
main(void)
{
    printf("%s\n", quantization_tables_warning);
}
