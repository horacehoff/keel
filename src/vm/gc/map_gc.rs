use crate::RegisterFile;
use crate::array_gc::track;
use crate::data::Data;
use crate::vm::MapPool;
use crate::vm::ObjectPool;
use std::collections::HashMap;

pub fn alloc_map(
    map_pool: &mut MapPool,
    obj_pool: &ObjectPool,
    free_maps: &mut Vec<u32>,
    registers: &RegisterFile,
    recursion_stack: &RegisterFile,
    gc_map_threshold: &mut u32,
    map_live: &mut Vec<bool>,
    live: &mut Vec<bool>,
    obj_gc_stack: &mut Vec<Data>,
) -> u32 {
    if let Some(id) = free_maps.pop() {
        map_pool[id as usize].clear();
        id
    } else {
        if map_pool.len() >= (*gc_map_threshold as usize) {
            *gc_map_threshold *= 2;
            map_gc(
                map_pool,
                obj_pool,
                free_maps,
                registers,
                recursion_stack,
                map_live,
                live,
                obj_gc_stack,
            );
        }
        if let Some(id) = free_maps.pop() {
            map_pool[id as usize].clear();
            id
        } else {
            let id = map_pool.len() as u32;
            map_pool.push(HashMap::default());
            id
        }
    }
}

pub fn map_gc(
    map_pool: &MapPool,
    obj_pool: &ObjectPool,
    free_maps: &mut Vec<u32>,
    registers: &RegisterFile,
    recursion_stack: &RegisterFile,
    map_live: &mut Vec<bool>,
    live: &mut Vec<bool>,
    obj_gc_stack: &mut Vec<Data>,
) {
    map_live.clear();
    map_live.resize(map_pool.len(), false);
    for data in registers.0.iter().chain(recursion_stack.0.iter()) {
        if data.is_map() {
            track_maps(
                data.as_map(),
                map_pool,
                obj_pool,
                live,
                map_live,
                obj_gc_stack,
            );
        } else if data.is_array() || data.is_struct() {
            track(*data, obj_pool, map_pool, live, map_live, obj_gc_stack);
        }
    }

    for &id in free_maps.iter() {
        unsafe {
            *map_live.get_unchecked_mut(id as usize) = true;
        }
    }

    for (i, map_alive) in map_live.iter().enumerate() {
        if !map_alive {
            free_maps.push(i as u32);
        }
    }
}

pub fn track_maps(
    idx: usize,
    map_pool: &MapPool,
    obj_pool: &ObjectPool,
    live: &mut Vec<bool>,
    map_live: &mut Vec<bool>,
    obj_gc_stack: &mut Vec<Data>,
) {
    let is_live = unsafe { map_live.get_unchecked_mut(idx) };
    if *is_live {
        return;
    }
    *is_live = true;
    let map = &map_pool[idx];
    let Some((&first_key, &first_val)) = map.iter().next() else {
        return;
    };
    let track_keys = first_key.is_array() || first_key.is_struct() || first_key.is_map();
    let track_vals = first_val.is_array() || first_val.is_struct() || first_val.is_map();
    if !track_keys && !track_vals {
        return;
    }
    for (k, v) in map {
        if track_keys {
            track(*k, obj_pool, map_pool, live, map_live, obj_gc_stack);
        }
        if track_vals {
            track(*v, obj_pool, map_pool, live, map_live, obj_gc_stack);
        }
    }
}
