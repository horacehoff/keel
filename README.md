![Keel logo](assets/keel_banner.png#gh-light-mode-only)
![Keel logo](assets/keel_banner_dark.png#gh-dark-mode-only)

> [!WARNING]
> Keel is under active development

Keel is a fast, statically-typed interpreted language that aims to combine Rust-like syntax with Python's ease-of-use.

Its goal is to provide a faster alternative to Python that sits closer to low-level languages while remaining accessible to a wide audience.

[Website](https://keel-lang.com)
[Try Keel in your browser](https://keel-lang.com/playground)

## Why Keel?

- **Fast**: ~2-10x faster than Python ([benchmarks](docs/BENCHMARKS.md)), with aggressive compile-time optimizations
- **Familiar syntax**: Rust-like, with Python's ease-of-use
- **Statically typed, zero annotations**: full type inference, static type checking, polymorphism
- **FFI support**: call C/dynamic libraries directly from Keel
- **Built-in REPL**

[Browse examples](examples/)

## Installation

### macOS / Linux

```sh
curl -fsSL https://raw.githubusercontent.com/horacehoff/keel/main/install.sh | sh
```

### Windows

Download the latest `.zip` from the [releases page](https://github.com/horacehoff/keel/releases/latest) and add the binary to your PATH.

### Build from source

Make sure [Rust](https://rustup.rs/) is installed.

```sh
git clone https://github.com/horacehoff/keel && cd keel && cargo build --release
./target/release/keel myfile.kl
```

## Usage

```sh
keel program.kl    # Run a file
keel               # Start the REPL
keel -v/--version  # Print version
keel -h/--help     # Print help
```

## Near-future roadmap

- Struct methods
- Higher-order functions
- [Better embedding API with limits](#embedding-experimental)

## Language tour

### Table of contents
- [Variables & Types](#variables--types)
- [Functions](#functions)
- [Blocks](#blocks)
- [Conditions](#conditions)
- [Loops](#loops)
  - [While loops](#while-loops)
  - [For loops](#for-loops)
  - [Infinite loops](#infinite-loops)
  - [Integer range loops](#integer-range-loops)
- [Match](#match)
- [Try/Catch blocks](#trycatch-blocks)
- [Importing other ".kl" files](#importing-other-kl-files)
- [Importing dynamic libraries](#importing-dynamic-libraries)
- [Embedding (experimental)](#embedding-experimental)
- [Arrays](#arrays)
- [Slices](#slices)
- [Structs](#structs)
- [Arithmetic Operations](#arithmetic-operations)
- [Documentation](#documentation)

### Variables & Types

Types are inferred and are never written explicitly.

```rs
let x = 42;
let name = "Keel";
let ratio = 3.14;
let flag = true;
let numbers = [1, 2, 3, 4, 5];
```

Built-in types: `Integer` (i32), `Float` (f64), `Boolean`, `String`, `Array<T>`.

### Functions

> A `main()` function is required when executing a `.kl` file.\
> It is the starting point for the execution of the program.

```rs
function add(a, b) {
    return a + b;
}

function main() {
    print(add(10, 32));
}
```

### Blocks

```rs
print("Beginning of program");
let y = 20;
// All blocks are anonymous namespace scopes
// (e.g. trying to access x outside of the following block would yield an error)
{
    let x = 10 + y;
    print(x);
}
```

### Conditions

```rs
let x = 20;
if x == 20 {
  print("20!");
} else if x == 15 {
  print("15!");
} else {
  print("else!");
}
```

Inline conditions work as expressions:

```rs
let my_number = 42;
let the_answer = if my_number == 42 { "It's the answer!" } else { "It's not the answer..." };
print(the_answer);
```

### Loops

Use `break` to exit the loop.\
Use `continue` to skip to the next iteration.

#### While loops

```rs
let i = 0;
while i < 10 {
  print(i);
  i += 1;
}
```

#### For loops

```rs
// Using _ as the variable name in a for loop will discard the value,
// making the program faster, but preventing access to the element
for x in [0,1,2,3] {
  for _ in "abcd" {
    print(x);
  }
}
```

#### Infinite loops

> Loops indefinitely until flow is stopped

```rs
let i = 0;
loop {
    i += 1;
    print("i is: "+str(i));
    if i == 10 {
        break;
    }
}
print("End of the loop!");
```

#### Integer range loops

> Loops over a range of integers

```rs
let x = 0;
// Loops from i=0 to i = max-1
for i in 0..10000000 {
    x += i;
}
print(x);
```

```rs
let x = 0;
// Defaults to 0
for i in ..10 {
    print(i);
    x += 1;
}
print(x);
```

### Match

> Match statements currently don't support binding variables

```rs
let x = "hello";
match x {
  "hello" => {
    print("Hi!");
  }
  "goodbye" => {
    print("Bye!");
  }
  _ => {
    print("You said: "+x);
  }
}
```

### Try/Catch blocks

> This is heavily subject to change, particularly once structs are implemented

The list of catchable errors is available [here](docs/CATCHABLE_ERRORS.md).

Errors can be caught with:

```rs
try {
    // error-prone code here
} catch "index_out_of_bounds" { // matches a specific error
    // code here
} catch "slice_out_of_bounds" {
    // code here
} catch e { // binds the error (a string) to a variable
    // code here
}
```

`catch e` is the catch-all, it handles any error not matched above and binds the error to `e`. If there is one, it must come last.

If no `catch` matches, the error propagates to the enclosing try, or crashes the program if there isn't one.

You can throw errors with `throw("error here")`, which raises a catchable error. In this case, it would be caught by `catch "error here"`.

### Importing other `.kl` files

You can import other `.kl` files with the following syntax:

```keel
use "fibonacci_lib.kl"

function main() {print(fibonacci(25));}
```

All top-level functions from the imported file become available immediately.
Imports can be nested, and circular imports trigger an error and crash the program.

### Importing dynamic libraries

You can load functions from dynamic libraries by specifying each function's
signature, with the following syntax:

```rust
import "dynamic_library_path" {
  function_return_type function_name(function_arg_type_1, function_arg_type_1, ..., function_arg_type_n);
}
```

For example:

```rust
import "my_test.dylib" {
    int add(int, int);
    float add(float, float);
    string add(string, string);
    int sum(int[], int);
}

function fib(n) {
    if n <= 1 {
        return n;
    }
    return fib(my_test::add(n, -1)) + fib(my_test::add(n,-2));
}

function main() {
    print(my_test::add(6, 1));
    print(fib(25));
}
```

If the extension is omitted, Keel will choose the correct extension based on your OS. For example:

```rust
// On macOS, it will try to load "my_test.dylib".
// On Windows, it will try to load "my_test.dll".
// On Linux, it will try to load "my_test.so".
import "my_test" {
    int add(int, int);
    float add(float, float);
    string add(string, string);
    int sum(int[], int);
}
function main() {print(my_test::add(6,1));}
```

### Embedding (experimental)
> The API is subject to change

Keel can be embedded in other programs through a C ABI.
Build it as a dynamic library:
```sh
cargo build --profile embed --features embed
# The library will be in target/embed/libkeel.dylib (OR .so / .dll)
```

Two functions are exposed:
```c
extern char* keel_run(const char* code); // Runs the code and returns the output
extern void keel_free_output(char* output); // Frees the returned string
```

Errors are returned in the output string and don't crash the host.


### Arrays

Arrays are homogeneous and can only hold one type.

```rs
let nums = [3, 1, 4, 1, 5, 9];
nums.sort();
print(nums[0]);         // 1
print(nums.len());      // 6
nums.push(2);
print(nums.contains(9)); // true
```

### Slices

```rs
let nums = [3, 1, 4, 1, 5, 9];
print(nums[..2]);  // [3,1]
print(nums[0..2]); // [3,1]
print(nums[2..4]); // [4,1]
```

```rs
let msg = "Hello world";
print(msg[..5]);   // "Hello"
print(msg[0..5]);  // "Hello"
print(msg[6..11]); // "world"
```

### Structs

```rs
struct TestStruct {
    x: int[][],
    y:bool
}

struct MyStruct {
    first_field:float,
    second_field:TestStruct[],
    third_field:string
}

let x = MyStruct {
    first_field:10.0,
    second_field:[
        TestStruct {
            x: [[0,1,2], [3,4,5]],
            y:false
    }],
    third_field: "Hello, World!"
};
x.third_field = x.third_field.uppercase();
x.second_field[0].x[0][0] += 99;
print(x.third_field); // "HELLO, WORLD!"
print(x.second_field[0]); // TestStruct {x:[[99,1,2],[3,4,5]],y:false}
print(x); // MyStruct {first_field:10,second_field:[TestStruct {x:[[99,1,2],[3,4,5]],y:false}],third_field:"HELLO, WORLD!"}
```


### Arithmetic Operations

```rs
let x = 0;

x = x + 1;
x += 1;

x = x - 1;
x -= 1;

x = x * 1;
x *= 1;

x = x / 1;
x /= 1;

x = x % 1;
x %= 1;

x = x ^ 1;
x ^= 1;

print(x == 1);
print(x != 1);
print(x > 1);
print(x >= 1);
print(x < 1);
print(x <= 1);
print(x > 1 || x < 1);
print(x > 1 && x < 1);
```

## Documentation

- [Standard library](docs/STD_LIB.md)
- [File system library](docs/FS_LIB.md)
- [Catchable errors](docs/CATCHABLE_ERRORS.md)
