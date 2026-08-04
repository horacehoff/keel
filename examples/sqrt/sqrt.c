#include <math.h>
#include <stdio.h>

int main() {
  double x = 0.0;
  for (int i = 0; i < 10000000; i++) {
    x += sqrt((double)i);
  }
  printf("%f", x);
  return 0;
}