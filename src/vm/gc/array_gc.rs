use crate::data::Data;
use crate::map_gc::track_maps;
use crate::vm::{MapPool, ObjectPool, RegisterFile};

/// Allocates a new array in the array pool. If reusing an array, it clears it.
pub fn alloc_array(
    obj_pool: &mut ObjectPool,
    map_pool: &MapPool,
    free_arrays: &mut Vec<u32>,
    registers: &RegisterFile,
    recursion_stack: &RegisterFile,
    gc_array_threshold: &mut u32,
    live: &mut Vec<bool>,
    map_live: &mut Vec<bool>,
    obj_gc_stack: &mut Vec<Data>,
) -> u32 {
    if let Some(id) = free_arrays.pop() {
        obj_pool[id as usize].clear();
        id
    } else {
        if obj_pool.len() >= (*gc_array_threshold as usize) {
            *gc_array_threshold *= 2;
            array_gc(
                obj_pool,
                map_pool,
                free_arrays,
                registers,
                recursion_stack,
                live,
                map_live,
                obj_gc_stack,
            );
        }
        if let Some(id) = free_arrays.pop() {
            obj_pool[id as usize].clear();
            id
        } else {
            let id = obj_pool.len() as u32;
            obj_pool.push(Vec::new());
            id
        }
    }
}

fn array_gc(
    obj_pool: &ObjectPool,
    map_pool: &MapPool,
    free_arrays: &mut Vec<u32>,
    registers: &RegisterFile,
    recursion_stack: &RegisterFile,
    live: &mut Vec<bool>,
    map_live: &mut Vec<bool>,
    obj_gc_stack: &mut Vec<Data>,
) {
    live.clear();
    live.resize(obj_pool.len(), false);

    // Find all used arrays
    for data in registers.0.iter().chain(recursion_stack.0.iter()) {
        if data.is_array() || data.is_struct() {
            track(*data, obj_pool, map_pool, live, map_live, obj_gc_stack);
        }
    }

    // Mark slots that are already free as live
    for &id in free_arrays.iter() {
        unsafe {
            *live.get_unchecked_mut(id as usize) = true;
        }
    }

    // Mark as free any array that isn't referenced by a register
    for (i, array_alive) in live.iter().enumerate() {
        if !array_alive {
            free_arrays.push(i as u32);
        }
    }
}

#[allow(clippy::ptr_arg)]
pub fn track(
    root: Data,
    obj_pool: &ObjectPool,
    map_pool: &MapPool,
    live: &mut Vec<bool>,
    map_live: &mut Vec<bool>,
    obj_gc_stack: &mut Vec<Data>,
) {
    obj_gc_stack.push(root);
    while let Some(d) = obj_gc_stack.pop() {
        if d.is_map() {
            track_maps(d.as_map(), map_pool, obj_pool, live, map_live, obj_gc_stack);
            continue;
        }
        let is_live = unsafe { live.get_unchecked_mut(d.as_array()) };
        if *is_live {
            continue;
        }
        *is_live = true;
        let arr = &obj_pool[d.as_array()];
        if arr.is_empty() {
            continue;
        }
        #[allow(clippy::blocks_in_conditions)]
        if d.is_struct() {
            for e in arr {
                if e.is_array() || e.is_struct() || e.is_map() {
                    obj_gc_stack.push(*e);
                }
            }
        } else if {
            let fst = unsafe { arr.get_unchecked(0) };
            fst.is_array() || fst.is_struct() || fst.is_map()
        } {
            obj_gc_stack.extend(arr);
        }
    }
}
