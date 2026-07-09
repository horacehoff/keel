---
icon: lucide/library-big
---
# Dynamic libraries
You can load functions from dynamic libraries by specifying each function's signature, with the following syntax at the top-level:

*[at the top-level]: Outside of any function.

```rust
dylib "dynamic_library_path" {
  function_return_type function_name(function_arg_type_1, function_arg_type_2, ..., function_arg_type_n);
}
```

For example:
```rust
dylib "my_test.dylib" {
    int add(int, int);
    float add(float, float);
    string add(string, string);
    int sum(int[], int);
}

fn fib(n) {
    if n <= 1 {
        return n;
    }
    return fib(my_test::add(n, -1)) + fib(my_test::add(n,-2));
}

fn main() {
    print(my_test::add(6, 1));
    print(fib(25));
}
```

If the extension is omitted, Keel will choose the correct extension based on your OS, and it will also try to load an architecture-specific version if one exists. This makes cross-platform & cross-architecture dynamic library loading possible from a single Keel file. For example:

```rust
// On macOS, it will try to load "my_test-aarch64.dylib"
// (or "my_test-x86_64.dylib", depending on your CPU), then fall back to "my_test.dylib".
// Same process on Windows, but with the ".dll" extension.
// Same process on Linux, but with the ".so" extension.
dylib "my_test" {
    int add(int, int);
    float add(float, float);
    string add(string, string);
    int sum(int[], int);
}
fn main() {print(my_test::add(6,1));}
```