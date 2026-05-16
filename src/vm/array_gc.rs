use crate::Data;
use crate::vm::ArrayPool;

pub fn alloc_array(
    array_pool: &mut ArrayPool,
    free_arrays: &mut Vec<u32>,
    registers: &[Data],
    recursion_stack: &[Data],
    gc_array_threshold: &mut u32,
    live: &mut Vec<bool>,
    array_gc_stack: &mut Vec<usize>,
) -> u32 {
    if let Some(id) = free_arrays.pop() {
        unsafe { array_pool.get_unchecked_mut(id as usize) }.clear();
        id
    } else {
        if array_pool.len() >= (*gc_array_threshold as usize) {
            *gc_array_threshold *= 2;
            array_gc(
                array_pool,
                free_arrays,
                registers,
                recursion_stack,
                live,
                array_gc_stack,
            );
        }
        if let Some(id) = free_arrays.pop() {
            unsafe { array_pool.get_unchecked_mut(id as usize) }.clear();
            id
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
    array_gc_stack: &mut Vec<usize>,
) {
    live.clear();
    live.resize(array_pool.len(), false);

    // Recursively find all "used" arrays
    for data in registers.iter().chain(recursion_stack.iter()) {
        if data.is_array() {
            track_arrays(data.as_array(), array_pool, live, array_gc_stack);
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
fn track_arrays(
    root_id: usize,
    array_pool: &ArrayPool,
    live: &mut [bool],
    array_gc_stack: &mut Vec<usize>,
) {
    array_gc_stack.push(root_id);
    while let Some(id) = array_gc_stack.pop() {
        let is_live = unsafe { live.get_unchecked_mut(id) };
        if *is_live {
            continue;
        }
        *is_live = true;
        let arr = unsafe { array_pool.get_unchecked(id) };
        if arr.is_empty() || unsafe { !arr.get_unchecked(0).is_array() } {
            continue;
        }
        array_gc_stack.extend(arr.iter().map(|elem| elem.as_array()));
    }
}
