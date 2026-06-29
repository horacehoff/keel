#ifdef _WIN32
#define EXPORT __declspec(dllexport)
#else
#define EXPORT __attribute__((visibility("default")))
#endif

#include <math.h>
#include <stdint.h>

EXPORT double keel_acos(double x) { return acos(x); }
EXPORT double keel_asin(double x) { return asin(x); }
EXPORT double keel_atan(double x) { return atan(x); }
EXPORT double keel_atan2(double x, double y) { return atan2(x, y); }
EXPORT double keel_cos(double x) { return cos(x); }
EXPORT double keel_sin(double x) { return sin(x); }
EXPORT double keel_tan(double x) { return tan(x); }
EXPORT double keel_acosh(double x) { return acosh(x); }
EXPORT double keel_asinh(double x) { return asinh(x); }
EXPORT double keel_atanh(double x) { return atanh(x); }
EXPORT double keel_cosh(double x) { return cosh(x); }
EXPORT double keel_sinh(double x) { return sinh(x); }
EXPORT double keel_tanh(double x) { return tanh(x); }
EXPORT double keel_exp(double x) { return exp(x); }
EXPORT double keel_expm1(double x) { return expm1(x); }
EXPORT double keel_log(double x) { return log(x); }
EXPORT double keel_log10(double x) { return log10(x); }
EXPORT double keel_log2(double x) { return log2(x); }
EXPORT double keel_log1p(double x) { return log1p(x); }
EXPORT double keel_logb(double x) { return logb(x); }
EXPORT double keel_ldexp(double x, int y) { return ldexp(x, y); }
EXPORT int32_t keel_ilogb(double x) { return (int32_t)ilogb(x); }
EXPORT double keel_scalbn(double x, int y) { return scalbn(x, y); }
EXPORT double keel_cbrt(double x) { return cbrt(x); }
EXPORT double keel_hypot(double x, double y) { return hypot(x, y); }
EXPORT double keel_erf(double x) { return erf(x); }
EXPORT double keel_erfc(double x) { return erfc(x); }