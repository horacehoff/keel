use crate::data::Data;
use crate::vm::Frame;

const MAX_STACK_LEN: usize = 16_000_000 / 8;

pub(super) struct Stack {
    #[cfg(not(target_arch = "wasm32"))]
    _buf: virtual_buffer::vec::Vec<Data>,
    #[cfg(target_arch = "wasm32")]
    _buf: Vec<Data>,
    pub ptr: *mut Data,
    pub frame_limit: Frame,
    max_frame_regs: usize,
}

impl Stack {
    pub(super) fn new(initial_stack: &[Data], max_frame_regs: usize) -> Self {
        let len = initial_stack.len() + MAX_STACK_LEN;

        let mut buf = {
            #[cfg(target_arch = "wasm32")]
            {
                unsafe { Box::<[Data]>::new_zeroed_slice(len).assume_init() }.into_vec()
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let mut buf = virtual_buffer::vec::Vec::new(len);
                buf.reserve_exact(len);
                buf
            }
        };

        let ptr: *mut Data = buf.as_mut_ptr();
        unsafe { core::ptr::copy_nonoverlapping(initial_stack.as_ptr(), ptr, initial_stack.len()) };
        let frame_limit = Frame(unsafe { ptr.add(buf.capacity() - max_frame_regs) });
        Self { _buf: buf, ptr, frame_limit, max_frame_regs }
    }

    #[inline(always)]
    pub(super) const fn roots(&self, frame: Frame) -> &[Data] {
        unsafe {
            core::slice::from_raw_parts(
                self.ptr,
                frame.0.offset_from(self.ptr) as usize + self.max_frame_regs,
            )
        }
    }
}
