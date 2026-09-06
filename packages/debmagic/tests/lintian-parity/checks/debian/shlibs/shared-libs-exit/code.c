#include <stdlib.h>

double e(void (*f)(char *)){
  char tmp[10];
  double x;
  f(tmp);
  x = atof(tmp);
  if (x < 0.0) {
    exit(1);
  } else {
    return x;
  }
}
