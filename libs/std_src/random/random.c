#ifdef _WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT __attribute__((visibility("default")))
#endif

#include "pcg_basic.h"
#include <math.h>
#include <stdint.h>
#include <time.h>

static pcg32_random_t rng;
static int8_t seeded = 0;

inline void seed(void) {
  pcg32_srandom_r(&rng, time(NULL), clock());
  seeded = 1;
}

EXPORT void keel_seed(int32_t seed) {
  pcg32_srandom_r(&rng, (uint64_t)(uint32_t)seed, 54u);
  seeded = 1;
}

EXPORT int32_t keel_random_int(void) {
  if (seeded == 0) {
    seed();
  }
  return (int32_t)pcg32_random_r(&rng);
}

EXPORT int32_t keel_random_int_range(int32_t min, int32_t max) {
  if (seeded == 0) {
    seed();
  }
  return min + (int32_t)pcg32_boundedrand_r(&rng, max - min + 1);
}

EXPORT double keel_random(void) {
  if (seeded == 0) {
    seed();
  }
  return ldexp(pcg32_random_r(&rng), -32);
}

EXPORT double keel_random_float_range(double min, double max) {
  if (seeded == 0) {
    seed();
  }
  return min + (keel_random() * (max - min));
}