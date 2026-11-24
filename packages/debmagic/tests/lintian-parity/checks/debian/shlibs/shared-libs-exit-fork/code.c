#include <stdlib.h>
#include <unistd.h>

double e(void (*f)(char *)){
  char tmp[10];
  double x;
  f(tmp);
  x = atof(tmp);
  if (fork() != 0)
    exit(1);
  return x;
}
