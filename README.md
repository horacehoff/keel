![Keel logo](assets/keel_banner.png#gh-light-mode-only)
![Keel logo](assets/keel_banner_dark.png#gh-dark-mode-only)

> [!WARNING]
> Keel is under active development

**Keel** is a fast, statically-typed interpreted language that aims to combine Rust-like syntax with Python's ease-of-use.

Its goal is to provide a faster alternative to Python that sits closer to low-level languages while remaining accessible to a wide audience.

**Contributions and issues are welcome!**

[Website](https://keel-lang.com)
[Documentation](https://docs.keel-lang.com)
[Try Keel in your browser](https://keel-lang.com/playground)

## Why Keel?

- **Fast**: ~2-15x faster than Python ([benchmarks](BENCHMARKS.md)), with aggressive compile-time optimizations
- **Familiar syntax**: Rust-like, with Python's ease-of-use
- **Statically typed, zero annotations**: full type inference, static type checking, polymorphism
- **FFI support**: call C/dynamic libraries directly from Keel
- **Built-in REPL**

[Browse examples](examples/)

## Quick showcase

```rust
struct Point { x: int, y: int }

fn add(a, b) {
    return a + b;
}

fn main() {
    let p = Point { x: 3, y: 4 };
    print(add(p.x, p.y)); // 7
    print(add("Hello, ", "world!")); // Hello, world!

    let nums = [4, 2, 6, 1, 7];
    if nums[0] == 4 {
        nums.sort();
        print(if nums[0] == 1 { nums[0..3] } else { -1 }); // [1,2,4]
    } else {
        throw("Error!");
    }
}
```

## Benchmarks
![Keel benchmarks](docs/docs/images/keel-benchmarks.png)

## Installation

### macOS / Linux

```sh
curl -fsSL https://raw.githubusercontent.com/horacehoff/keel/main/install.sh | sh
```

### Windows

Download the latest `.zip` from the [releases page](https://github.com/horacehoff/keel/releases/latest) and add the binary to your PATH.

### Build from source (without PGO)

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
- Better, more helpful errors
- Better module system
- Struct methods
- Proper higher-order functions implementation
- Better embedding API with limits