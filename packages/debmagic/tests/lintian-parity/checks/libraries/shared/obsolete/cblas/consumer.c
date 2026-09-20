#include <assert.h>

#include "cblas.h"

int main(void)
{
    float array[] = {1.,-1.,1.,-1.};
    assert(4 == sasum(4, array, 1));

    return 0;
}
