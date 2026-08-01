#include <stdio.h>

int fib(int n) {
  if (n <= 1) {
    return n;
  } else {
    return fib(n - 1) + fib(n - 2);
  }
}

int main() {
  volatile int n1 = 10;
  volatile int n2 = 15;
  volatile int n3 = 20;
  volatile int n4 = 25;
  volatile int n5 = 30;
  volatile int n6 = 33;

  printf("%d\n", fib(n1));
  printf("%d\n", fib(n2));
  printf("%d\n", fib(n3));
  printf("%d\n", fib(n4));
  printf("%d\n", fib(n5));
  printf("%d\n", fib(n6));
  return 0;
}