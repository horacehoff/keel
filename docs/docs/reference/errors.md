---
icon: lucide/circle-alert
---
# Errors

Keel aims to have pretty & helpful error messages. Right now, they're not very helpful...
```rust
fn main() {
    print(x)
}

-- OUTPUT -- 
KEEL ERROR
Error:
   ╭─[ test.kl:3:1 ]
   │
 3 │ }
   │ ┬
   │ ╰── Expected SemiColon, but got '}'. Lines must end with a ';'.
───╯
```
... but they're pretty! (thanks [Ariadne](https://crates.io/crates/ariadne)!)

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
- `c_array_return_type_not_supported`
- `invalid_return_type`