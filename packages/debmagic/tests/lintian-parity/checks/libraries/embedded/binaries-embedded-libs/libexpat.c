#include <stdio.h>
#include "hardening-trigger.h"

/*
 * The XML_DTD warning string is always present, even if expat was
 * built with XML_DTD
 */
static const char xml_dtd[]
    = "requested feature requires XML_DTD support in Expat";

int
main(void)
{
    printf("%s\n", xml_dtd);
}
