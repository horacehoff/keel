use inline_colorization::*;
use std::fs;
use std::process::Command;

const BENCHMARK_RUNS: u16 = 150;
const BENCHMARK_WARMUP: u16 = 10;

struct Prog {
    name: &'static str,
    source: &'static str,
    python: Option<&'static str>,
    lua: Option<&'static str>,
}

const PROGRAMS: &[Prog] = &[
    Prog {
        name: "iter_fib_40_x_200000",
        source: r#"
function main() {
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
        python: Some(
            r#"
for _ in range(200000):
    a = 0
    b = 1
    c = 0
    for i in range(39):
        c = a + b
        a = b
        b = c
                "#,
        ),
        lua: Some(
            r#"
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
        ),
    },
    Prog {
        name: "rec_fib_30",
        source: r#"
function fib(n) {
    if n <= 1 { return n; }
    return fib(n - 1) + fib(n - 2);
}

function main() {
    print(fib(30));
}
        "#,
        python: Some(
            r#"
def fib(n):
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)

print(fib(30))
"#,
        ),
        lua: Some(
            r#"
local function fib(n)
    if n <= 1 then return n end
    return fib(n - 1) + fib(n - 2)
end

print(fib(30))
"#,
        ),
    },
    Prog {
        name: "multiply_branch_modulo_x_1000000",
        source: r#"
function main() {
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
        python: Some(
            r#"
count = 0
result = 1
while count < 1000000:
    result *= 2
    if result > 1000000:
        result %= 1000000
    count += 1
print(result)
"#,
        ),
        lua: Some(
            r#"
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
        ),
    },
    Prog {
        name: "sqrt_x_10000000",
        source: r#"
function main() {
    let x = 0.0;
    for i in 0..10000000 {
        x += float(i).sqrt();
    }
    print(x);
}
        "#,
        python: Some(
            r#"
import math
x = 0.0
for i in range(10000000):
    x += math.sqrt(float(i))
print(x)
"#,
        ),
        lua: Some(
            r#"
local x = 0.0
for i = 0, 9999999 do
    x = x + math.sqrt(i)
end
print(x)
"#,
        ),
    },
    Prog {
        name: "sieve_100000",
        source: r#"
function main() {
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
        python: Some(
            r#"
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
        ),
        lua: Some(
            r#"
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
        ),
    },
    Prog {
        name: "string_ops_array_split_search_x_50000",
        source: r#"
function main() {
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
        python: Some(
            r#"
s = "the quick brown fox"
count = 0
for _ in range(50000):
    parts = s.split(" ")
    if "fox" in parts:
        count += 1
print(count)
"#,
        ),
        lua: Some(
            r#"
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
        ),
    },
    Prog {
        name: "fizzbuzz_x_1000000",
        source: r#"
function main() {
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
        python: Some(
            r#"
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
        ),
        lua: Some(
            r#"
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
        ),
    },
];

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

#[cold]
#[inline(never)]
pub fn benchmark() {
    let exe = std::env::current_exe().unwrap_or_else(|e| {
        eprintln!("{color_red}KEEL ERROR{color_reset}\nCannot locate current executable: {e}");
        std::process::exit(1);
    });
    let temp_dir = std::env::temp_dir().join(format!("keel-bench-{}", std::process::id()));
    fs::create_dir_all(&temp_dir).unwrap_or_else(|e| {
        eprintln!(
            "{color_red}KEEL ERROR{color_reset}\nCannot create benchmark directory {}: {e}",
            temp_dir.display()
        );
        std::process::exit(1);
    });

    let has_python = has_command("python3");
    let has_lua = has_command("luajit");

    if !has_python {
        eprintln!(
            "{color_yellow}WARNING{color_reset}: python3 not found, skipping Python benchmarks"
        );
    }
    if !has_lua {
        eprintln!("{color_yellow}WARNING{color_reset}: luajit not found, skipping Lua benchmarks");
    }

    let json_path = temp_dir.join("hyperfine.json");
    let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let mut hyperfine = Command::new("hyperfine");
    hyperfine
        .stdout(std::process::Stdio::inherit())
        .arg("--show-output")
        .arg("--warmup")
        .arg(BENCHMARK_WARMUP.to_string())
        .arg("--runs")
        .arg(BENCHMARK_RUNS.to_string())
        .arg("--export-json")
        .arg(&json_path);

    for program in PROGRAMS {
        // Keel
        let keel_path = temp_dir.join(format!("{}.kl", program.name));
        fs::write(&keel_path, program.source).unwrap_or_else(|e| {
            eprintln!(
                "{color_red}KEEL ERROR{color_reset}\nCannot write benchmark program {}: {e}",
                keel_path.display()
            );
            std::process::exit(1);
        });
        hyperfine
            .arg("--command-name")
            .arg(format!("{} [keel]", program.name))
            .arg(format!(
                "{} {}",
                quote(&exe.to_string_lossy()),
                quote(&keel_path.to_string_lossy())
            ));

        // Python
        if has_python && let Some(py_src) = program.python {
            let py_path = temp_dir.join(format!("{}.py", program.name));
            fs::write(&py_path, py_src).unwrap_or_else(|e| {
                eprintln!(
                    "{color_red}KEEL ERROR{color_reset}\nCannot write Python benchmark {}: {e}",
                    py_path.display()
                );
                std::process::exit(1);
            });
            hyperfine
                .arg("--command-name")
                .arg(format!("{} [python]", program.name))
                .arg(format!("python3 {}", quote(&py_path.to_string_lossy())));
        }

        // LuaJIT (-joff)
        if has_lua && let Some(lua_src) = program.lua {
            let lua_path = temp_dir.join(format!("{}.lua", program.name));
            fs::write(&lua_path, lua_src).unwrap_or_else(|e| {
                eprintln!(
                    "{color_red}KEEL ERROR{color_reset}\nCannot write Lua benchmark {}: {e}",
                    lua_path.display()
                );
                std::process::exit(1);
            });
            hyperfine
                .arg("--command-name")
                .arg(format!("{} [luajit]", program.name))
                .arg(format!(
                    "luajit -joff {}",
                    quote(&lua_path.to_string_lossy())
                ));
        }
    }

    let output = hyperfine.output().unwrap_or_else(|e| {
        eprintln!("{color_red}KEEL ERROR{color_reset}\nCannot run hyperfine: {e}");
        std::process::exit(1);
    });
    if !output.status.success() {
        eprintln!(
            "{color_red}KEEL ERROR{color_reset}\nhyperfine failed with status {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let json = fs::read_to_string(&json_path).unwrap_or_else(|e| {
        eprintln!(
            "{color_red}KEEL ERROR{color_reset}\nCannot read hyperfine results {}: {e}",
            json_path.display()
        );
        std::process::exit(1);
    });

    // [(name, mean_ms)]
    let mut results: Vec<(String, f64)> = Vec::new();
    for chunk in json.split("\"command\"").skip(1) {
        let name = chunk
            .split_once(':')
            .and_then(|(_, rest)| rest.split('"').nth(1))
            .unwrap_or("unknown")
            .to_string();
        let mean = chunk
            .split_once("\"mean\"")
            .and_then(|(_, rest)| rest.split_once(':'))
            .map(|(_, rest)| {
                rest.trim_start()
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
                    .collect::<String>()
            })
            .and_then(|n| n.parse::<f64>().ok())
            .unwrap_or(0.0)
            * 1000.0;
        results.push((name, mean));
    }

    // Group by program and print with relative speedup ratios
    println!();
    for program in PROGRAMS {
        let keel_ms = results
            .iter()
            .find(|(n, _)| n == &format!("{} [keel]", program.name))
            .map(|(_, v)| *v);

        println!("{color_cyan}{}{color_reset}", program.name);
        if let Some(ms) = keel_ms {
            println!("  {color_blue}keel  {color_reset}: {ms:.3} ms");
        }

        if has_python
            && let Some((_, ms)) = results
                .iter()
                .find(|(n, _)| n == &format!("{} [python]", program.name))
        {
            if let Some(keel_ms) = keel_ms {
                let ratio = ms / keel_ms;
                if ratio >= 1.0 {
                    println!(
                        "  {color_yellow}python {color_reset}: {ms:.3} ms  ({ratio:.2}x slower)"
                    );
                } else {
                    println!(
                        "  {color_yellow}python {color_reset}: {ms:.3} ms  ({:.2}x faster)",
                        1.0 / ratio
                    );
                }
            } else {
                println!("  {color_yellow}python {color_reset}: {ms:.3} ms");
            }
        }

        if has_lua
            && let Some((_, ms)) = results
                .iter()
                .find(|(n, _)| n == &format!("{} [luajit]", program.name))
        {
            if let Some(keel_ms) = keel_ms {
                let ratio = ms / keel_ms;
                if ratio >= 1.0 {
                    println!(
                        "  {color_green}luajit {color_reset}: {ms:.3} ms  ({ratio:.2}x slower)"
                    );
                } else {
                    println!(
                        "  {color_green}luajit {color_reset}: {ms:.3} ms  ({:.2}x faster)",
                        1.0 / ratio
                    );
                }
            } else {
                println!("  {color_green}luajit {color_reset}: {ms:.3} ms");
            }
        }

        println!();
    }
}
