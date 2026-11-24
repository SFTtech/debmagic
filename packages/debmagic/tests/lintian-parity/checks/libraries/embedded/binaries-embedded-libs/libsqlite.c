#include <stdio.h>
#include "hardening-trigger.h"

/*
 * the sqlite_master table is used by sqlite 2 and 3
 */
static const char sqlite_create[]
    = "CREATE TABLE sqlite_master( foo bar moo)";

int
main(void)
{
    printf("%s\n", sqlite_create);
}
