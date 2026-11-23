---
icon: lucide/variable
---

# Variables

Variables are containers that allow you to store data. In Keel, all variables are mutable.<br/>
Variables are declared with `let name = value`. To mutate a variable, simply write `name = value`.
A variable's type is fixed at declaration. The type can also be declared explicitly with `let name: type = value`.

``` rust
fn main() {
    let my_var = 42;
    my_var = 314; // This mutates `my_var`.

    let my_other_var: float = 6.7 + 3.14;
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

## Global variables

Global variables are declared at the toplevel with `static name = value` or `static name: type = value`. They are mutable and accessible from every functon in the file, and from importing files as `module::global_var_name`.

```rust
static first_name = "David";
static last_name: string = "Bowie";

fn main() {
    print("An awesome human artist is..." + first_name + " " + last_name);
    first_name = "The";
    last_name = "Beatles";
    print("But no one can beat " + first_name + " " + last_name);
}
```