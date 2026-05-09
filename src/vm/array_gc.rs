use crate::Data;
use crate::vm::ArrayPool;

pub fn alloc_array(
    array_pool: &mut ArrayPool,
    free_arrays: &mut Vec<u32>,
    registers: &[Data],
    recursion_stack: &[Data],
    gc_array_threshold: &mut u32,
    live: &mut Vec<bool>,
) -> u32 {
    if let Some(id) = free_arrays.pop() {
        array_pool[id as usize].clear();
        id as u32
    } else {
        if free_arrays.is_empty() && array_pool.len() >= (*gc_array_threshold as usize) {
            *gc_array_threshold *= 2;
            array_gc(array_pool, free_arrays, registers, recursion_stack, live);
        }
        if let Some(id) = free_arrays.pop() {
            array_pool[id as usize].clear();
            id as u32
        } else {
            let id = array_pool.len() as u32;
            array_pool.push(Vec::new());
            id
        }
    }
}

fn array_gc(
    array_pool: &ArrayPool,
    free_arrays: &mut Vec<u32>,
    registers: &[Data],
    recursion_stack: &[Data],
    live: &mut Vec<bool>,
) {
    live.clear();
    live.resize(array_pool.len(), false);

    // Recursively find all "used" arrays
    for data in registers.iter().chain(recursion_stack.iter()) {
        if data.is_array() {
            track_arrays(data.as_array(), array_pool, live);
        }
    }

    // Prevent duplicates: mark already-free slots as live
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

/// Tracks nested arrays
fn track_arrays(id: usize, array_pool: &ArrayPool, live: &mut [bool]) {
    if live[id] {
        return;
    }
    live[id] = true;
    // Arrays can only hold a single type
    // As such, if the first element in the array is not an array, then the other elements aren't either
    if array_pool[id].is_empty() || !array_pool[id][0].is_array() {
        return;
    }
    for elem in &array_pool[id] {
        track_arrays(elem.as_array(), array_pool, live);
    }
}
