# Keel benchmarks
> Keel is still experimental, and more optimizations are still to come.

All times are measured with [hyperfine](https://github.com/sharkdp/hyperfine) (`--runs 150 --warmup 10`). All benchmarks are run on a 2021 M1 Pro Macbook Pro with 16GBs of ram.

Keel release binaries are built with PGO, and the instrumented binary is trained on representative Keel programs, including smaller-input versions of some benchmarks shown here.
The PGO workflow is visible [here](.github/workflows/release.yml).

Native binaries are built with `-O2`. The compiler used is Clang.


## Iterative fib(46) x 200000

| Keel | Python 3.14.5 | LuaJIT (-joff) | Native (C) |
| --- | --- | --- | --- |
| [iter_fib.kl](/examples/iter_fib/iter_fib.kl) | [iter_fib.py](/examples/iter_fib/iter_fib.py) | [iter_fib.lua](/examples/iter_fib/iter_fib.lua) | [iter_fib.c](/examples/iter_fib/iter_fib.c) |
| 73.4ms | 740ms | 72.5ms | 2.1ms |


## Recursive fib(10,15,20,25,30,33)

| Keel | Python 3.14.5 | LuaJIT (-joff) | Native (C) |
| --- | --- | --- | --- |
| [fib.kl](/examples/fib/fib.kl) | [fib.py](/examples/fib/fib.py) | [fib.lua](/examples/fib/fib.lua) | [fib.c](/examples/fib/fib.c) |
| 178.9ms | 507.4ms | 183.4ms | 16.3ms |


## N-body (N=500000)
Based on [this benchmark from The Computer Language Benchmarks Game](https://benchmarksgame-team.pages.debian.net/benchmarksgame/description/nbody.html#nbody) and [the fastest Lua implementation](https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/nbody-lua-2.html).

| Keel | Python 3.14.5 | LuaJIT (-joff) | Native (C) |
| --- | --- | --- | --- |
| [nbody.kl](/examples/nbody/nbody.kl) | [nbody.py](/examples/nbody/nbody.py) | [nbody.lua](/examples/nbody/nbody.lua) | [nbody.c](/examples/nbody/nbody.c) |
| 451.5ms | 2649ms | 458.5ms | 48.9ms |

## Binary trees (N=16)
Based on [this benchmark from The Computer Language Benchmarks Game](https://benchmarksgame-team.pages.debian.net/benchmarksgame/description/binarytrees.html#binarytrees).\
Based on [this Python implementation](https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/binarytrees-python3-2.html) and [this Lua implementation](https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/binarytrees-lua-2.html).

| Keel | Python 3.14.5 | LuaJIT (-joff) | Native (C) |
| --- | --- | --- | --- |
| [binary-trees.kl](/examples/binary-trees/binary-trees.kl) | [binary-trees.py](/examples/binary-trees/binary-trees.py) | [binary-trees.lua](/examples/binary-trees/binary-trees.lua) | [binary-trees.c](/examples/binary-trees/binary-trees.c) |
| 540.7ms | 1264ms | 541.8ms | 247.4ms |


## Quicksort (N=10000)

| Keel | Python 3.14.5 | LuaJIT (-joff) |
| --- | --- | --- |
| [quicksort.kl](/examples/quicksort/quicksort.kl) | [quicksort.py](/examples/quicksort/quicksort.py) | [quicksort.lua](/examples/quicksort/quicksort.lua) |
| 344.5ms | 730.9ms | 1755ms |

## Sqrt (N=0 to 9999999)

<table>
<tr>
  <th>Keel</th>
  <th>Python 3</th>
  <th>LuaJIT (-joff)</th>
</tr>
<tr>
<td><pre><code class="language-rust">fn main() {
    let x = 0.0;
    for i in 0..10000000 {
        x += float(i).sqrt();
    }
    print(x);
}</code></pre></td>
<td><pre><code class="language-python">from math import sqrt

x = 0.0
for i in range(10000000):
    x += sqrt(i)
print(x)</code></pre></td>
<td><pre><code class="language-lua">local x = 0.0
for i = 0, 9999999 do
    x = x + math.sqrt(i)
end
print(x)</code></pre></td>
</tr>
<tr>
  <td><b>76.5ms</b></td>
  <td>1164ms</td>
  <td>167ms</td>
</tr>
</table>


## String.split(), Array.contains() * 50 000

<table>
<tr>
  <th>Keel</th>
  <th>Python 3</th>
  <th>LuaJIT (-joff)</th>
</tr>
<tr>
<td><pre><code class="language-rust">fn main() {
    let s = "the quick brown fox";
    let count = 0;
    for _ in 0..50000 {
        let parts = s.split(" ");
        if parts.contains("fox") {
            count += 1;
        }
    }
    print(count);
}</code></pre></td>
<td><pre><code class="language-python">s = "the quick brown fox"
count = 0
for _ in range(50000):
    parts = s.split(" ")
    if "fox" in parts:
        count += 1
print(count)</code></pre></td>
<td><pre><code class="language-lua">local s = "the quick brown fox"
local count = 0
for _ = 1, 50000 do
    if s:find("fox") then
        count = count + 1
    end
end
print(count)</code></pre></td>
</tr>
<tr>
  <td><b>5.8ms</b></td>
  <td>28.2ms</td>
  <td>27.6ms</td>
</tr>
</table>


## FizzBuzz - 1 000 000 iterations

| Keel | Python 3.14.5 | LuaJIT (-joff) | Native (C) |
| --- | --- | --- | --- |
| [fizzbuzz.kl](/examples/fizzbuzz/fizzbuzz.kl) | [fizzbuzz.py](/examples/fizzbuzz/fizzbuzz.py) | [fizzbuzz.lua](/examples/fizzbuzz/fizzbuzz.lua) | [fizzbuzz.c](/examples/fizzbuzz/fizzbuzz.c) |
| 32.5ms | 304.8ms | 124.3ms | 71.5ms |


## Standard library operations * 100 000

Almost all the functions from the standard library are tested, except file system functions to avoid IO interference.

<table>
<tr>
  <th>Keel</th>
  <th>Python 3</th>
  <th>LuaJIT (-joff)</th>
</tr>
<tr>
<td><pre><code class="language-rust">fn main() {
    let count = 0;
    for _ in 0..100000 {
        let s = "  Hello, World!  ";
        let t = s.trim();
        let tl = s.trim_left();
        let tr = s.trim_right();
        let ts = "-Hello-".trim_sequence("-");
        let tsl = "-Hello-".trim_sequence_left("-");
        let tsr = "-Hello-".trim_sequence_right("-");
        let u = t.uppercase();
        let l = u.lowercase();
        let c = t.contains("World");
        let f = t.find("World");
        let sw = t.starts_with("Hello");
        let ew = t.ends_with("!");
        let isf = "3.14".is_float();
        let isi = "42".is_int();
        let parts = l.split(", ");
        let joined = parts.join("-");
        let r = joined.replace("-", " ");
        let length = r.len();
        let rev = r.reverse();
        let rep = "ab".repeat(3);
        let n = 42.7;
        let sq = n.sqrt();
        let fl = n.floor();
        let ro = n.round();
        let ab = (-5).abs();
        let fab = (-3.14).abs();
        let to_f = float(42);
        let to_i = int(3.14);
        let to_s = str(42);
        let to_b = bool("true");
        let rng = range(10);
        let arr = [3, 1, 4, 1, 5];
        arr.sort();
        arr.reverse();
        count += length;
    }
    print(count);
}</code></pre></td>
<td><pre><code class="language-python">import math
count = 0
for _ in range(100000):
    s = "  Hello, World!  "
    t = s.strip()
    tl = s.lstrip()
    tr = s.rstrip()
    ts = "-Hello-".strip("-")
    tsl = "-Hello-".lstrip("-")
    tsr = "-Hello-".rstrip("-")
    u = t.upper()
    l = u.lower()
    c = "World" in t
    f = t.find("World")
    sw = t.startswith("Hello")
    ew = t.endswith("!")
    isf = True
    isi = True
    parts = l.split(", ")
    joined = "-".join(parts)
    r = joined.replace("-", " ")
    length = len(r)
    rev = r[::-1]
    rep = "ab" * 3
    n = 42.7
    sq = math.sqrt(n)
    fl = math.floor(n)
    ro = round(n)
    ab = abs(-5)
    fab = abs(-3.14)
    to_f = float(42)
    to_i = int(3.14)
    to_s = str(42)
    to_b = bool("true")
    rng = list(range(10))
    arr = [3, 1, 4, 1, 5]
    arr.sort()
    arr.reverse()
    count += length
print(count)</code></pre></td>
<td><pre><code class="language-lua">local count = 0
for _ = 1, 100000 do
    local s = "  Hello, World!  "
    local t = s:match("^%s*(.-)%s*$")
    local tl = s:match("^%s*(.*)")
    local tr = s:match("(.-)%s*$")
    local ts = ("-Hello-"):match("^%-(.-)%-$")
    local tsl = ("-Hello-"):match("^%-(.*)")
    local tsr = ("-Hello-"):match("(.-)%-$")
    local u = t:upper()
    local l = u:lower()
    local c = t:find("World") ~= nil
    local f = t:find("World")
    local sw = t:sub(1,5) == "Hello"
    local ew = t:sub(-1) == "!"
    local isf = tonumber("3.14") ~= nil
    local isi = tonumber("42") ~= nil
    local parts = {}
    for p in l:gmatch("[^,]+") do
        parts[#parts+1] = p
    end
    local joined = table.concat(parts, "-")
    local r = joined:gsub("-", " ")
    local length = #r
    local rev = r:reverse()
    local rep = ("ab"):rep(3)
    local n = 42.7
    local sq = math.sqrt(n)
    local fl = math.floor(n)
    local ro = math.floor(n + 0.5)
    local ab = math.abs(-5)
    local fab = math.abs(-3.14)
    local to_f = 42 + 0.0
    local to_i = math.floor(3.14)
    local to_s = tostring(42)
    local to_b = ("true") == "true"
    local rng = {}
    for i = 0, 9 do rng[#rng+1] = i end
    local arr = {3, 1, 4, 1, 5}
    table.sort(arr)
    local j = 1
    local k = #arr
    while j < k do
        arr[j], arr[k] = arr[k], arr[j]
        j = j + 1
        k = k - 1
    end
    count = count + length
end
print(count)</code></pre></td>
</tr>
<tr>
  <td><b>46.6ms</b></td>
  <td>191.9ms</td>
  <td>253.3ms</td>
</tr>
</table>


## C FFI call overhead * 10 000 000

FFI call overhead is measured by comparing the execution time of an increment loop entirely in the given language against an equivalent loop that performs the increment through an FFI C function call. 
The C function is intentionally trivial so that the measured time reflects the cost of crossing the language-C boundary, not the C computation itself.

[ffi.c](/examples/ffi/ffi.c) (compiled with `-O2`):
```c
int increment(int x) { return x + 1; }
```

| Keel | Python 3.14.5 | LuaJIT (-joff) |
| --- | --- | --- |
| [ffi_baseline.kl](/examples/ffi/ffi_baseline.kl) | [ffi_baseline.py](/examples/ffi/ffi_baseline.py) | [ffi_baseline.lua](/examples/ffi/ffi_baseline.lua) |
| 48.4ms | 485.7ms | 62.8ms |
| [ffi.kl](/examples/ffi/ffi.kl) | [ffi.py](/examples/ffi/ffi.py) | [ffi.lua](/examples/ffi/ffi.lua) |
| 177.4ms | 2860ms | 511.6ms |
| **FFI OVERHEAD** | **FFI OVERHEAD** | **FFI OVERHEAD** |
| 129ms | 2374.3ms | 448.8ms |