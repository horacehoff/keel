#ifdef _WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT __attribute__((visibility("default")))
#endif

#include <time.h>

EXPORT int now(void) { return (int)time(NULL); }
EXPORT const char *format(int timestamp, const char *fmt) {
  static char buffer[128];
  time_t t = (time_t)timestamp;
  struct tm *info = localtime(&t);
  strftime(buffer, 128, fmt, info);
  return buffer;
}