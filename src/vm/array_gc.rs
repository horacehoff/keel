use crate::data::Data;
use crate::vm::ObjectPool;

/// Allocates a new array in the array pool. If reusing an array, it clears it.
pub fn alloc_array(
    obj_pool: &mut ObjectPool,
    free_arrays: &mut Vec<u32>,
    registers: &[Data],
    recursion_stack: &[Data],
    gc_array_threshold: &mut u32,
    live: &mut Vec<bool>,
    array_gc_stack: &mut Vec<Data>,
) -> u32 {
    if let Some(id) = free_arrays.pop() {
        unsafe { obj_pool.get_unchecked_mut(id as usize) }.clear();
        id
    } else {
        if obj_pool.len() >= (*gc_array_threshold as usize) {
            *gc_array_threshold *= 2;
            array_gc(
                obj_pool,
                free_arrays,
                registers,
                recursion_stack,
                live,
                array_gc_stack,
            );
        }
        if let Some(id) = free_arrays.pop() {
            unsafe { obj_pool.get_unchecked_mut(id as usize) }.clear();
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
    free_arrays: &mut Vec<u32>,
    registers: &[Data],
    recursion_stack: &[Data],
    live: &mut Vec<bool>,
    obj_gc_stack: &mut Vec<Data>,
) {
    live.clear();
    live.resize(obj_pool.len(), false);

    // Find all used arrays
    for data in registers.iter().chain(recursion_stack.iter()) {
        if data.is_array() || data.is_struct() {
            track(*data, obj_pool, live, obj_gc_stack);
        }
    }

    // Mark slots that are already free as live
    for &id in free_arrays.iter() {
        live[id as usize] = true;
    }

    // Mark as free any array that isn't referenced by a register
    for (i, array_alive) in live.iter().enumerate() {
        if !array_alive {
            free_arrays.push(i as u32);
        }
    }
}

fn track(root: Data, obj_pool: &ObjectPool, live: &mut [bool], obj_gc_stack: &mut Vec<Data>) {
    obj_gc_stack.push(root);
    while let Some(d) = obj_gc_stack.pop() {
        let is_live = unsafe { live.get_unchecked_mut(d.as_array()) };
        if *is_live {
            continue;
        }
        *is_live = true;
        let arr = unsafe { obj_pool.get_unchecked(d.as_array()) };
        if arr.is_empty() {
            continue;
        }
        #[allow(clippy::blocks_in_conditions)]
        if d.is_struct() {
            for e in arr {
                if e.is_array() || e.is_struct() {
                    obj_gc_stack.push(*e);
                }
            }
        } else if {
            let fst = unsafe { arr.get_unchecked(0) };
            fst.is_array() || fst.is_struct()
        } {
            obj_gc_stack.extend(arr);
        }
    }
}
