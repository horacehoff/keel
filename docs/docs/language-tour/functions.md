---
icon: lucide/square-function
---

# Functions
Keel functions use the following syntax:
``` rust
fn name(arg1, arg2, ..., argN) {
    // put the function's code here
}
```

They're called with `#!rust name(arg1, arg2, ..., argN)`. A function can also not take any arguments as input.

Functions can be defined at the toplevel or inside other functions.

The main function is special. It doesn't take any arguments, and any Keel file used as the program's entry point must define a `main` function. 

Keel supports function [polymorphism](https://wikipedia.org/wiki/Polymorphism_(computer_science)) (and does [compile-time monomorphization](https://wikipedia.org/wiki/Monomorphization)), meaning that a single function can operate on arguments of different types. For example:
``` rust
fn add(x, y) {
    return x+y;
}

fn main() {
    let result = add(1, 1);
    let result2 = add(1.5, 1.5);
    let result3 = add("Hello", ", world!");
    let result4 = add([0,1], [2,3]);
}
```