#include <string.h>
#include <stdio.h>

void e(char *p, int i, void (*f)(char *)){
  char test[10];
  memcpy(test, p, i);
  f(test);
  printf("%s", test);
}
