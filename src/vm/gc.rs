use fixedbitset::FixedBitSet;

use crate::data::Data;
use crate::vm::{MapPool, ObjectPool, RegisterFile, StringPool};
use std::collections::HashMap;

pub struct Gc {
    pub free_arrays: Vec<u32>,
    pub free_maps: Vec<u32>,
    pub free_strings: Vec<u16>,
    pub array_live: FixedBitSet,
    pub map_live: FixedBitSet,
    pub string_live: FixedBitSet,
    pub stack: Vec<Data>,
    pub array_threshold: u32,
    pub map_threshold: u32,
    pub string_threshold: u32,
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
            stack: Vec::with_capacity(obj_pool.len()),
            array_threshold: 256,
            map_threshold: 256,
            string_threshold: 256,
        }
    }

    fn mark(
        &mut self,
        obj_pool: &ObjectPool,
        map_pool: &MapPool,
        str_pool_len: usize,
        registers: &RegisterFile,
        recursion_stack: &RegisterFile,
    ) {
        self.array_live.clear();
        self.array_live.grow(obj_pool.len());
        self.map_live.clear();
        self.map_live.grow(map_pool.len());
        self.string_live.clear();
        self.string_live.grow(str_pool_len);

        // Find all used strings, arrays, maps, and structs
        for data in registers.0.iter().chain(recursion_stack.0.iter()) {
            mark_data(*data, &mut self.string_live, &mut self.stack);
        }

        while let Some(d) = self.stack.pop() {
            if d.is_map() {
                let was_live = unsafe { self.map_live.put_unchecked(d.as_map()) };
                if was_live {
                    continue;
                }
                for (k, v) in &map_pool[d.as_map()] {
                    mark_data(*k, &mut self.string_live, &mut self.stack);
                    mark_data(*v, &mut self.string_live, &mut self.stack);
                }
            } else {
                let was_live = unsafe { self.array_live.put_unchecked(d.as_array()) };
                if was_live {
                    continue;
                }
                for e in &obj_pool[d.as_array()] {
                    mark_data(*e, &mut self.string_live, &mut self.stack);
                }
            }
        }
    }

    /// Allocates a new array in the array pool. If reusing an array, it clears it.
    pub fn alloc_array(
        &mut self,
        obj_pool: &mut ObjectPool,
        map_pool: &MapPool,
        str_pool: &StringPool,
        registers: &RegisterFile,
        recursion_stack: &RegisterFile,
    ) -> u32 {
        if let Some(id) = self.free_arrays.pop() {
            obj_pool[id as usize].clear();
            return id;
        }
        if obj_pool.len() >= self.array_threshold as usize {
            self.array_threshold *= 2;
            self.mark(obj_pool, map_pool, str_pool.len(), registers, recursion_stack);
            self.free_arrays.clear();
            // Mark as free any array that isn't referenced by a register
            for i in self.array_live.zeroes() {
                self.free_arrays.push(i as u32);
            }
            if let Some(id) = self.free_arrays.pop() {
                obj_pool[id as usize].clear();
                return id;
            }
        }
        let id = obj_pool.len() as u32;
        obj_pool.push(Vec::new());
        id
    }

    pub fn alloc_map(
        &mut self,
        map_pool: &mut MapPool,
        obj_pool: &ObjectPool,
        str_pool: &StringPool,
        registers: &RegisterFile,
        recursion_stack: &RegisterFile,
    ) -> u32 {
        if let Some(id) = self.free_maps.pop() {
            map_pool[id as usize].clear();
            return id;
        }
        if map_pool.len() >= self.map_threshold as usize {
            self.map_threshold *= 2;
            self.mark(obj_pool, map_pool, str_pool.len(), registers, recursion_stack);
            self.free_maps.clear();
            for i in self.map_live.zeroes() {
                self.free_maps.push(i as u32);
            }
            if let Some(id) = self.free_maps.pop() {
                map_pool[id as usize].clear();
                return id;
            }
        }
        let id = map_pool.len() as u32;
        map_pool.push(HashMap::default());
        id
    }

    pub fn collect_free_strings(
        &mut self,
        obj_pool: &ObjectPool,
        map_pool: &MapPool,
        str_pool_len: usize,
        registers: &RegisterFile,
        recursion_stack: &RegisterFile,
    ) {
        self.string_threshold *= 2;
        self.mark(obj_pool, map_pool, str_pool_len, registers, recursion_stack);
        self.free_strings.clear();
        for i in self.string_live.zeroes() {
            self.free_strings.push(i as u16);
        }
    }

    #[inline(always)]
    pub const fn str_pool_needs_gc(&self, str_pool_len: usize) -> bool {
        str_pool_len >= self.string_threshold as usize && self.free_strings.is_empty()
    }
}
