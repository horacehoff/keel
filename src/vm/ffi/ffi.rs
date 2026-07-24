use super::Data;
use super::DataType;
use super::NULL;
use super::ObjectPool;
use super::RegisterFile;
use super::Span;
use super::StringPool;
use super::Struct;
use super::UncheckedSliceOps;
use smol_strc::SmolStr;
use std::hint::unreachable_unchecked;

/// Writes `bytes` into `dst` at `index`
pub unsafe fn write_bytes_at_offset(
    dst: &mut [std::mem::MaybeUninit<u8>],
    index: usize,
    bytes: &[u8],
) {
    debug_assert!((index + bytes.len()) <= dst.len());
    unsafe {
        // u8 and MaybeUninit<u8> have the exact same layout, so this pointer cast is safe
        dst.get_unchecked_mut(index..(index + bytes.len()))
            .copy_from_slice(&*(bytes as *const [u8] as *const [std::mem::MaybeUninit<u8>]));
    }
}

/// Puts `buf` in `keep_alive` and returns a pointer to its now stable address
pub fn keep_buffer_alive(
    buf: Box<[std::mem::MaybeUninit<u8>]>,
    keep_alive: &mut Vec<Box<[u8]>>,
) -> usize {
    let ptr = buf.as_ptr() as usize;
    keep_alive.push(unsafe { buf.assume_init() });
    ptr
}

/// Converts a Keel array to a C pointer for libffi
#[cfg(not(target_arch = "wasm32"))]
pub fn array_to_c_ptr(
    data: Data,
    obj_pool: &ObjectPool,
    string_pool: &StringPool,
    // Boxed so the address doesn't move
    keep_alive: &mut Vec<Box<[u8]>>,
) -> usize {
    let elems = &obj_pool[data.as_array()];
    let first_array_element = unsafe { elems.get_unchecked(0) };
    if first_array_element.is_int() {
        // C expects [u8; 4] for ints
        let mut bytes = Box::new_uninit_slice(elems.len() * 4);
        for (i, e) in elems.iter().enumerate() {
            unsafe {
                write_bytes_at_offset(&mut bytes, i * 4, &e.as_int().to_ne_bytes());
            }
        }
        keep_buffer_alive(bytes, keep_alive)
    } else if first_array_element.is_float() {
        // C expects [u8; 8] for doubles
        let mut bytes = Box::new_uninit_slice(elems.len() * 8);
        for (i, e) in elems.iter().enumerate() {
            unsafe {
                write_bytes_at_offset(&mut bytes, i * 8, &e.as_float().to_ne_bytes());
            }
        }
        keep_buffer_alive(bytes, keep_alive)
    } else if first_array_element.is_string() {
        // builds a char** from null-terminated strings
        let mut ptr_bytes = Box::new_uninit_slice(elems.len() * 8);
        for (i, e) in elems.iter().enumerate() {
            let bytes = std::ffi::CString::new(e.as_str(string_pool))
                .expect("interior null byte in string passed to C")
                .into_bytes_with_nul()
                .into_boxed_slice();
            let p = bytes.as_ptr() as usize;
            keep_alive.push(bytes);
            unsafe {
                write_bytes_at_offset(&mut ptr_bytes, i * 8, &p.to_ne_bytes());
            }
        }
        keep_buffer_alive(ptr_bytes, keep_alive)
    } else if first_array_element.is_array() {
        let mut ptr_bytes = Box::new_uninit_slice(elems.len() * 8);
        for (i, e) in elems.iter().enumerate() {
            let ptr = array_to_c_ptr(*e, obj_pool, string_pool, keep_alive);
            unsafe {
                write_bytes_at_offset(&mut ptr_bytes, i * 8, &ptr.to_ne_bytes());
            }
        }
        keep_buffer_alive(ptr_bytes, keep_alive)
    }
    // Any other element type has no C equivalent
    else {
        unsafe { unreachable_unchecked() }
    }
}

/// Computes a Keel struct's size, alignment, and per-field offsets
/// Returns (size, alignment, field_offsets)
#[cfg(not(target_arch = "wasm32"))]
fn get_struct_size(struct_fields: &[Data], obj_pool: &ObjectPool) -> (usize, usize, Vec<usize>) {
    let mut offset: usize = 0;
    let mut max_alignment: usize = 0;
    let mut field_offsets: Vec<usize> = Vec::new();
    field_offsets.reserve_exact(struct_fields.len());
    for field in struct_fields {
        let elem_size: usize;
        let elem_alignment: usize;
        if field.is_int() {
            elem_size = 4;
            elem_alignment = 4;
        } else if field.is_float() || field.is_array() || field.is_string() {
            elem_size = 8;
            elem_alignment = 8;
        } else if field.is_struct() {
            (elem_size, elem_alignment, _) =
                get_struct_size(&obj_pool[field.as_struct()], obj_pool);
        } else {
            unsafe { unreachable_unchecked() }
        }
        let field_offset = offset.next_multiple_of(elem_alignment);
        offset = field_offset + elem_size;
        field_offsets.push(field_offset);
        max_alignment = max_alignment.max(elem_alignment);
    }
    (
        offset.next_multiple_of(max_alignment),
        max_alignment,
        field_offsets,
    )
}

/// (Uses DataType)
/// Computes a Keel struct's size, alignment, and per-field offsets
/// Returns (size, alignment, field_offsets)
#[cfg(not(target_arch = "wasm32"))]
pub fn get_struct_size_datatype(
    struct_fields: &[(SmolStr, DataType, Span)],
    structs: &[Struct],
) -> (usize, usize, Vec<usize>) {
    let mut offset: usize = 0;
    let mut max_alignment: usize = 0;
    let mut field_offsets: Vec<usize> = Vec::new();
    for (_, field, _) in struct_fields {
        let elem_size: usize;
        let elem_alignment: usize;
        match field {
            DataType::Int => {
                elem_size = 4;
                elem_alignment = 4;
            }
            DataType::Float | DataType::Array(_) | DataType::String => {
                elem_size = 8;
                elem_alignment = 8;
            }
            DataType::Struct(struct_id) => {
                (elem_size, elem_alignment, _) = get_struct_size_datatype(
                    unsafe { &structs.get_unchecked(*struct_id as usize).fields },
                    structs,
                );
            }
            _ => unsafe { unreachable_unchecked() },
        }
        let field_offset = offset.next_multiple_of(elem_alignment);
        offset = field_offset + elem_size;
        field_offsets.push(field_offset);
        max_alignment = max_alignment.max(elem_alignment);
    }
    (
        offset.next_multiple_of(max_alignment),
        max_alignment,
        field_offsets,
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn keel_struct_to_c_struct(
    struct_index: usize,
    obj_pool: &ObjectPool,
    string_pool: &StringPool,
    keep_alive: &mut Vec<Box<[u8]>>,
) -> Vec<u8> {
    let struct_fields = &obj_pool[struct_index];
    let (struct_size, _, field_offsets) = get_struct_size(struct_fields, obj_pool);
    let mut buf: Vec<u8> = vec![0u8; struct_size];
    for (i, field) in struct_fields.iter().enumerate() {
        unsafe {
            let offset = *field_offsets.get_unchecked(i);
            if field.is_int() {
                let bytes = field.as_int().to_ne_bytes();
                buf.get_unchecked_mut(offset..offset + 4)
                    .copy_from_slice_unchecked(&bytes);
            } else if field.is_float() {
                let bytes = field.as_float().to_ne_bytes();
                buf.get_unchecked_mut(offset..offset + 8)
                    .copy_from_slice_unchecked(&bytes);
            } else if field.is_string() {
                let bytes = std::ffi::CString::new(field.as_str(string_pool))
                    .expect("interior null byte in string passed to C")
                    .into_bytes_with_nul()
                    .into_boxed_slice();
                let ptr = (bytes.as_ptr() as u64).to_ne_bytes();
                keep_alive.push(bytes);
                buf.get_unchecked_mut(offset..offset + 8)
                    .copy_from_slice_unchecked(&ptr);
            } else if field.is_array() {
                let ptr = array_to_c_ptr(*field, obj_pool, string_pool, keep_alive).to_ne_bytes();
                buf.get_unchecked_mut(offset..offset + 8)
                    .copy_from_slice_unchecked(&ptr);
            } else if field.is_struct() {
                let b =
                    keel_struct_to_c_struct(field.as_struct(), obj_pool, string_pool, keep_alive);
                buf.get_unchecked_mut(offset..offset + b.len())
                    .copy_from_slice_unchecked(&b);
            } else {
                unreachable_unchecked()
            }
        }
    }
    buf
}

#[cfg(not(target_arch = "wasm32"))]
pub fn c_struct_to_keel_struct(
    c_struct: &[u8],
    field_offsets: &[usize],
    obj_pool: &mut ObjectPool,
    string_pool: &mut StringPool,
    struct_fields: &[(SmolStr, DataType, Span)],
    r: &mut RegisterFile,
    recursion_stack: &RegisterFile,
    free_strings: &mut Vec<u16>,
    gc_string_threshold: &mut u32,
    string_live: &mut Vec<bool>,
    structs: &[Struct],
) -> Vec<Data> {
    let mut buf: Vec<Data> = Vec::new();
    buf.reserve_exact(struct_fields.len());
    for (i, (_, field_type, _)) in struct_fields.iter().enumerate() {
        let field_offset = *unsafe { field_offsets.get_unchecked(i) };
        match field_type {
            DataType::Int => {
                let mut bytes: [u8; 4] = [0; 4];
                unsafe {
                    bytes.copy_from_slice_unchecked(&c_struct[field_offset..(field_offset + 4)]);
                }
                buf.push(Data::int(i32::from_ne_bytes(bytes)));
            }
            DataType::Float => {
                let mut bytes: [u8; 8] = [0; 8];
                unsafe {
                    bytes.copy_from_slice_unchecked(&c_struct[field_offset..(field_offset + 8)]);
                }
                buf.push(Data::float(f64::from_ne_bytes(bytes)));
            }
            DataType::String => {
                let mut bytes: [u8; 8] = [0; 8];
                unsafe {
                    bytes.copy_from_slice_unchecked(&c_struct[field_offset..(field_offset + 8)]);
                }
                let ptr = usize::from_ne_bytes(bytes) as *const std::ffi::c_char;
                buf.push(if ptr.is_null() {
                    NULL
                } else {
                    Data::string(
                        unsafe { std::ffi::CStr::from_ptr(ptr) }
                            .to_string_lossy()
                            .into_owned(),
                        obj_pool,
                        string_pool,
                        r,
                        recursion_stack,
                        free_strings,
                        gc_string_threshold,
                        string_live,
                    )
                });
            }
            DataType::Struct(nested_struct_id) => {
                let s = unsafe { structs.get_unchecked(*nested_struct_id as usize) };
                let (_, _, inner_offsets) = get_struct_size_datatype(&s.fields, structs);
                let nested_data_fields = c_struct_to_keel_struct(
                    &c_struct[field_offset..],
                    &inner_offsets,
                    obj_pool,
                    string_pool,
                    &s.fields,
                    r,
                    recursion_stack,
                    free_strings,
                    gc_string_threshold,
                    string_live,
                    structs,
                );
                let new_struct_id = obj_pool.len();
                obj_pool.push(nested_data_fields);
                buf.push(Data::struct_instance(
                    *nested_struct_id,
                    new_struct_id as u32,
                ));
            }
            _ => unsafe { unreachable_unchecked() },
        }
    }
    buf
}
