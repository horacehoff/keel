---
icon: lucide/package
---
# Modules

## Importing other Keel files

You can import other Keel files with the `import` keyword at the top-level.

Imports can be nested, and circular imports are detected and produce an error.

*[at the top-level]: Outside of any function.

```rust
import "fibonacci_lib.kl"; // all functions/structs are available under fibonacci_lib::
import "other_lib.kl" as mylib; // all functions/structs are available under mylib::

fn main() {print(mylib::my_func(42));}
```

## Importing libraries

!!! note

    This feature will release with Keel 0.3.0, coming soon.


Keel libraries are ordinary Keel files.
The `libs/` folder located next to the Keel executable (currently in `/Library/Keel/` on macOS and `/usr/local/lib/keel/` on Linux) is checked by the `import` keyword if the file isn't found locally, making global Keel libraries possible. For example, the `math`, `time`, `random` libraries are located in `libs/std/`. As such, by placing `.kl` files in the `libs/` folder, you can make libraries available globally.
```rs
import "std/math.kl";
import "std/time.kl";
import "std/random.kl";

fn main() {
    print(math::cos(3.14159265359));
    print(random::random_range(10.0,20.0));
    print(time::format(time::now(), "%x - %X %p"));
}
```