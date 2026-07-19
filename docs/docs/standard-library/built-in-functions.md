---
icon: lucide/parentheses
---
# Built-in functions

> `T` represents any type.<br/>
> `<T>` represents any expression of type `T`.<br/>
> `arg: T` represents a function argument of type `T`.<br/>
> `-> T` means that the function returns a value of type `T`.

## Print

`print(T)`<br/>
Used to print anything.

```
print("Hello, World!");
print([42]);
```

## Type

`type(T) -> string`<br/>
Returns the type of the object as a string.

```
type("Hello, World!"); // Returns "string"
type([42]); // Returns "int[]"
```

## Float

`float(string | int) -> float`<br/>
Returns the string or int interpreted as a float. It will crash the program at runtime if the given string cannot be converted into a float.

```
float(42); // Returns 42.0
float("42"); // Returns 42.0
float("Hello, World!"); // Crashes
```

## Int

`int(string | float) -> int`<br/>
Returns the string or float interpreted as an int. It will crash the program at runtime if the given string cannot be converted into an int.

```
int(42.0); // Returns 42
int("42"); // Returns 42
int("Hello, World!"); // Crashes
```

## Str

`str(T) -> string`<br/>
Returns the given object as a string.

```
str(42); // Returns "42"
str([0,1,2,3]); // Returns "[0,1,2,3]"
```

## Bool

`bool(s: string) -> bool`<br/>
Returns `s` interpreted as a boolean. It will crash the program at runtime if `s` cannot be converted into a boolean.

```
bool("true"); // Returns true
bool("42"); // Crashes
```

## Input

`input() -> string`<br/>
`input(p: string) -> string`<br/>
Asks the user for input.
If provided, it will print `p` prompt before asking.

## Range

`range(j: int) -> int[]`<br/>
`range(i: int, j: int) -> int[]`<br/>
Returns an array containing the numbers from 0 or `i` to `j`-1.

```
range(5); // Returns [0,1,2,3,4]
range(1,5); // Returns [1,2,3,4]
```

## TheAnswer

`the_answer() -> int`<br/>
Prints "The answer to the Ultimate Question of Life, the Universe, and Everything is 42." and returns the int 42.

## Uppercase

`<string>.uppercase() -> string`<br/>
Returns the given string as uppercase.

```
"Hello, World!".uppercase() // Returns "HELLO, WORLD!"
```

## Lowercase

`<string>.lowercase() -> string`<br/>
Returns the given string as lowercase.

```
"Hello, World!".lowercase() // Returns "hello, world!"
```

## Len

`<string | T[]>.len() -> int`<br/>
Returns the length of the given collection.

```
"Hello".len() // Returns 5
[1,2,3].len() // Returns 3
```

## Contains

`<string>.contains(e: string) -> bool`<br/>
`<T[]>.contains(e: T) -> bool`<br/>
Returns a bool depicting whether or not the collection contains `e`.

```
"Hello".contains("H") // Returns true
[1,2,3].contains(0) // Returns false
```

## Trim

`<string>.trim() -> string`<br/>
Returns the given string, trimmed (leading and trailing whitespace removed).

```
" Hello ".trim() // Returns "Hello"
```

## TrimLeft

`<string>.trim_left() -> string`<br/>
Returns the given string, with the left trimmed (leading whitespace removed).

```
" Hello ".trim_left() // Returns "Hello "
```

## TrimRight

`<string>.trim_right() -> string`<br/>
Returns the given string, with the right trimmed (trailing whitespace removed).

```
" Hello ".trim_right() // Returns " Hello"
```

## TrimSequence

`<string>.trim_sequence(s: string) -> string`<br/>
Returns the given string, with `s` removed from the start and end of the string.

```
"-Hi!-".trim_sequence("-") // Returns "Hi!"
```

## TrimSequenceLeft

`<string>.trim_sequence_left(s: string) -> string`<br/>
Returns the given string, with `s` removed from the start of the string.

```
"-Hi!-".trim_sequence_left("-") // Returns "Hi!-"
```

## TrimSequenceRight

`<string>.trim_sequence_right(s: string) -> string`<br/>
Returns the given string, with `s` removed from the end of the string.

```
"-Hi!-".trim_sequence_right("-") // Returns "-Hi!"
```

## Find

`<string>.find(e: string) -> int`<br/>
`<T[]>.find(e: T) -> int`<br/>
Returns the index of `e` in the collection. If the element isn't found, it will return `-1`.

```
[1,2,3,4].find(2) // Returns 1
"Hello".find("l") // Returns 2
"Hello".find("el") // Returns 1
[1,2,3,4].find(5) // Returns -1
```

## Repeat

`<string>.repeat(n: int) -> string`<br/>
`<T[]>.repeat(n: int) -> T[]`<br/>
Returns a collection repeated n times.

```
"AB".repeat(2) // Returns "ABAB"
[0,1,2].repeat(2) // Returns [0,1,2,0,1,2]
```

## Push

`<T[]>.push(e: T)`<br/>
Adds `e` to the end of an array.

```
let my_array = [1,2];
my_array.push(3);
print(my_array); // Prints "[1,2,3]"
```

## Remove

`<T[]>.remove(n: int)`<br/>
Removes the n-th element from an array.

```
let my_array = [1,2];
my_array.remove(1);
print(my_array); // Prints "[1]"
```

## Sqrt

`<float>.sqrt() -> float`<br/>
Returns the square root of a float.

```
36.0.sqrt() // Returns 6.0
42.0.sqrt() // Returns 6.48074069840786
```

## Round

`<float>.round() -> float`<br/>
Rounds a float to the nearest int

```
36.4.round() // Returns 36.0
6.7.round() // Returns 7.0
```

## Floor

`<float>.floor() -> float`<br/>
Floors a float.

```
36.4.floor() // Returns 36.0
6.7.floor() // Returns 6.0
6.9.floor() // Returns 6.0
```

## Abs

`<float>.abs() -> float`<br/>
`<int>.abs() -> int`<br/>
Returns the absolute value of a number.

```
6.abs() // Returns 6
-6.abs() // Returns -6
(-6).abs() // Returns 6
(-42.0).abs() // Returns 42.0
```

## IsFloat

`<string>.is_float() -> bool`<br/>
Returns whether or not a string represents a float.

```
"6".is_float() // Returns false
"Hello, World!".is_float() // Returns false
"42.0".is_float() // Returns true
"6.7".is_float() // Returns true
```

## IsInt

`<string>.is_int() -> bool`<br/>
Returns whether or not a string represents an int.

```
"6".is_int() // Returns true
"Hello, World!".is_int() // Returns false
"42.0".is_int() // Returns false
"6.7".is_int() // Returns false
```

## Reverse

`<T[]>.reverse()`<br/>
`<string>.reverse() -> string`<br/>
Reverses a collection.

```
let x = [1,2,3];
x.reverse();
print(x); // Prints "[3,2,1]"

print("Hello".reverse()); // Prints "olleH"
```

## Split

`<string>.split(separator: string) -> string[]`<br/>
Splits a string with the given separator `separator`.

```
"a;b;c".split(";") // Returns ["a", "b", "c"]
```

## Partition

`<T[]>.partition(separator: T) -> T[][]`<br/>
Partitions a collection with the given separator `separator`.

```
[1,2,3,0,4,5,6].partition(0) // Returns [[1,2,3],[4,5,6]]
```

## StartsWith

`<string>.starts_with(s: string) -> bool`<br/>
Returns whether or not the given string starts with `s`.

```
"Hello".starts_with("He") // Returns true
"Hello".starts_with("l") // Returns false
```

## EndsWith

`<string>.ends_with(s: string) -> bool`<br/>
Returns whether or not the given string ends with `s`.

```
"Hello".ends_with("lo") // Returns true
"Hello".ends_with("H") // Returns false
```

## Replace

`<string>.replace(a: string, b: string) -> string`<br/>
Returns the given string with all occurrences of `a` replaced with `b`.

```
"1;2;3".replace(";", "_") // Returns "1_2_3"
"BBBB".replace("BB", "AB") // Returns "ABAB"
```

## Join

`<string[]>.join() -> string`<br/>
`<string[]>.join(separator: string) -> string`<br/>
Joins all elements of the array into a single string, with `separator` or `""` inserted between each element.

```
["a","b","c"].join() // Returns "abc"
["a","b","c"].join(",") // Returns "a,b,c"
["1","2"].join("--") // Returns "1--2"
```

## Sort

`<T[]>.sort()`<br/>
Sorts an array in place and returns it. Supports arrays of ints, floats, and strings.

```
let arr = [3, 1, 2];
arr.sort();
print(arr); // Prints "[1,2,3]"
```

## Argv

`argv() -> string[]`<br/>
Returns the arguments passed to the script, excluding the interpreter path and script name.

```
// ./keel script.kl foo bar
argv() // Returns ["foo", "bar"]
```
## Exit

`exit()`<br/>
`exit(exit_code: int)`<br/>
Exits the program with the exit code 0, if not provided with one.

## Throw

`throw(error: string)`<br/>
Throws an error. Read more in [Error handling](../language-tour/error-handling.md)

```rust
fn main() {
    try {
        let idk = [][0];
    } catch "index_out_of_bounds" {
        print("This WILL be printed!");
    }
}
```

## Get

`<{K: V}>.get(key: T) -> V`<br/>
Returns the value associated with the key in the given map. If the key doesn't exist, it raises the `unknown_map_key` error.

```
let map = {"test1": 42, "test2": 67};
print(map.get("test2")); // prints 67
```

## Insert

`<{K: V}>.insert(key: T, value: V)`<br/>
Inserts the given key-value pair in the map. It updates the value if the key already exists.

```
let map = {"test1": 42, "test2": 67};
map.insert("test3", 314);
map.insert("test1", 3000);
print(map); // prints {"test1":3000,"test2":67,"test3":314}
```