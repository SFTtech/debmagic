#include <stdlib.h>
#include <math.h>

double e(void (*f)(char *)){
  char tmp[10];
  double x;
  f(tmp);
  x = atof(tmp);
  return exp(x);
}
