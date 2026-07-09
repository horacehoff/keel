---
icon: lucide/variable
---

# Variables

Variables are containers that allow you to store data. In Keel, all variables are mutable.<br/>
Variables are declared with `let name = value`. To mutate a variable, simply write `name = value`.

``` rust
fn main() {
    let my_var = 42;
    my_var = 314; // This mutates `my_var`.
}
```

You can declare a variable with the same name as an already-existing variable. This will shadow the previous variable, meaning that the name of the variable will reference the second one, until the second variable's scope ends. For example:

*[scope]: The area of the program where an item with an identifier name (a variable, a function, ...) can be recognized.

``` rust
fn main() {
    let v = "hello";
    {
        let v = "goodbye";
        print(v); // This will print "goodbye".
    }
    print(v); // This will print "hello".
}
```