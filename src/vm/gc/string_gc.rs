use crate::data::Data;
use crate::vm::ObjectPool;
use crate::vm::RegisterFile;
use crate::vm::StringPool;

pub fn string_gc(
    array_pool: &ObjectPool,
    string_pool: &StringPool,
    free_strings: &mut Vec<u16>,
    registers: &RegisterFile,
    recursion_stack: &RegisterFile,
    live: &mut Vec<bool>,
) {
    live.clear();
    live.resize(string_pool.len(), false);
    for data in registers.0.iter().chain(recursion_stack.0.iter()) {
        if data.is_large_str() {
            live[data.get_str_pool_id()] = true;
        } else if data.is_array() {
            track_strings(array_pool, &array_pool[data.as_array()], live);
        }
    }

    for &id in free_strings.iter() {
        live[id as usize] = true;
    }

    for (i, s) in live.iter().enumerate() {
        if !s {
            free_strings.push(i as u16);
        }
    }
}

fn track_strings(array_pool: &ObjectPool, array: &Vec<Data>, live: &mut [bool]) {
    if let Some(first) = array.first() {
        if first.is_string() {
            for x in array {
                if x.is_large_str() {
                    live[x.get_str_pool_id()] = true;
                }
            }
        } else if first.is_array() {
            for x in array {
                track_strings(array_pool, &array_pool[x.as_array()], live);
            }
        }
    }
}

#[inline(always)]
pub fn raise_string_gc_threshold(gc_string_threshold: &mut u32, string_pool_len: usize) {
    *gc_string_threshold = string_pool_len.next_power_of_two().min(u32::MAX as usize) as u32;
}
