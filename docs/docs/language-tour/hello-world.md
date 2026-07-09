---
icon: lucide/earth
---

# Hello World
## Code
``` rust title="Hello World"
// This is a comment. Comments don't do anything.
// Comments begin with `//` and continue until the end of the line.
fn main() { // This is another comment!
    print("Hello, world!");
}
```

## Breakdown
The heart of every Keel file is the main function, it's the entry point of your program. Any Keel file used as the program's entry point must define a `main` function.<br/>


`fn` is the keyword used to declare the function. `main` is the name of the function. `()` indicates that our function doesn't take any arguments as input (more on functions and function arguments in [Functions](functions.md).).<br/>We then use curly braces to enclose the function's code. You'll encounter curly braces pretty much everywhere, as they're used to enclose 'blocks'.<br/>
Then, we print "Hello, world!". To do that, we call the `print` function which takes a String as input, and prints it to the terminal. We end the line with a semicolon.