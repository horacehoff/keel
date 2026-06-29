# Time
Import with `import "std/time.kl";`.

> `arg: T` represents a function argument of type T.

## Now
`time::now() -> Integer`\
Returns the number of seconds since January 1, 1970, 00:00:00 UTC.

## Format
`time::format(date: Integer, format: String) -> String`\
Formats `date` into a string according to the given `format` pattern.
```rust
print(time::format(time::now(), "%x - %X %p")); // prints "06/29/26 - 10:59:21 AM"
```