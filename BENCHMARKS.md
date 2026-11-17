# Keel benchmarks
> Keel is still experimental, and more optimizations are still to come.

All times are measured with [hyperfine](https://github.com/sharkdp/hyperfine) (`--runs 150 --warmup 10`). All benchmarks are run on a 2021 M1 Pro Macbook Pro with 16GBs of ram.

Keel release binaries are built with PGO, and the instrumented binary is trained on representative Keel programs, including smaller-input versions of some benchmarks shown here.
The PGO workflow is visible [here](.github/workflows/release.yml).

Native binaries are built with `-O2`. The compiler used is Clang.


## Iterative fib(46) x 200000

| Keel | Python 3.15.2 | LuaJIT (-joff) | Native (C) |
| --- | --- | --- | --- |
| [iter_fib.kl](/examples/iter_fib/iter_fib.kl) | [iter_fib.py](/examples/iter_fib/iter_fib.py) | [iter_fib.lua](/examples/iter_fib/iter_fib.lua) | [iter_fib.c](/examples/iter_fib/iter_fib.c) |
| 40.9ms | 1029ms | 69.4ms | 12.4ms |


## Recursive fib(10,15,20,25,30,33)

| Keel | Python 3.15.2 | LuaJIT (-joff) | Native (C) |
| --- | --- | --- | --- |
| [fib.kl](/examples/fib/fib.kl) | [fib.py](/examples/fib/fib.py) | [fib.lua](/examples/fib/fib.lua) | [fib.c](/examples/fib/fib.c) |
| 92.0ms | 403.1ms | 181.7ms | 16.3ms |


## N-body (N=500000)
Based on [this benchmark from The Computer Language Benchmarks Game](https://benchmarksgame-team.pages.debian.net/benchmarksgame/description/nbody.html#nbody) and [the fastest Lua implementation](https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/nbody-lua-2.html).

| Keel | Python 3.15.2 | LuaJIT (-joff) | Native (C) |
| --- | --- | --- | --- |
| [nbody.kl](/examples/nbody/nbody.kl) | [nbody.py](/examples/nbody/nbody.py) | [nbody.lua](/examples/nbody/nbody.lua) | [nbody.c](/examples/nbody/nbody.c) |
| 373.4ms | 2509ms | 458.5ms | 48.9ms |

## Binary trees (N=16)
Based on [this benchmark from The Computer Language Benchmarks Game](https://benchmarksgame-team.pages.debian.net/benchmarksgame/description/binarytrees.html#binarytrees).\
Based on [this Python implementation](https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/binarytrees-python3-2.html) and [this Lua implementation](https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/binarytrees-lua-2.html).

| Keel | Python 3.15.2 | LuaJIT (-joff) | Native (C) |
| --- | --- | --- | --- |
| [binary-trees.kl](/examples/binary-trees/binary-trees.kl) | [binary-trees.py](/examples/binary-trees/binary-trees.py) | [binary-trees.lua](/examples/binary-trees/binary-trees.lua) | [binary-trees.c](/examples/binary-trees/binary-trees.c) |
| 305.6ms | 1149ms | 1005ms | 247.4ms |


## Quicksort (N=10000)

| Keel | Python 3.15.2 | LuaJIT (-joff) |
| --- | --- | --- |
| [quicksort.kl](/examples/quicksort/quicksort.kl) | [quicksort.py](/examples/quicksort/quicksort.py) | [quicksort.lua](/examples/quicksort/quicksort.lua) |
| 265.6ms | 698.8ms | 1755ms |

## Sqrt (N=0 to 9999999)

| Keel | Python 3.15.2 | LuaJIT (-joff) | Native (C) |
| --- | --- | --- | --- |
| [sqrt.kl](/examples/sqrt/sqrt.kl) | [sqrt.py](/examples/sqrt/sqrt.py) | [sqrt.lua](/examples/sqrt/sqrt.lua) | [sqrt.c](/examples/sqrt/sqrt.c) |
| 42.5ms | 1039ms | 167ms | 2.1ms |

## Matmul 120x120

| Keel | Python 3.15.2 | LuaJIT (-joff) |
| --- | --- | --- |
| [matmul.kl](/examples/matmul/matmul.kl) | [matmul.py](/examples/matmul/matmul.py) | [matmul.lua](/examples/matmul/matmul.lua) |
| 16.9ms | 254.8ms | 38.7ms |

## Naive string building (100000 x "Hello, world!")

| Keel | Python 3.15.2 | LuaJIT (-joff) |
| --- | --- | --- |
| [string_concat.kl](/examples/string_concat/string_concat.kl) | [string_concat.py](/examples/string_concat/string_concat.py) | [string_concat.lua](/examples/string_concat/string_concat.lua) |
| 3.6ms | 1198ms | 7696ms |

## Idiomatic string building (100000 x "Hello, world!")

| Keel | Python 3.15.2 | LuaJIT (-joff) |
| --- | --- | --- |
| [string_building.kl](/examples/string_building/string_building.kl) | [string_building.py](/examples/string_building/string_building.py) | [string_building.lua](/examples/string_building/string_building.lua) |
| 3.6ms | 27.5ms | 4.6ms |

## FizzBuzz - 1 000 000 iterations

> [!IMPORTANT]
> The C program uses a per-line `printf`.

### With `--show-output`
| Keel | Python 3.15.2 | LuaJIT (-joff) | Native (C) |
| --- | --- | --- | --- |
| [fizzbuzz.kl](/examples/fizzbuzz/fizzbuzz.kl) | [fizzbuzz.py](/examples/fizzbuzz/fizzbuzz.py) | [fizzbuzz.lua](/examples/fizzbuzz/fizzbuzz.lua) | [fizzbuzz.c](/examples/fizzbuzz/fizzbuzz.c) |
| 1770ms | 2200ms | 1874ms | 2158 |

### Default
| Keel | Python 3.15.2 | LuaJIT (-joff) | Native (C) |
| --- | --- | --- | --- |
| [fizzbuzz.kl](/examples/fizzbuzz/fizzbuzz.kl) | [fizzbuzz.py](/examples/fizzbuzz/fizzbuzz.py) | [fizzbuzz.lua](/examples/fizzbuzz/fizzbuzz.lua) | [fizzbuzz.c](/examples/fizzbuzz/fizzbuzz.c) |
| 15.8ms | 317.9ms | 99.1ms | 71.5ms |


## Standard library operations * 100 000

Almost all the functions from the standard library are tested, except file system functions to avoid IO interference.

| Keel | Python 3.15.2 | LuaJIT (-joff) |
| --- | --- | --- |
| [stdlib.kl](/examples/stdlib/stdlib.kl) | [stdlib.py](/examples/stdlib/stdlib.py) | [stdlib.lua](/examples/stdlib/stdlib.lua) |
| 31.6ms | 207.5ms | 253.3ms |

## C FFI call overhead * 10 000 000

FFI call overhead is measured by comparing the execution time of an increment loop entirely in the given language against an equivalent loop that performs the increment through an FFI C function call.
The C function is intentionally trivial so that the measured time reflects the cost of crossing the language-C boundary, not the C computation itself.

[ffi.c](/examples/ffi/ffi.c) (compiled with `-O2`):
```c
int increment(int x) { return x + 1; }
```

| Keel | Python 3.15.2 | LuaJIT (-joff) |
| --- | --- | --- |
| [ffi_baseline.kl](/examples/ffi/ffi_baseline.kl) | [ffi_baseline.py](/examples/ffi/ffi_baseline.py) | [ffi_baseline.lua](/examples/ffi/ffi_baseline.lua) |
| 29.2ms | 574.1ms | 62.8ms |
| [ffi.kl](/examples/ffi/ffi.kl) | [ffi.py](/examples/ffi/ffi.py) | [ffi.lua](/examples/ffi/ffi.lua) |
| 154.8ms | 3420ms | 511.6ms |
| **FFI OVERHEAD** | **FFI OVERHEAD** | **FFI OVERHEAD** |
| 125.6ms | 2845.9ms | 448.8ms |