---
icon: lucide/package
---
# Modules

You can import other Keel files with the `import` keyword at the top-level.

Imports can be nested and circular.

*[at the top-level]: Outside of any function.

```rust
import "fibonacci_lib.kl"; // all functions/structs/globals are available under fibonacci_lib::
import "other_lib.kl" as mylib; // all functions/structs/globals are available under mylib::

fn main() {print(mylib::my_func(42));}
```

## Names and paths
With `.kl`, the path must be that exact file. Without it, the path must be either `<name>.kl` or `<name>/lib.kl`. Both existing yields an error.

The paths searched are, in order:

1. The directory of the importing file
2. `libs/` next to the file that was executed
3. `~/.keel/libs/` (`%USERPROFILE%\.keel\libs\` on Windows)
4. `libs/` next to the resolved Keel executable

Without an `as`, the namespace is the last component of the path, without `.kl`, and with `-` and `.` replaced with `_`.

## The standard library
`std` is installed with Keel in `~/.keel/libs/std/`. It currently contains math, random, and time (for example: `import "std/math"`).