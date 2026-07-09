---
icon: lucide/arrow-down-up
---

# Control flow

## If blocks

If blocks allow you to execute different code based on conditions ([booleans](data-types.md#boolean-bool)), and to check against other conditions (via `else if` and `else`).
Their general syntax is:
```rust
fn main() {
    let x = 1;
    if x == 1 {
        // This code is executed if the condition is true
        print("Yes, they're equal!");
    } else if x == 2 {
        // This code is executed if the previous condition is false,
        // and this one is true
        print("If this runs, we're in trouble!");
    } else {
        // This code is executed if all previous conditions are false
        print("I don't know what to say...");
    }
}
```

## Inline If blocks

Inline If blocks are If blocks used as expressions. They differ from If blocks in that the code inside the braces must be a single expression (thus without a semicolon).

```rust
fn main() {
    let my_number = 42;
    let the_answer = 
        if my_number == 42 { "It's the answer to the question of life!" }
        else if my_number == 20 { "Nope" } 
        else { "It's not the answer..." };
    print(the_answer);
}
```

## Match statements
!!! warning

    Match statements currently don't support binding variables.

Match statements are currently a shorthand for if blocks, allowing you to compare a value against multiple other ones. The value `_` is a catch-all, and will match if nothing else previously matched.

```rust
fn main() {
    let x = "Hello";
    match x {
        "hello" => {
            print("This will be printed!");
        }
        "goodbye" => {
            print("This will never be printed!");
        }
        _ => {
            print("Something else: "+x);
        }
    }
}
```

## Loops

Loops allow you to execute code multiple times, either indefinitely or until a specific condition is reached.

To "break" from a loop (that is, stop its execution right now), use the `break` keyword. To skip to the next iteration of the loop (and thus not execute the rest of the code in the current iteration of the loop), use the `continue` keyword.


### Indefinite loops
Indefinite loops are the simplest form of loop. They run code repeatedly until `break` is called or the program stops.
``` rust
fn main() {
    let i = 0;
    loop {
        i += 1;
        if i == 1 { continue; }
        print("You're going to see this a lot!");
        if i == 10 {
            break;
        }
    }
}
```

### While loops
While loops, like their name implies, execute code in a loop while a specific condition (again, a [boolean](data-types.md#boolean-bool)) is true. At the beginning of each iteration, it checks if the condition is true or false. If it's true, it runs the next iteration. If it's false, it stops the loop and moves on with the program.
```rust
fn main() {
    let i = 0;
    while i < 10 {
        print(i);
        i += 1;
    }
}
```

### For loops
For loops allow you to iterate over every element in a collection. You can currently iterate over [Strings](data-types.md#string-string) and [Arrays](data-types.md#array-t).

For loops bind the current element to a variable with the name of your choosing, which you use inside the loop body.

```rust
fn main() {
    for e in [0,1,2] {
        print(e);
    }
    let hello = "Hello, world!";
    for letter in hello {
        print(letter);
    }
}
```

### Integer range loops
Integer range loops are almost exactly similar to for loops, except that they loop over a list of numbers in a range (a right-open interval). The syntax of the range is `#!rust start..end`, or `#!rust ..end`, to have start = 0. If you name the variable `_`, the variable won't be accessible, which will make your program slightly faster.

```rust
fn main() {
    for i in 5..10 {
        // This will print:
        // 5
        // 6
        // 7
        // 8
        // 9
        print(i);
    }
    for _ in ..20 {
        print("This will be repeated 20 times");
    }
}
```