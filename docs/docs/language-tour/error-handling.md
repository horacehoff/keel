---
icon: lucide/triangle-alert
---
# Error handling

Keel does error handling through Try/Catch blocks. These allow you to run code potentially expecting one or more errors, and to run code in case an error arises, and are not entirely dissimilar to the if blocks.

You can view the list of errors that can be caught in [Errors](../reference/errors.md#list-of-catchable-errors).

You can throw errors with `#!rust throw("error here")`, which raises a catchable error. In this case, it would be caught by `#!rust catch "error here"`.

!!! note

    Currently, errors are strings. In the future, they may become structs.

## Syntax

```rust
try {
    // error-prone code here
    throw("my_own_error");
} catch "index_out_of_bounds" {
    // this matches a specific error
    // code here
} catch "slice_out_of_bounds" {
    // code here
} catch e {
    // this binds the error (a string) to a variable
    // code here
}
```

## Breakdown

`catch e` is the optional catch-all, it handles any error not matched above and binds the error to `e` (or any other variable name). If there is a catch-all, it must come last.

If no `catch` matches, the error propagates to the enclosing Try/Catch block, or crashes the program if there isn't one.