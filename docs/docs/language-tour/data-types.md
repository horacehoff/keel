---
icon: lucide/binary
---

# Data types
Every data has a given type. Keel does full type inference, requiring zero annotations, meaning Keel will automatically infer the type of everything in your program (which allows the compiler to perform multiple optimizations).

Keel has 6 built-in types, plus user-defined structs.

You can view all the built-in functions in [Built-in functions](../standard-library/built-in-functions.md).

## Integer (`int`)

An [integer](https://wikipedia.org/wiki/Integer) is a whole number that can be written without a fractional component. Behind the scenes, Keel integers are `i32`, meaning they're signed (they can be negative or positive), and are 32 bits long.
They can store values from `-2147483648` to `2147483647`.

In the case of integer overflow, Keel will **not** warn you and will wrap, meaning the value `-2147483649` will become `2147483647`, and the value `2147483648` will become `-2147483648`.

[Integer operators](../reference/operators.md#integerfloat-operators)

``` rust
fn main() {
    let my_int = 3;
    let another_int = -40000;
}
```

!!! note

    While integers are stored as `i32`, it would technically be possible to stretch them to `i48`. While this isn't planned for now, there's room to grow in the future if it becomes desirable.

## Float (`float`)
A [float](https://wikipedia.org/wiki/Floating-point_arithmetic) is a number with a decimal point. Keel floats are `f64`, meaning they're 64 bits big and can store values from `-1.7976931348623157 * 10^308` to `1.7976931348623157 * 10^308`. If a value become too big, floats saturate to positive or negative infinity.
Floats must always be written with the decimal point included.

[Float operators](../reference/operators.md#integerfloat-operators)

``` rust
fn main() {
    let pi = 3.14;
    let e = 2.718;
    let the_answer = 42.0;
    let negative = -1.0;
}
```

## Boolean (`bool`)
A [boolean](https://wikipedia.org/wiki/Boolean_data_type) is the simplest data type. It has two possible values: `true`, or `false`. Booleans are used in conditions and branches (more on that later).

[Boolean operators](../reference/operators.md#boolean-operators)

``` rust
fn main() {
    let b = true;
    let b2 = false;
    let b3 = (b || b2) && (b && b2);
    let b4 = (1 > 2) || (1 >= 2) || (1 == 2) || (1 <= 2) || (1 < 2);
}
```

## String (`string`)
A [string](https://wikipedia.org/wiki/String_(computer_science)) is a sequence of characters. Strings are immutable collections, so they can be indexed, sliced, and concatenated.

The following escape characters are supported: `\n`, `\t`, `\r`, `\\`, `\"`, and `\0`.

[String operators](../reference/operators.md#stringarray-operators)

``` rust
fn main() {
    let s = "Hello, world!";
    // This concatenates "Hello" and ", world!".
    let s2 = "Hello" + ", world!"; 
    // This indexes `s` and retrieves the first letter: "H".
    let s3 = s[0]; 
    // This slices `s` and retrieves the first five letters: "Hello". 
    let s4 = s[0..5];
    // "Hello
    // World!"
    print("\"Hello\nWorld!\"");
    // Prints the number of letters in `s`.
    print(s.len());
}
```

## Array (`T[]`)
An [array](https://en.wikipedia.org/wiki/Array_(data_structure)) is simply a mutable collection of elements. Keel arrays are homogeneous, meaning they can only hold elements of a single type. An array's type is written `T[]`, with `T` representing the type the array holds, which can be any type (even another array!).

Since arrays are collections, they can be indexed, sliced, and concatenated.

[Array operators](../reference/operators.md#stringarray-operators)

*[indexed]: To index a collection means to retrieve a specific element located at the nth position (called the index) in the collection, starting at position 0.
*[sliced]: To slice a collection means to retrieve the range of elements between two indices (a right-open interval)
*[concatenated]: To concatenate two collections together means to join two collections into one longer collection.

``` rust
fn main() {
    // The type of `a` is int[].
    let a = [1,2] + [3,4];
    // This retrieves [4,5,6,7].
    let a3 = [a, [4,5,6,7]][1];
    // This retrieves the first two elements: [0,1].
    let a4 = a[0..2];

    // This appends `5` at the end of `a`.
    a.push(5);
    // This removes the element at index 0 (`1`).
    a.remove(0);
}
```

## Struct
A [struct](https://wikipedia.org/wiki/Record_(computer_science)) is a data structure that can store multiple values of different data types under a single name, with each value accessed and associated with a field (an identifier).

``` rust
// Structs can be declared inside or outside functions.
// Here, we declare the struct `TestStruct`, and its two fields:
// - x, which is an int matrix (array of arrays)
// - y, which is a boolean
struct TestStruct {
    x: int[][],
    y: bool
}
fn main() {
    struct OtherStruct {
        first_field: float,
        // Since structs are a type, you can make structs containing structs
        second_field: TestStruct[],
        third_field: string
    }
    // This creates an instance of the struct
    let s1 = OtherStruct {
        first_field: 22.0 + 20.0,
        second_field: [
            TestStruct {
                x: [[0],[1],[2]],
                y: true
            }
        ],
        third_field: "Hello, world!"
    };
    // This will access the value associated with `first_field` and print 42.0.
    print(s1.first_field);
    // You can modify a field's value with the same syntax as variables
    s1.third_field = "Goodbye, world!";
}
```

## Map (`[K: V]`)
A [map](https://wikipedia.org/wiki/Associative_array) is a collection of key-value pairs in which keys are unique. They allow for O(1) retrieval and insertion. Keel maps can only store one key-value type, and their type is written `[K: V]`, `K` representing the type of the keys, which can be any type, and `V` representing the type of the values, which can also be any type. Map keys must be literals.

``` rust
fn main() {
    let map = {"one": 1, "two": 2, "other": 42};
    // Retrieve the value associated with the key "one": 1.
    let k = map.get("one");
    // This inserts a new key-value pair in the map.
    map.insert("three", 3);
    // This overwrites the value of the existing key.
    map.insert("one", 0);
}
```

*[Map keys must be literals]: This requirement will be removed soon.