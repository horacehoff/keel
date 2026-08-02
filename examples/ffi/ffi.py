import ctypes

lib = ctypes.CDLL("./ffi.dylib")
lib.increment.restype = ctypes.c_int
lib.increment.argtypes = [ctypes.c_int]

x = 0
for _ in range(10_000_000):
    x = lib.increment(x)
print(x)