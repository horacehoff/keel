use crate::data::{Data, PoolString};
use crate::vm::{MapPool, ObjectPool, StringPool, UncheckedVecOps};
use fixedbitset::FixedBitSet;
use std::collections::HashMap;

const GC_THRESHOLD: u32 = 256;

pub struct Gc {
    free_arrays: Vec<u32>,
    free_maps: Vec<u32>,
    free_strings: Vec<u32>,
    array_live: FixedBitSet,
    map_live: FixedBitSet,
    string_live: FixedBitSet,
    to_mark: Vec<Data>,
    pinned: Vec<Data>,
    array_threshold: u32,
    map_threshold: u32,
    string_threshold: u32,
}

#[inline(always)]
fn mark_data(d: Data, string_live: &mut FixedBitSet, stack: &mut Vec<Data>) {
    if d.is_heap() {
        if d.is_large_str() {
            unsafe {
                string_live.insert_unchecked(d.get_str_pool_id());
            }
        } else if !d.is_function() {
            // ARRAY, STRUCT, MAP
            stack.push(d);
        }
    }
}

impl Gc {
    pub fn new(obj_pool: &ObjectPool, map_pool: &MapPool, str_pool: &StringPool) -> Self {
        Self {
            free_arrays: Vec::with_capacity(obj_pool.len()),
            free_maps: Vec::with_capacity(map_pool.len()),
            free_strings: Vec::with_capacity(str_pool.len()),
            array_live: FixedBitSet::new(),
            map_live: FixedBitSet::new(),
            string_live: FixedBitSet::new(),
            to_mark: Vec::with_capacity(obj_pool.len()),
            pinned: Vec::with_capacity(2),
            array_threshold: GC_THRESHOLD,
            map_threshold: GC_THRESHOLD,
            string_threshold: GC_THRESHOLD,
        }
    }

    #[inline(always)]
    pub fn pin(&mut self, value: Data) {
        self.pinned.push(value);
    }
    #[inline(always)]
    pub fn unpin(&mut self) {
        self.pinned.pop_unchecked();
    }

    fn mark(
        &mut self,
        obj_pool: &ObjectPool,
        map_pool: &MapPool,
        str_pool_len: usize,
        roots: &[Data],
    ) {
        self.array_live.clear();
        self.array_live.grow(obj_pool.len());
        self.map_live.clear();
        self.map_live.grow(map_pool.len());
        self.string_live.clear();
        self.string_live.grow(str_pool_len);

        // Find all used strings, arrays, maps, and structs
        for data in roots.iter().chain(&self.pinned) {
            mark_data(*data, &mut self.string_live, &mut self.to_mark);
        }

        while let Some(d) = self.to_mark.pop() {
            if d.is_map() {
                let was_live = unsafe { self.map_live.put_unchecked(d.as_map()) };
                if was_live {
                    continue;
                }
                for (k, v) in &map_pool[d.as_map()] {
                    mark_data(*k, &mut self.string_live, &mut self.to_mark);
                    mark_data(*v, &mut self.string_live, &mut self.to_mark);
                }
            } else {
                let was_live = unsafe { self.array_live.put_unchecked(d.as_array()) };
                if was_live {
                    continue;
                }
                for e in &obj_pool[d.as_array()] {
                    mark_data(*e, &mut self.string_live, &mut self.to_mark);
                }
            }
        }
    }

    #[cold]
    #[inline(never)]
    fn collect(
        &mut self,
        obj_pool: &ObjectPool,
        map_pool: &MapPool,
        str_pool: &StringPool,
        roots: &[Data],
    ) {
        self.mark(obj_pool, map_pool, str_pool.len(), roots);

        self.free_arrays.clear();
        self.free_arrays.extend(self.array_live.zeroes().map(|id| id as u32));
        self.free_maps.clear();
        self.free_maps.extend(self.map_live.zeroes().map(|id| id as u32));
        self.free_strings.clear();
        self.free_strings.extend(self.string_live.zeroes().map(|id| id as u32));

        self.array_threshold =
            ((obj_pool.len() - self.free_arrays.len()) as u32 * 2).max(GC_THRESHOLD);
        self.map_threshold = ((map_pool.len() - self.free_maps.len()) as u32 * 2).max(GC_THRESHOLD);
        self.string_threshold =
            ((str_pool.len() - self.free_strings.len()) as u32 * 2).max(GC_THRESHOLD);
    }

    /// Allocates a new array in the array pool. If reusing an array, it clears it.
    pub fn alloc_array(
        &mut self,
        obj_pool: &mut ObjectPool,
        map_pool: &MapPool,
        str_pool: &StringPool,
        roots: &[Data],
    ) -> u32 {
        if self.free_arrays.is_empty() && obj_pool.len() >= self.array_threshold as usize {
            self.collect(obj_pool, map_pool, str_pool, roots);
        }
        if let Some(id) = self.free_arrays.pop() {
            obj_pool[id as usize].clear();
            return id;
        }
        let id = obj_pool.len() as u32;
        obj_pool.push(Vec::new());
        id
    }

    pub fn alloc_map(
        &mut self,
        obj_pool: &ObjectPool,
        map_pool: &mut MapPool,
        str_pool: &StringPool,
        roots: &[Data],
    ) -> u32 {
        if self.free_maps.is_empty() && map_pool.len() >= self.map_threshold as usize {
            self.collect(obj_pool, map_pool, str_pool, roots);
        }
        if let Some(id) = self.free_maps.pop() {
            map_pool[id as usize].clear();
            return id;
        }
        let id = map_pool.len() as u32;
        map_pool.push(HashMap::default());
        id
    }

    #[inline]
    pub fn alloc_string<S: PoolString>(
        &mut self,
        s: S,
        obj_pool: &ObjectPool,
        map_pool: &MapPool,
        str_pool: &mut StringPool,
        roots: &[Data],
    ) -> u32 {
        if self.free_strings.is_empty() && str_pool.len() >= self.string_threshold as usize {
            self.collect(obj_pool, map_pool, str_pool, roots);
        }
        if let Some(id) = self.free_strings.pop() {
            s.move_to_slot(&mut str_pool[id as usize]);
            return id;
        }
        let id = str_pool.len() as u32;
        s.push_to_pool(str_pool);
        id
    }
    #[inline]
    pub fn alloc_string_slot(
        &mut self,
        obj_pool: &ObjectPool,
        map_pool: &MapPool,
        str_pool: &mut StringPool,
        roots: &[Data],
    ) -> u32 {
        if let Some(id) = self.free_strings.pop() {
            return id;
        }
        if str_pool.len() >= self.string_threshold as usize {
            self.collect(obj_pool, map_pool, str_pool, roots);
            if let Some(id) = self.free_strings.pop() {
                return id;
            }
        }
        let id = str_pool.len() as u32;
        str_pool.push(String::new());
        id
    }
}
