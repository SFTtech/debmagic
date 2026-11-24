#include <stdlib.h>
#include <math.h>
#include "code.h"

double e(void (*f)(char *)){
  char tmp[10];
  double x;
  f(tmp);
  x = atof(tmp);
  return exp(x);
}

double energy(double mass){
  return pow(10.0, 8.0) * pow(3.0, 2.0) * mass;
}

