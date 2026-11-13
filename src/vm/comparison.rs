use crate::vm::Data;
use crate::vm::MapPool;
use crate::vm::ObjectPool;
use crate::vm::StringPool;

#[inline(never)]
#[must_use]
fn container_eq(
    x: Data,
    y: Data,
    obj_pool: &ObjectPool,
    map_pool: &MapPool,
    string_pool: &StringPool,
) -> bool {
    if x.is_array() || x.is_struct() {
        let x_obj = &obj_pool[x.as_array()];
        let y_obj = &obj_pool[y.as_array()];
        x_obj.len() == y_obj.len()
            && x_obj.iter().zip(y_obj).all(|(x, y)| obj_eq(*x, *y, obj_pool, map_pool, string_pool))
    } else if x.is_map() {
        let x_map = &map_pool[x.as_map()];
        let y_map = &map_pool[y.as_map()];
        x_map.len() == y_map.len()
            && x_map.iter().all(|(k, v)| {
                y_map.get(k).is_some_and(|y_v| obj_eq(*v, *y_v, obj_pool, map_pool, string_pool))
            })
    } else {
        false
    }
}

#[inline(always)]
#[must_use]
pub(super) fn obj_eq(
    x: Data,
    y: Data,
    obj_pool: &ObjectPool,
    map_pool: &MapPool,
    string_pool: &StringPool,
) -> bool {
    if x == y {
        true
    } else if x.tag() != y.tag() {
        false
    } else if x.is_string() {
        x.string_eq(y, string_pool)
    } else {
        container_eq(x, y, obj_pool, map_pool, string_pool)
    }
}

#[must_use]
#[inline(never)]
pub(super) fn get_heap_array_position(
    array: &[Data],
    needle: Data,
    obj_pool: &ObjectPool,
    map_pool: &MapPool,
    str_pool: &StringPool,
) -> Option<usize> {
    if needle.is_large_str() {
        let needle = str_pool[needle.get_str_pool_id()].as_bytes();
        array.iter().position(|e| {
            e.is_large_str() && {
                let bytes = str_pool[e.get_str_pool_id()].as_bytes();
                bytes.len() == needle.len() && bytes == needle
            }
        })
    } else {
        debug_assert!(
            needle.is_array() || needle.is_struct() || needle.is_map() || needle.is_function()
        );
        array.iter().position(|e| obj_eq(*e, needle, obj_pool, map_pool, str_pool))
    }
}

#[must_use]
#[inline(always)]
pub(super) fn get_array_position(
    array: &[Data],
    needle: Data,
    obj_pool: &ObjectPool,
    map_pool: &MapPool,
    str_pool: &StringPool,
) -> Option<usize> {
    if needle.is_heap() {
        return get_heap_array_position(array, needle, obj_pool, map_pool, str_pool);
    }
    let Some(tail) = array.last_chunk::<8>() else {
        return array.iter().position(|e| *e == needle);
    };
    let find_first_match = |start_idx: usize, data_window: &[Data; 8]| {
        let mask = data_window
            .iter()
            .enumerate()
            .fold(0u8, |mask, (i, e)| mask | (u8::from(*e == needle) << i));
        (mask != 0).then(|| start_idx + mask.trailing_zeros() as usize)
    };
    array
        .as_chunks::<8>()
        .0
        .iter()
        .enumerate()
        .find_map(|(i, window)| find_first_match(i * 8, window))
        // Checks the remaining array.len() % 8 elements
        .or_else(|| find_first_match(array.len() - 8, tail))
}
