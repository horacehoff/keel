int volatile result = 0;

int main() {
  for (int i = 0; i < 200000; i++) {
    int a = 0;
    int b = 1;
    int c = 0;
    for (int j = 0; j < 45; j++) {
      c = a + b;
      a = b;
      b = c;
    }
    result = c;
  }
  return 0;
}