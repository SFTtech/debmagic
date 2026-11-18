#include <stdlib.h>
#include <math.h>

float sasum(int n, float* x, int incx) {
   float s = 0;
   for (int i = 0; i < n; i++)
       s += abs(*(x+i));
   return s;
}
