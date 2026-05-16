> Keel is experimental — more optimizations are still to come.

All times are measured with [hyperfine](https://github.com/sharkdp/hyperfine) (`--runs 150 --warmup 10`). All benchmarks are run on the same machine.

---

## Iterative Fibonacci - fib(40) × 200 000

<table>
<tr>
  <th>Keel</th>
  <th>Python 3</th>
  <th>LuaJIT (-joff)</th>
</tr>
<tr>
<td><pre><code class="language-rust">function main() {
    for _ in 0..200000 {
        let a = 0;
        let b = 1;
        let c = 0;
        for i in 0..39 {
            c = a + b;
            a = b;
            b = c;
        }
    }
}</code></pre></td>
<td><pre><code class="language-python">for _ in range(200000):
    a, b, c = 0, 1, 0
    for i in range(39):
        c = a + b
        a = b
        b = c</code></pre></td>
<td><pre><code class="language-lua">for r = 0, 199999 do
    local a, b, c = 0, 1, 0
    for i = 0, 38 do
        c = a + b
        a = b
        b = c
    end
end</code></pre></td>
</tr>
<tr>
  <td><b>59.1ms</b></td>
  <td>668ms</td>
  <td>60.2ms</td>
</tr>
</table>

---

## Recursive Fibonacci - fib(30)

<table>
<tr>
  <th>Keel</th>
  <th>Python 3</th>
  <th>LuaJIT (-joff)</th>
</tr>
<tr>
<td><pre><code class="language-rust">function fib(n) {
    if n <= 1 { return n; }
    return fib(n - 1) + fib(n - 2);
}

function main() {
    print(fib(30));
}</code></pre></td>
<td><pre><code class="language-python">def fib(n):
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

print(fib(30))</code></pre></td>
<td><pre><code class="language-lua">local function fib(n)
    if n <= 1 then return n end
    return fib(n - 1) + fib(n - 2)
end

print(fib(30))</code></pre></td>
</tr>
<tr>
  <td><b>41.1ms</b></td>
  <td>111.8ms</td>
  <td>36.5ms</td>
</tr>
</table>

---

## Multiply, branch, modulo × 1 000 000

<table>
<tr>
  <th>Keel</th>
  <th>Python 3</th>
  <th>LuaJIT (-joff)</th>
</tr>
<tr>
<td><pre><code class="language-rust">function main() {
    let count = 0;
    let result = 1;
    while count < 1000000 {
        result *= 2;
        if result > 1000000 {
            result %= 1000000;
        }
        count += 1;
    }
    print(result);
}</code></pre></td>
<td><pre><code class="language-python">count = 0
result = 1
while count < 1000000:
    result *= 2
    if result > 1000000:
        result %= 1000000
    count += 1
print(result)</code></pre></td>
<td><pre><code class="language-lua">local count = 0
local result = 1
while count < 1000000 do
    result = result * 2
    if result > 1000000 then
        result = result % 1000000
    end
    count = count + 1
end
print(result)</code></pre></td>
</tr>
<tr>
  <td><b>18.5ms</b></td>
  <td>136ms</td>
  <td>25.6ms</td>
</tr>
</table>

---

## Sqrt × 10 000 000

<table>
<tr>
  <th>Keel</th>
  <th>Python 3</th>
  <th>LuaJIT (-joff)</th>
</tr>
<tr>
<td><pre><code class="language-rust">function main() {
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
  <td><b>99.8ms</b></td>
  <td>1164ms</td>
  <td>167ms</td>
</tr>
</table>

---

## Sieve of Eratosthenes up to 100 000

<table>
<tr>
  <th>Keel</th>
  <th>Python 3</th>
  <th>LuaJIT (-joff)</th>
</tr>
<tr>
<td><pre><code class="language-rust">function main() {
    let limit = 100000;
    let sieve = range(limit);
    sieve[0] = 0;
    sieve[1] = 0;
    let i = 2;
    while i * i <= limit {
        if sieve[i] != 0 {
            let j = i * i;
            while j < limit {
                sieve[j] = 0;
                j += i;
            }
        }
        i += 1;
    }
    let count = 0;
    for x in sieve {
        if x != 0 { count += 1; }
    }
    print(count);
}</code></pre></td>
<td><pre><code class="language-python">limit = 100000
sieve = list(range(limit))
sieve[0] = 0
sieve[1] = 0
i = 2
while i * i <= limit:
    if sieve[i]:
        j = i * i
        while j < limit:
            sieve[j] = 0
            j += i
    i += 1
count = sum(1 for x in sieve if x)
print(count)</code></pre></td>
<td><pre><code class="language-lua">local limit = 100000
local sieve = {}
for i = 0, limit - 1 do
    sieve[i] = i
end
sieve[0] = 0
sieve[1] = 0
local i = 2
while i * i <= limit do
    if sieve[i] ~= 0 then
        local j = i * i
        while j < limit do
            sieve[j] = 0
            j = j + i
        end
    end
    i = i + 1
end
local count = 0
for _, v in pairs(sieve) do
    if v ~= 0 then count = count + 1 end
end
print(count)</code></pre></td>
</tr>
<tr>
  <td><b>6.2ms</b></td>
  <td>40ms</td>
  <td>7ms</td>
</tr>
</table>

---

## String operations, array split and search × 50 000

<table>
<tr>
  <th>Keel</th>
  <th>Python 3</th>
  <th>LuaJIT (-joff)</th>
</tr>
<tr>
<td><pre><code class="language-rust">function main() {
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
  <td><b>7.7ms</b></td>
  <td>32ms</td>
  <td>27ms</td>
</tr>
</table>

---

## FizzBuzz - 1 000 000 iterations

<table>
<tr>
  <th>Keel</th>
  <th>Python 3</th>
  <th>LuaJIT (-joff)</th>
</tr>
<tr>
<td><pre><code class="language-rust">function main() {
    let last = "";
    for i in 1..1000001 {
        if i % 15 == 0 {
            last = "FizzBuzz";
        } else if i % 3 == 0 {
            last = "Fizz";
        } else if i % 5 == 0 {
            last = "Buzz";
        } else {
            last = str(i);
        }
    }
    print(last);
}</code></pre></td>
<td><pre><code class="language-python">last = ""
for i in range(1, 1000001):
    if i % 15 == 0:
        last = "FizzBuzz"
    elif i % 3 == 0:
        last = "Fizz"
    elif i % 5 == 0:
        last = "Buzz"
    else:
        last = str(i)
print(last)</code></pre></td>
<td><pre><code class="language-lua">local last = ""
for i = 1, 1000000 do
    if i % 15 == 0 then
        last = "FizzBuzz"
    elseif i % 3 == 0 then
        last = "Fizz"
    elseif i % 5 == 0 then
        last = "Buzz"
    else
        last = tostring(i)
    end
end
print(last)</code></pre></td>
</tr>
<tr>
  <td><b>26.9ms</b></td>
  <td>171ms</td>
  <td>82.4ms</td>
</tr>
</table>

---

## Standard library operations × 100 000

<table>
<tr>
  <th>Keel</th>
  <th>Python 3</th>
  <th>LuaJIT (-joff)</th>
</tr>
<tr>
<td><pre><code class="language-rust">function main() {
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
  <td><b>53.67ms</b></td>
  <td>191.9ms</td>
  <td>253.3ms</td>
</tr>
</table>

---

## C FFI call overhead × 10 000 000

All programs call the same shared C library function in a tight loop. The C function is intentionally trivial so the measured time reflects the cost of crossing the language–C boundary, not the C computation itself.

**`bench_ffi.c`** (compiled with `-O2`):
```c
int increment(int x) {
    return x + 1;
}
```

<table>
<tr>
  <th>Keel</th>
  <th>Python 3</th>
  <th>LuaJIT (-joff)</th>
</tr>
<tr>
<td><pre><code class="language-rust">import "./bench_ffi.dylib" {
    int increment(int);
}

function main() {
    let x = 0;
    for _ in 0..10000000 {
        x = bench_ffi::increment(x);
    }
    print(x);
}</code></pre></td>
<td><pre><code class="language-python">import ctypes

lib = ctypes.CDLL("./bench_ffi.dylib")
lib.increment.restype = ctypes.c_int
lib.increment.argtypes = [ctypes.c_int]

x = 0
for _ in range(10_000_000):
    x = lib.increment(x)
print(x)</code></pre></td>
<td><pre><code class="language-lua">local ffi = require("ffi")
ffi.cdef[[
    int increment(int x);
]]
local lib = ffi.load("./bench_ffi")

local x = 0
for _ = 1, 10000000 do
    x = lib.increment(x)
end
print(x)</code></pre></td>
</tr>
<tr>
  <td><b>211.6ms</b></td>
  <td>2907ms</td>
  <td>535.8ms</td>
</tr>
</table>