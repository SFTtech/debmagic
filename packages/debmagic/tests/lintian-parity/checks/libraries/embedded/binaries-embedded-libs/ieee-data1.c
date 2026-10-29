#include <stdio.h>

static const char ieee_data_url[]
    = "http://standards.ieee.org/develop/regauth/oui/oui.txt";

int
main(void)
{
    printf("%s\n", ieee_data_url);
}
