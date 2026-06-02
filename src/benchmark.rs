use inline_colorization::*;
use std::fs;
use std::process::Command;

const BENCHMARK_RUNS: u16 = 150;
const BENCHMARK_WARMUP_RUNS: u16 = 10;

struct Benchmark {
    name: &'static str,
    source: &'static str,
    python: &'static str,
    lua: &'static str,
}

const PROGRAMS: &[Benchmark] = &[
    Benchmark {
        name: "iter_fib_40_x_200000",
        source: r#"
fn main() {
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
}
        "#,
        python: r#"
for _ in range(200000):
    a = 0
    b = 1
    c = 0
    for i in range(39):
        c = a + b
        a = b
        b = c
                "#,
        lua: r#"
for _ = 1, 200000 do
    local a = 0
    local b = 1
    local c = 0
    for i = 1, 39 do
        c = a + b
        a = b
        b = c
    end
end
        "#,
    },
    Benchmark {
        name: "rec_fib_30",
        source: r#"
fn fib(n) {
    if n <= 1 { return n; }
    return fib(n - 1) + fib(n - 2);
}

fn main() {
    print(fib(30));
}
        "#,
        python: r#"
def fib(n):
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

print(fib(30))
"#,
        lua: r#"
local function fib(n)
    if n <= 1 then return n end
    return fib(n - 1) + fib(n - 2)
end

print(fib(30))
"#,
    },
    Benchmark {
        name: "multiply_branch_modulo_x_1000000",
        source: r#"
fn main() {
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
}
        "#,
        python: r#"
count = 0
result = 1
while count < 1000000:
    result *= 2
    if result > 1000000:
        result %= 1000000
    count += 1
print(result)
"#,

        lua: r#"
local count = 0
local result = 1
while count < 1000000 do
    result = result * 2
    if result > 1000000 then
        result = result % 1000000
    end
    count = count + 1
end
print(result)
"#,
    },
    Benchmark {
        name: "sqrt_x_10000000",
        source: r#"
fn main() {
    let x = 0.0;
    for i in 0..10000000 {
        x += float(i).sqrt();
    }
    print(x);
}
        "#,
        python: r#"
import math
x = 0.0
for i in range(10000000):
    x += math.sqrt(float(i))
print(x)
"#,

        lua: r#"
local x = 0.0
for i = 0, 9999999 do
    x = x + math.sqrt(i)
end
print(x)
"#,
    },
    Benchmark {
        name: "sieve_100000",
        source: r#"
fn main() {
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
}
        "#,
        python: r#"
limit = 100000
sieve = list(range(limit))
sieve[0] = 0
sieve[1] = 0
i = 2
while i * i <= limit:
    if sieve[i] != 0:
        j = i * i
        while j < limit:
            sieve[j] = 0
            j += i
    i += 1
count = sum(1 for x in sieve if x != 0)
print(count)
"#,
        lua: r#"
local limit = 100000
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
for x = 0, limit - 1 do
    if sieve[x] ~= 0 then count = count + 1 end
end
print(count)
"#,
    },
    Benchmark {
        name: "string_ops_array_split_search_x_50000",
        source: r#"
fn main() {
    let s = "the quick brown fox";
    let count = 0;
    for _ in 0..50000 {
        let parts = s.split(" ");
        if parts.contains("fox") {
            count += 1;
        }
    }
    print(count);
}
        "#,
        python: r#"
s = "the quick brown fox"
count = 0
for _ in range(50000):
    parts = s.split(" ")
    if "fox" in parts:
        count += 1
print(count)
"#,
        lua: r#"
local function split(s, sep)
    local parts = {}
    local pattern = "([^" .. sep .. "]+)"
    for part in s:gmatch(pattern) do
        parts[#parts + 1] = part
    end
    return parts
end

local function contains(tbl, val)
    for _, v in ipairs(tbl) do
        if v == val then return true end
    end
    return false
end

local s = "the quick brown fox"
local count = 0
for _ = 1, 50000 do
    local parts = split(s, " ")
    if contains(parts, "fox") then
        count = count + 1
    end
end
print(count)
"#,
    },
    Benchmark {
        name: "fizzbuzz_x_1000000",
        source: r#"
fn main() {
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
}
        "#,
        python: r#"
last = ""
for i in range(1, 1000001):
    if i % 15 == 0:
        last = "FizzBuzz"
    elif i % 3 == 0:
        last = "Fizz"
    elif i % 5 == 0:
        last = "Buzz"
    else:
        last = str(i)
print(last)
    "#,
        lua: r#"
    local last = ""
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
    print(last)
    "#,
    },
    Benchmark {
        name: "stdlib_ops_x_100000",
        source: r#"
fn main() {
    let count = 0;
    for _ in 0..100000 {
        let s = "  Hello, World!  ";
        // Trim variants
        let t = s.trim();
        let tl = s.trim_left();
        let tr = s.trim_right();
        let ts = "-Hello-".trim_sequence("-");
        let tsl = "-Hello-".trim_sequence_left("-");
        let tsr = "-Hello-".trim_sequence_right("-");
        // Case
        let u = t.uppercase();
        let l = u.lowercase();
        // Search
        let c = t.contains("World");
        let f = t.find("World");
        let sw = t.starts_with("Hello");
        let ew = t.ends_with("!");
        // Type checks
        let isf = "3.14".is_float();
        let isi = "42".is_int();
        // Split/Join/Replace
        let parts = l.split(", ");
        let joined = parts.join("-");
        let r = joined.replace("-", " ");
        // Len
        let length = r.len();
        // Reverse (returning)
        let rev = r.reverse();
        // Repeat
        let rep = "ab".repeat(3);
        // Numeric
        let n = 42.7;
        let sq = n.sqrt();
        let fl = n.floor();
        let ro = n.round();
        let ab = (-5).abs();
        let fab = (-3.14).abs();
        // Conversions
        let to_f = float(42);
        let to_i = int(3.14);
        let to_s = str(42);
        let to_b = bool("true");
        // Range
        let rng = range(10);
        // Sort (void)
        let arr = [3, 1, 4, 1, 5];
        arr.sort();
        // Reverse (void)
        arr.reverse();
        count += length;
    }
    print(count);
}
        "#,
        python: r#"
import math
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
    isf = True  # no direct equivalent
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
    to_b = bool("true")  # not equivalent but close
    rng = list(range(10))
    arr = [3, 1, 4, 1, 5]
    arr.sort()
    arr.reverse()
    count += length
print(count)
"#,
        lua: r#"
local count = 0
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
    for p in l:gmatch("[^,]+") do parts[#parts+1] = p end
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
print(count)
"#,
    },
];

#[cold]
#[inline(never)]
pub fn benchmark() {
    let exe = std::env::current_exe().unwrap();
    // let temp_dir = std::env::current_dir().unwrap();
    let temp_dir = std::env::temp_dir().join(format!("keel-bench-{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap();

    fn has_command(cmd: &str) -> bool {
        Command::new(if cfg!(target_os = "windows") {
            "where"
        } else {
            "which"
        })
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    }

    if !has_command("python3") || !has_command("luajit") {
        eprintln!("{color_yellow}ERROR{color_reset}: python3 or luajit not found.");
        std::process::exit(1);
    }

    let csv_path = temp_dir.join("hyperfine.csv");
    let mut hyperfine = Command::new("hyperfine");
    hyperfine
        .stdout(std::process::Stdio::inherit())
        .arg("--show-output")
        .arg("--warmup")
        .arg(BENCHMARK_WARMUP_RUNS.to_string())
        .arg("--runs")
        .arg(BENCHMARK_RUNS.to_string())
        .arg("--export-csv")
        .arg(&csv_path);

    for program in PROGRAMS {
        // Keel
        let keel_path = temp_dir.join(format!("{}.kl", program.name));
        fs::write(&keel_path, program.source).unwrap();
        hyperfine
            .arg("--command-name")
            .arg(format!("{} [keel]", program.name))
            .arg(format!(
                "'{}' '{}'",
                &exe.to_string_lossy(),
                &keel_path.to_string_lossy()
            ));

        // Python
        let py_path = temp_dir.join(format!("{}.py", program.name));
        fs::write(&py_path, program.python).unwrap();
        hyperfine
            .arg("--command-name")
            .arg(format!("{} [python]", program.name))
            .arg(format!("python3.15 '{}'", &py_path.to_string_lossy()));

        // LuaJIT (-joff)
        let lua_path = temp_dir.join(format!("{}.lua", program.name));
        fs::write(&lua_path, program.lua).unwrap();
        hyperfine
            .arg("--command-name")
            .arg(format!("{} [luajit]", program.name))
            .arg(format!("luajit -joff '{}'", &lua_path.to_string_lossy()));
    }

    let output = hyperfine.output().unwrap();
    if !output.status.success() {
        eprintln!(
            "{color_red}KEEL ERROR{color_reset}\nhyperfine failed with exit code {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let csv_content = fs::read_to_string(&csv_path).unwrap();
    // Csv content looks like this:
    //
    //command,mean,stddev,median,user,system,min,max
    //iter_fib_40_x_200000 [keel],0.06368376776000001,0.004353804194680242,0.06258985454000002,0.05942135999999998,0.00205494,0.06025604204,0.09224020804000001
    //iter_fib_40_x_200000 [python],0.6372500478133335,0.04141564910220596,0.6282088545400001,0.6150846466666667,0.010579173333333336,0.6062124160400001,1.03219041604
    //iter_fib_40_x_200000 [luajit],0.06374261062000001,0.002347164374014165,0.06312772954000001,0.06067061999999999,0.0016885266666666662,0.06117808304000001,0.08365400004000001
    //rec_fib_30 [keel],0.04416020171333333,0.005159782665204017,0.04272989604000001,0.04021073333333335,0.001942013333333333,0.040943459040000005,0.08020116604000001
    //rec_fib_30 [python],0.08872272000000003,0.007319715036770666,0.08681718804,0.07984067999999996,0.00575162,0.08479912504,0.14347783404

    let mut results: Vec<(String, f64)> = Vec::new();
    for line in csv_content.lines().skip(1) {
        let mut cols = line.split(',');
        let name = cols.next().unwrap().to_string();
        let mean = cols.next().unwrap().parse::<f64>().unwrap() * 1000.0;
        results.push((name, mean));
    }

    // Group by program and print relative speedup ratios
    println!();
    for program in PROGRAMS {
        let keel_time = results
            .iter()
            .find(|(n, _)| n == &format!("{} [keel]", program.name))
            .map(|(_, v)| *v)
            .unwrap();

        println!("{color_cyan}{}{color_reset}", program.name);
        println!("  {color_blue}keel  {color_reset}: {keel_time:.3} ms");

        if let Some((_, ms)) = results
            .iter()
            .find(|(n, _)| n == &format!("{} [python]", program.name))
        {
            let ratio = ms / keel_time;
            println!(
                "  {color_yellow}python {color_reset}: {ms:.3} ms  ({:.2}x {})",
                if ratio >= 1.0 { ratio } else { 1.0 / ratio },
                if ratio >= 1.0 { "slower" } else { "faster" }
            );
        }

        if let Some((_, ms)) = results
            .iter()
            .find(|(n, _)| n == &format!("{} [luajit]", program.name))
        {
            let ratio = ms / keel_time;
            println!(
                "  {color_green}luajit {color_reset}: {ms:.3} ms  ({:.2}x {})",
                if ratio >= 1.0 { ratio } else { 1.0 / ratio },
                if ratio >= 1.0 { "slower" } else { "faster" }
            );
        }

        println!();
    }
}
