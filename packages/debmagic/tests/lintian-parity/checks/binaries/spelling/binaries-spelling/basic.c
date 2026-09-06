#include <stdio.h>
#include <string.h>

static void
hardening_trigger(char *p, int i, void (*f)(char *))
{
    char test[10];
    memcpy(test, p, i);
    f(test);
    printf("%s", test);
}

int
main(void)
{
    printf("I iz an exprimental speling error!\n");
    printf("I also have teh broken teh!\n");
    printf("But tEH non-broken tEh needs to be on its own line!\n");
    printf("res.size is okay!\n"); /* #818003 */
    printf("Georg Nees was early pioneer of computer art and generative graphics.\n");
    hardening_trigger(NULL, 0, NULL);
    return 0;
}
