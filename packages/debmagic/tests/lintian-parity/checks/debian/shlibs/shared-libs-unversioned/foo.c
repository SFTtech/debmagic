/*
 * Be sure that this library uses a function from libc.  Otherwise, gcc is
 * smart enough not to link it with libc and we get more tags for missing
 * dependency lines.
 */

#include <stdio.h>
#include <stdlib.h>
#include <math.h>

double e(void (*f)(char *)){
  char tmp[10];
  double x;
  f(tmp);
  x = atof(tmp);
  return exp(x);
}

int
foo(int num)
{
    printf("%d\n", num);
    return num * 42;
}
