pub(super) trait UncheckedVecOps<T> {
    fn pop_unchecked(&mut self) -> T;
    fn push_unchecked(&mut self, value: T);
}
pub(super) trait UncheckedSliceOps<T: Copy> {
    unsafe fn copy_from_slice_unchecked(&mut self, src: &[T]);
}

impl<T> UncheckedVecOps<T> for Vec<T> {
    #[inline(always)]
    fn pop_unchecked(&mut self) -> T {
        debug_assert!(!self.is_empty());
        let new_len = self.len() - 1;
        unsafe {
            self.set_len(new_len);
            self.as_mut_ptr().add(new_len).read()
        }
    }
    #[inline]
    fn push_unchecked(&mut self, value: T) {
        debug_assert!(self.len() < self.capacity());
        unsafe {
            self.as_mut_ptr().add(self.len()).write(value);
            self.set_len(self.len() + 1);
        }
    }
}

impl<T: Copy> UncheckedSliceOps<T> for [T] {
    #[inline(always)]
    unsafe fn copy_from_slice_unchecked(&mut self, src: &[T]) {
        debug_assert_eq!(self.len(), src.len());
        unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), self.as_mut_ptr(), self.len()) };
    }
}
