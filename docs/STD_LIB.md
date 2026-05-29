# Standard library

> T represents any type.\
> Array\<T\> represents an array containing elements of type T\
> \<T\> represents any expression of type T

## Print

`print(T)`\
Used to print anything.
Example:
```
print("Hello, World!");
print([42]);
```

## Type

`type(T) -> String`\
Returns the type of the object as a string.
Example:
```
type("Hello, World!"); // Returns "String"
type([42]); // Returns "Array<Integer>"
```

## Float

`float(String | Int) -> Float`\
Returns the string or integer interpreted as a float. It will crash the program at runtime if the given string cannot be converted into a float.
Example:
```
float(42); // Returns 42.0
float("42"); // Returns 42.0
float("Hello, World!"); // Crashes
```

## Int

`int(String | Float) -> Int`\
Returns the string or float interpreted as an integer. It will crash the program at runtime if the given string cannot be converted into an integer.
Example:
```
int(42.0); // Returns 42
int("42"); // Returns 42
int("Hello, World!"); // Crashes
```

## Str

`str(T) -> String`\
Returns the given object as a string.
Example:
```
str(42); // Returns "42"
str([0,1,2,3]); // Returns "[0,1,2,3]"
```


## Bool

`bool(s: String) -> Bool`\
Returns `s` interpreted as a boolean. It will crash the program at runtime if `s` cannot be converted into a boolean.
Example:
```
bool("true"); // Returns true
bool("42"); // Crashes
```

## Input

`input() -> String`\
`input(p: String) -> String`\
Asks the user for input.
If provided, it will print `p` prompt before asking.

## Range

`range(j: Int) -> Array<Int>`\
`range(i: Int, j: Int) -> Array<Int>`\
Returns an array containing the numbers from 0 or `i` to `j`-1.
Example:
```
range(5); // Returns [0,1,2,3,4]
range(1,5); // Returns [1,2,3,4]
```

## TheAnswer

`the_answer() -> Int`\
Prints "The answer to the Ultimate Question of Life, the Universe, and Everything is 42." and returns the integer 42.

## Uppercase

`<String>.uppercase() -> String`\
Returns the given string as uppercase.
Example:
```
"Hello, World!".uppercase() // Returns "HELLO, WORLD!"
```

## Lowercase

`<String>.lowercase() -> String`\
Returns the given string as lowercase.
Example:
```
"Hello, World!".lowercase() // Returns "hello, world!"
```

## Len

`<String | Array<T>>.len() -> Int`\
Returns the length of the given collection.
Example:
```
"Hello".len() // Returns 5
[1,2,3].len() // Returns 3
```

## Contains

`<String>.contains(e: String) -> Bool`\
`<Array<T>>.contains(e: T) -> Bool`\
Returns a bool depicting whether or not the collection contains `e`.
Example:
```
"Hello".contains("H") // Returns true
[1,2,3].contains(0) // Returns false
```

## Trim

`<String>.trim() -> String`\
Returns the given string, trimmed (leading and trailing whitespace removed).
Example:
```
" Hello ".trim() // Returns "Hello"
```

## TrimLeft

`<String>.trim_left() -> String`\
Returns the given string, with the left trimmed (leading whitespace removed).
Example:
```
" Hello ".trim_left() // Returns "Hello "
```

## TrimRight

`<String>.trim_right() -> String`\
Returns the given string, with the right trimmed (trailing whitespace removed).
Example:
```
" Hello ".trim_right() // Returns " Hello"
```

## TrimSequence

`<String>.trim_sequence(s: String) -> String`\
Returns the given string, with `s` removed from the start and end of the string.
Example:
```
"-Hi!-".trim_sequence("-") // Returns "Hi!"
```

## TrimSequenceLeft

`<String>.trim_sequence_left(s: String) -> String`\
Returns the given string, with `s` removed from the start of the string.
Example:
```
"-Hi!-".trim_sequence_left("-") // Returns "Hi!-"
```

## TrimSequenceRight

`<String>.trim_sequence_right(s: String) -> String`\
Returns the given string, with `s` removed from the end of the string.
Example:
```
"-Hi!-".trim_sequence_right("-") // Returns "-Hi!"
```

## Find

`<String>.find(e: String) -> Int`\
`<Array<T>>.find(e: T) -> Int`\
Returns the index of `e` in the collection. If the element isn't found, it will return `-1`.
Example:
```
[1,2,3,4].find(2) // Returns 1
"Hello".find("l") // Returns 2
"Hello".find("el") // Returns 1
[1,2,3,4].find(5) // Returns -1
```

## Repeat

`<String>.repeat(n: Int) -> String`\
`<Array<T>>.repeat(n: Int) -> Array<T>`\
Returns a collection repeated n times.
Example:
```
"AB".repeat(2) // Returns "ABAB"
[0,1,2].repeat(2) // Returns [0,1,2,0,1,2]
```

## Push

`<Array<T>>.push(e: T)`\
Adds `e` to the end of an array.
Example:
```
let my_array = [1,2];
my_array.push(3);
print(my_array); // Prints "[1,2,3]"
```

## Remove

`<Array<T>>.remove(n: Int)`\
Removes the n-th element from an array.
Example:
```
let my_array = [1,2];
my_array.remove(1);
print(my_array); // Prints "[1]"
```

## Sqrt

`<Float>.sqrt() -> Float`\
Returns the square root of a float.
Example:
```
36.0.sqrt() // Returns 6.0
42.0.sqrt() // Returns 6.48074069840786
```

## Round

`<Float>.round() -> Float`\
Rounds a float to the nearest integer
Example:
```
36.4.round() // Returns 36.0
6.7.round() // Returns 7.0
```

## Floor

`<Float>.floor() -> Float`\
Floors a float.
Example:
```
36.4.floor() // Returns 36.0
6.7.floor() // Returns 6.0
6.9.floor() // Returns 6.0
```

## Abs

`<Float>.abs() -> Float`\
`<Int>.abs() -> Int`\
Returns the absolute value of a number.
Example:
```
6.abs() // Returns 6
-6.abs() // Returns -6
(-6).abs() // Returns 6
(-42.0).abs() // Returns 42.0
```

## IsFloat

`<String>.is_float() -> Bool`\
Returns whether or not a string represents a float.
Example:
```
"6".is_float() // Returns false
"Hello, World!".is_float() // Returns false
"42.0".is_float() // Returns true
"6.7".is_float() // Returns true
```

## IsInt

`<String>.is_int() -> Bool`\
Returns whether or not a string represents an integer.
Example:
```
"6".is_int() // Returns true
"Hello, World!".is_int() // Returns false
"42.0".is_int() // Returns false
"6.7".is_int() // Returns false
```

## Reverse

`<Array<T>>.reverse()`\
`<String>.reverse() -> String`\
Reverses a collection.
Example:
```
let x = [1,2,3];
x.reverse();
print(x); // Prints "[3,2,1]"

print("Hello".reverse()); // Prints "olleH"
```

## Split

`<String>.split(separator: String) -> Array<String>`\
Splits a string with the given separator `separator`.
Example:
```
"a;b;c".split(";") // Returns ["a", "b", "c"]
```

## Partition

`<Array<T>>.partition(separator: T) -> Array<Array<T>>`\
Partitions a collection with the given separator `separator`.
Example:
```
[1,2,3,0,4,5,6].partition(0) // Returns [[1,2,3],[4,5,6]]
```

## StartsWith

`<String>.starts_with(s: String) -> Bool`\
Returns whether or not the given string starts with `s`.
Example:
```
"Hello".starts_with("He") // Returns true
"Hello".starts_with("l") // Returns false
```

## EndsWith

`<String>.ends_with(s: String) -> Bool`\
Returns whether or not the given string ends with `s`.
Example:
```
"Hello".ends_with("lo") // Returns true
"Hello".ends_with("H") // Returns false
```

## Replace

`<String>.replace(a: String, b: String) -> String`\
Returns the given string with all occurrences of `a` replaced with `b`.
Example:
```
"1;2;3".replace(";", "_") // Returns "1_2_3"
"BBBB".replace("BB", "AB") // Returns "ABAB"
```

## Join

`<Array<String>>.join() -> String`\
`<Array<String>>.join(separator: String) -> String`\
Joins all elements of the array into a single string, with `separator` or `""` inserted between each element.
Example:
```
["a","b","c"].join() // Returns "abc"
["a","b","c"].join(",") // Returns "a,b,c"
["1","2"].join("--") // Returns "1--2"
```

## Sort

`<Array<T>>.sort()`\
Sorts an array in place and returns it. Supports arrays of integers, floats, and strings.
Example:
```
let arr = [3, 1, 2];
arr.sort();
print(arr); // Prints "[1,2,3]"
```

## Argv

`argv() -> Array<String>`\
Returns the arguments passed to the script, excluding the interpreter path and script name.
Example:
```
// ./keel script.kl foo bar
argv() // Returns ["foo", "bar"]
```
## Exit

`exit()`\
`exit(exit_code: Int)`\
Exits the program with the exit code 0, if not provided with one.