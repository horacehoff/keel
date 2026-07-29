# Keel

Syntax highlighting for [Keel](https://github.com/horacehoff/keel), a fast, statically-typed interpreted language with Rust-like syntax and Python's ease-of-use, ~10x faster than Python and competitive with LuaJIT (-joff).

This extension highlights `.kl` files.

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