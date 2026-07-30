---
icon: lucide/circle-alert
---
# Errors

Keel aims to have pretty (thanks [Ariadne](https://crates.io/crates/ariadne)!) and helpful error messages. Currently, some error messages are more helpful than others. This is a work in progress, and error messages will continue to improve over time.
```rust
struct Point {
    x:float,
    y:float,
    z:float
}

fn main() {
    let origin = Point {x: 0.0, y: 0.0, z: 0.0};
    origin.x = 10;
}
```
```
-- OUTPUT -- 
Error: Incompatible types
   ╭─[ testing/error.kl:2:7 ]
   │
 2 │     x:float,
   │       ───┬──
   │          ╰──── Field x in struct Point expects type float
   │
 9 │     origin.x = 10;
   │                ─┬
   │                 ╰── This expression is of type int
   │
   │ Help: Try using the float() function
───╯
```

## List of catchable errors

### Misc

- `division_by_zero`
- `modulo_by_zero`
- `index_out_of_bounds`
- `slice_out_of_bounds`
- `unknown_map_key`

### Runtime parsing

- `invalid_float`
- `invalid_int`
- `invalid_bool`

### File system

- `fs_already_exists`
- `fs_deadlock`
- `fs_file_too_large`
- `fs_interrupted`
- `fs_invalid_data`
- `fs_invalid_filename`
- `fs_is_a_directory`
- `fs_not_a_directory`
- `fs_not_found`
- `fs_permission_denied`
- `fs_out_of_memory`
- `fs_read_only_filesystem`
- `fs_storage_full`
- `fs_timed_out`

### FFI

- `null_byte_in_string`
- `invalid_return_type`