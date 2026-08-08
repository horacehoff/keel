use crate::data::Data;
use crate::vm::{MapPool, ObjectPool, RegisterFile, StringPool};
use std::collections::HashMap;

pub struct Gc {
    pub free_arrays: Vec<u32>,
    pub free_maps: Vec<u32>,
    pub free_strings: Vec<u16>,
    pub array_live: Vec<bool>,
    pub map_live: Vec<bool>,
    pub string_live: Vec<bool>,
    pub stack: Vec<Data>,
    pub array_threshold: u32,
    pub map_threshold: u32,
    pub string_threshold: u32,
}

#[inline(always)]
fn mark_data(d: Data, string_live: &mut [bool], stack: &mut Vec<Data>) {
    if d.is_large_str() {
        unsafe {
            *string_live.get_unchecked_mut(d.get_str_pool_id()) = true;
        }
    } else if d.is_array() || d.is_struct() || d.is_map() {
        stack.push(d);
    }
}

impl Gc {
    pub fn new(obj_pool: &ObjectPool, map_pool: &MapPool, str_pool: &StringPool) -> Self {
        Self {
            free_arrays: Vec::with_capacity(obj_pool.len()),
            free_maps: Vec::with_capacity(map_pool.len()),
            free_strings: Vec::with_capacity(str_pool.len()),
            array_live: Vec::new(),
            map_live: Vec::new(),
            string_live: Vec::new(),
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
        self.array_live.resize(obj_pool.len(), false);
        self.map_live.clear();
        self.map_live.resize(map_pool.len(), false);
        self.string_live.clear();
        self.string_live.resize(str_pool_len, false);

        // Find all used strings, arrays, maps, and structs
        for data in registers.0.iter().chain(recursion_stack.0.iter()) {
            mark_data(*data, &mut self.string_live, &mut self.stack);
        }

        while let Some(d) = self.stack.pop() {
            if d.is_map() {
                let is_live = unsafe { self.map_live.get_unchecked_mut(d.as_map()) };
                if *is_live {
                    continue;
                }
                *is_live = true;
                for (k, v) in &map_pool[d.as_map()] {
                    mark_data(*k, &mut self.string_live, &mut self.stack);
                    mark_data(*v, &mut self.string_live, &mut self.stack);
                }
            } else {
                let is_live = unsafe { self.array_live.get_unchecked_mut(d.as_array()) };
                if *is_live {
                    continue;
                }
                *is_live = true;
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
            for (i, is_array_alive) in self.array_live.iter().enumerate() {
                if !is_array_alive {
                    self.free_arrays.push(i as u32);
                }
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
            for (i, is_map_alive) in self.map_live.iter().enumerate() {
                if !is_map_alive {
                    self.free_maps.push(i as u32);
                }
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
        self.string_threshold = str_pool_len.next_power_of_two().min(u32::MAX as usize) as u32;
        self.mark(obj_pool, map_pool, str_pool_len, registers, recursion_stack);
        self.free_strings.clear();
        for (i, is_str_alive) in self.string_live.iter().enumerate() {
            if !is_str_alive {
                self.free_strings.push(i as u16);
            }
        }
    }

    #[inline(always)]
    pub const fn str_pool_needs_gc(&self, str_pool_len: usize) -> bool {
        str_pool_len >= self.string_threshold as usize && self.free_strings.is_empty()
    }
}
