#include <math.h>
#include <string.h>

double pw(double p){
  return exp(p);
}

void stackprotfix(void (*f)(char *, size_t)) {
  char buffer[10];
  f(buffer, sizeof(buffer));
}
