use crate::data::Data;
use core::ops::{Deref, DerefMut, Index, IndexMut};
use rustc_hash::FxBuildHasher;
use std::collections::HashMap;

pub type ObjectPool = Pool<Vec<Data>>;
pub type MapPool = Pool<HashMap<Data, Data, FxBuildHasher>>;
pub type StringPool = Pool<String>;

#[repr(transparent)]
pub struct Pool<T>(pub Vec<T>);

impl<T> Pool<T> {
    #[inline(always)]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }
}

impl<T> Deref for Pool<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> DerefMut for Pool<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Index<usize> for Pool<T> {
    type Output = T;
    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { self.0.get_unchecked(index) }
    }
}
impl<T> IndexMut<usize> for Pool<T> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut T {
        unsafe { self.0.get_unchecked_mut(index) }
    }
}

impl ObjectPool {
    /// Extends `obj_pool[dst_idx]` with `obj_pool[src_idx][idx_start..idx_end]`.
    #[inline(always)]
    pub fn extend_within(
        &mut self,
        src_idx: usize,
        dst_idx: usize,
        idx_start: usize,
        idx_end: usize,
    ) {
        debug_assert!(src_idx < self.len());
        debug_assert!(dst_idx < self.len());
        debug_assert!(idx_start <= idx_end);
        unsafe {
            let ptr = self.as_mut_ptr();
            let src = ptr.add(src_idx);
            let dst = ptr.add(dst_idx);
            debug_assert!(idx_end <= (&*src).len());
            (*dst).extend_from_slice((&*src).get_unchecked(idx_start..idx_end));
        }
    }
    #[inline(always)]
    pub fn repeat_within(&mut self, src_idx: usize, dst_idx: usize, n: usize) {
        debug_assert!(src_idx < self.len());
        debug_assert!(dst_idx < self.len());
        unsafe {
            let ptr = self.as_mut_ptr();
            let src = &*ptr.add(src_idx);
            let dst = &mut *(ptr.add(dst_idx));
            dst.reserve_exact(src.len() * n);
            for _ in 0..n {
                dst.extend_from_slice(src);
            }
        }
    }
}
