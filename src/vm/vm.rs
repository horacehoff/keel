use crate::array_gc::alloc_array;
use crate::compiler::compiler_data::DynamicLibFn;
use crate::compiler::compiler_data::ErrorCatch;
use crate::compiler::compiler_data::Pools;
use crate::compiler::type_system::DataType;
use crate::data::Data;
use crate::data::DataHash;
use crate::data::FALSE;
use crate::data::NULL;
use crate::data::TRUE;
use crate::errors::ErrType;
use crate::errors::ErrorCtx;
use crate::errors::throw_error;
use crate::fs;
use crate::instr::Instr;
use crate::instr::LibFunc;
use crate::instr::LibFuncVoid;
use crate::map_gc::alloc_map;
use crate::string_gc::raise_string_gc_threshold;
use lexical_core::FormattedSize;
use memchr::memmem;
use smol_strc::SmolStr;
use smol_strc::ToSmolStr;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::hint::cold_path;
use std::io::Write;
use std::ops::Index;
use std::ops::IndexMut;

#[cfg(target_arch = "wasm32")]
use crate::errors::wasm_error;

pub type ObjectPool = Pool<Vec<Data>>;
pub type MapPool = Pool<HashMap<Data, Data, BuildHasherDefault<DataHash>>>;
pub type StringPool = Pool<String>;

/// Converts a Keel array to a C pointer for libffi
#[cfg(not(target_arch = "wasm32"))]
fn array_to_c_ptr(
    data: Data,
    elem_type: &DataType,
    obj_pool: &ObjectPool,
    string_pool: &StringPool,
    // boxed so the address doesn't move
    keep_alive: &mut Vec<Box<[u8]>>,
) -> u64 {
    let elems = &obj_pool[data.as_array()];

    match elem_type {
        DataType::Int => {
            // C expects [u8; 4] for ints
            let bytes: Box<[u8]> = elems
                .iter()
                .flat_map(|e| e.as_int().to_ne_bytes())
                .collect();
            let ptr = bytes.as_ptr() as u64;
            keep_alive.push(bytes);
            ptr
        }

        // C expects [u8; 8] for doubles
        DataType::Float => {
            let bytes: Box<[u8]> = elems
                .iter()
                .flat_map(|e| e.as_float().to_ne_bytes())
                .collect();
            let ptr = bytes.as_ptr() as u64;
            keep_alive.push(bytes);
            ptr
        }
        // builds a char** from null-terminated strings
        DataType::String => {
            let mut ptrs: Vec<usize> = Vec::with_capacity(elems.len());
            for e in elems {
                let bytes = std::ffi::CString::new(e.as_str(string_pool))
                    .expect("interior null byte in string passed to C")
                    .into_bytes_with_nul()
                    .into_boxed_slice();
                ptrs.push(bytes.as_ptr() as usize);
                keep_alive.push(bytes);
            }
            let ptr_bytes: Box<[u8]> = ptrs.iter().flat_map(|p| p.to_ne_bytes()).collect();
            let ptr = ptr_bytes.as_ptr() as u64;
            keep_alive.push(ptr_bytes);
            ptr
        }
        DataType::Array(Some(inner)) => {
            let mut ptrs: Vec<usize> = Vec::with_capacity(elems.len());
            for e in elems {
                ptrs.push(array_to_c_ptr(*e, inner, obj_pool, string_pool, keep_alive) as usize);
            }
            let ptr_bytes: Box<[u8]> = ptrs.iter().flat_map(|p| p.to_ne_bytes()).collect();
            let ptr = ptr_bytes.as_ptr() as u64;
            keep_alive.push(ptr_bytes);
            ptr
        }
        // Any other element type has no C equivalent
        t => unreachable!("Unsupported array element type for C FFI: {t:?}"),
    }
}

fn obj_eq(
    x: Data,
    y: Data,
    obj_pool: &ObjectPool,
    map_pool: &MapPool,
    string_pool: &StringPool,
) -> bool {
    if x == y {
        return true;
    }
    if x.tag() != y.tag() {
        return false;
    }
    if x.is_str() && y.is_str() {
        return x.as_str(string_pool) == y.as_str(string_pool);
    }
    if (x.is_array() || x.is_struct()) && (y.is_array() || y.is_struct()) {
        let x_obj = &obj_pool[x.as_array()];
        let y_obj = &obj_pool[y.as_array()];
        if x_obj.len() != y_obj.len() {
            return false;
        }
        if x_obj == y_obj {
            return true;
        }
        for (x, y) in x_obj.iter().zip(y_obj) {
            if !obj_eq(*x, *y, obj_pool, map_pool, string_pool) {
                return false;
            }
        }
        return true;
    }
    if x.is_map() && y.is_map() {
        let x_map = &map_pool[x.as_map()];
        let y_map = &map_pool[y.as_map()];
        if x_map.len() != y_map.len() {
            return false;
        }
        if x_map == y_map {
            return true;
        }
        for (k, v) in x_map {
            if !y_map
                .get(k)
                .is_some_and(|y_v| obj_eq(*v, *y_v, obj_pool, map_pool, string_pool))
            {
                return false;
            }
        }
        return true;
    }
    false
}

struct CallFrame {
    return_addr: u16,
    return_reg: u16,
    callsite_id: u16,
}

pub trait UncheckedVecOps<T> {
    fn pop_unchecked(&mut self) -> T;
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
}

#[repr(transparent)]
pub struct Pool<T>(pub Vec<T>);

impl<T> Pool<T> {
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.0.len()
    }
    #[inline(always)]
    pub fn push(&mut self, value: T) {
        self.0.push(value);
    }
    #[inline(always)]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }
    #[inline(always)]
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }
    #[inline(always)]
    pub fn get_mut(&mut self, index: usize) -> &mut T {
        unsafe { self.0.get_unchecked_mut(index) }
    }
    #[inline(always)]
    pub const fn as_mut_ptr(&mut self) -> *mut T {
        self.0.as_mut_ptr()
    }
    #[inline(always)]
    pub fn split_at_mut(&mut self, mid: usize) -> (&mut [T], &mut [T]) {
        unsafe { self.0.split_at_mut_unchecked(mid) }
    }
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
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

#[repr(transparent)]
pub struct RegisterFile(pub Vec<Data>);

impl RegisterFile {
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.0.len()
    }
}

impl Index<u16> for RegisterFile {
    type Output = Data;
    #[inline(always)]
    fn index(&self, index: u16) -> &Self::Output {
        unsafe { self.0.get_unchecked(index as usize) }
    }
}
impl IndexMut<u16> for RegisterFile {
    #[inline(always)]
    fn index_mut(&mut self, index: u16) -> &mut Data {
        unsafe { self.0.get_unchecked_mut(index as usize) }
    }
}
impl Index<usize> for RegisterFile {
    type Output = Data;
    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { self.0.get_unchecked(index) }
    }
}
impl IndexMut<usize> for RegisterFile {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut Data {
        unsafe { self.0.get_unchecked_mut(index) }
    }
}

#[allow(unused_unsafe)]
pub fn execute(
    instructions: &[Instr],
    r: &mut RegisterFile,
    Pools {
        objs: obj_pool,
        maps: map_pool,
        strings: string_pool,
    }: &mut Pools,
    err_ctx: &ErrorCtx,
    fn_registers: &[Vec<u16>],
    dyn_libs: &[DynamicLibFn],
    struct_fields: &[(SmolStr, Vec<SmolStr>)],
    allocated_arg_count: usize,
    allocated_call_depth: usize,
) {
    let mut i: usize = 0;

    let mut args: Vec<u16> = Vec::with_capacity(allocated_arg_count);
    let mut call_frames: Vec<CallFrame> = Vec::with_capacity(allocated_call_depth);
    let mut recursion_stack = RegisterFile(Vec::with_capacity(allocated_call_depth * r.len()));

    #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
    let mut handle = std::io::stdout().lock();
    #[cfg(any(target_arch = "wasm32", feature = "embed"))]
    let mut handle = crate::captured_output::CapturedOutputWriter;

    let mut free_arrays: Vec<u32> = Vec::with_capacity(obj_pool.len());
    let mut free_maps: Vec<u32> = Vec::with_capacity(map_pool.len());
    let mut free_strings: Vec<u16> = Vec::with_capacity(string_pool.len());
    let mut array_live: Vec<bool> = Vec::new();
    let mut map_live: Vec<bool> = Vec::new();
    let mut string_live: Vec<bool> = Vec::new();

    let mut dyn_lib_args: Vec<u64> = Vec::new();
    let mut keep_alive: Vec<Box<[u8]>> = Vec::new();
    let mut obj_gc_stack: Vec<Data> = Vec::with_capacity(obj_pool.len());

    let mut gc_string_threshold: u32 = 256;
    let mut gc_array_threshold: u32 = 256;
    let mut gc_map_threshold: u32 = 256;

    let mut error_handles: Vec<ErrorCatch> = Vec::new();

    macro_rules! str {
        ($e: expr) => {
            Data::str(
                $e,
                obj_pool,
                string_pool,
                r,
                &recursion_stack,
                &mut free_strings,
                &mut gc_string_threshold,
                &mut string_live,
            )
        };
    }
    macro_rules! string {
        ($e: expr) => {
            Data::string(
                $e,
                obj_pool,
                string_pool,
                r,
                &recursion_stack,
                &mut free_strings,
                &mut gc_string_threshold,
                &mut string_live,
            )
        };
    }

    macro_rules! error_with_catch {
        ($err:expr) => {
            cold_path();
            if !error_handles.is_empty() {
                let err_handle = unsafe { error_handles.pop_unchecked() };
                unsafe {
                    args.set_len(err_handle.args_len as usize);
                    call_frames.set_len(err_handle.call_frames_len as usize);
                }
                r[err_handle.error_reg] = str!($err.kind());
                i = err_handle.catch_loc as usize;
                continue;
            }
            throw_error(err_ctx, unsafe {*instructions.get_unchecked(i)}, $err);
        };
        ($err:expr, $label:lifetime) => {
            cold_path();
            if !error_handles.is_empty() {
                let err_handle = unsafe { error_handles.pop_unchecked() };
                unsafe {
                    args.set_len(err_handle.args_len as usize);
                    call_frames.set_len(err_handle.call_frames_len as usize);
                }
                r[err_handle.error_reg] = str!($err.kind());
                i = err_handle.catch_loc as usize;
                continue $label;
            }
            throw_error(err_ctx, unsafe {*instructions.get_unchecked(i)}, $err);
        };
    }

    'main: loop {
        match unsafe { *instructions.get_unchecked(i) } {
            Instr::Jmp(size) => {
                i += size as usize;
                continue;
            }
            Instr::JmpBack(size) => {
                i -= size as usize;
                continue;
            }
            Instr::Mov(tgt, dest) => r[dest] = r[tgt],
            Instr::SetInt(dest, n) => r[dest] = n.into(),
            Instr::SetBool(b, dest) => r[dest] = b.into(),
            Instr::CallFunc(new_loc, return_id) => {
                call_frames.push(CallFrame {
                    return_addr: i as u16,
                    return_reg: return_id,
                    callsite_id: 0,
                });
                i = new_loc as usize;
                continue;
            }
            Instr::CallFuncRecursive(new_loc, _) => {
                i = new_loc as usize;
                continue;
            }
            Instr::VoidReturn => {
                // Simply jump back to the callsite, since there's nothing to return
                i = call_frames.pop_unchecked().return_addr as usize;
            }
            Instr::SaveFrame(relative_func_loc, return_register, callsite_id) => {
                call_frames.push(CallFrame {
                    return_addr: (i as u16) + relative_func_loc,
                    return_reg: return_register,
                    callsite_id,
                });
                recursion_stack.0.extend(
                    unsafe { fn_registers.get_unchecked(callsite_id as usize) }
                        .iter()
                        .map(|&reg| r[reg]),
                );
            }
            Instr::Return(tgt) => {
                // Pop the latest call frame, set the return value and jump back to the callsite
                let call_frame = call_frames.pop_unchecked();
                i = call_frame.return_addr as usize;
                r[call_frame.return_reg] = r[tgt];
            }
            Instr::RecursiveReturn(tgt) => {
                let call_frame = call_frames.pop_unchecked();
                let temp = r[tgt];
                let regs = unsafe { fn_registers.get_unchecked(call_frame.callsite_id as usize) };
                let base = recursion_stack.len() - regs.len();
                for (reg, &saved) in regs
                    .iter()
                    .zip(unsafe { recursion_stack.0.get_unchecked(base..) })
                {
                    r[*reg] = saved;
                }
                unsafe {
                    recursion_stack.0.set_len(base);
                }
                i = call_frame.return_addr as usize;
                r[call_frame.return_reg] = temp;
            }
            Instr::IsFalseJmp(cond_id, size) => {
                if r[cond_id] == FALSE {
                    i += size as usize;
                    continue;
                }
            }
            Instr::IsTrueJmp(cond_id, size) => {
                if r[cond_id] == TRUE {
                    i += size as usize;
                    continue;
                }
            }
            #[cfg(target_arch = "wasm32")]
            Instr::CallDynamicLibFunc(_, _) => unsafe { std::hint::unreachable_unchecked() },
            #[cfg(not(target_arch = "wasm32"))]
            Instr::CallDynamicLibFunc(fn_id, dest) => {
                let func = unsafe { dyn_libs.get_unchecked(fn_id as usize) };
                let args_len = args.len();
                dyn_lib_args.clear();
                keep_alive.clear();

                for idx in 0..args.len() {
                    let data = r[unsafe { *args.get_unchecked(idx) }];
                    dyn_lib_args.push({
                        match func.get_nth_arg_type(idx) {
                            DataType::Int => data.as_int() as u64,
                            DataType::Float => data.as_float().to_bits(),
                            DataType::String => {
                                let bytes = if let Ok(b) =
                                    std::ffi::CString::new(data.as_str(string_pool))
                                {
                                    b.into_bytes_with_nul().into_boxed_slice()
                                } else {
                                    error_with_catch!(ErrType::NullByteInString, 'main);
                                };
                                let ptr = bytes.as_ptr() as u64;
                                keep_alive.push(bytes);
                                ptr
                            }
                            DataType::Array(Some(inner)) => {
                                array_to_c_ptr(data, inner, obj_pool, string_pool, &mut keep_alive)
                            }
                            _ => unreachable!(),
                        }
                    });
                }
                args.clear();

                // Args converted from Data to libffi args are stored here
                let mut ffi_args: Vec<libffi::middle::Arg> = Vec::with_capacity(args_len);
                for x in &dyn_lib_args {
                    ffi_args.push(libffi::middle::Arg::new(x));
                }

                // Call the function, and convert the result back into Data
                r[dest] = unsafe {
                    match func.get_return_type() {
                        DataType::Int => func.cif.call::<i32>(func.ptr, &ffi_args).into(),
                        DataType::Float => func.cif.call::<f64>(func.ptr, &ffi_args).into(),
                        DataType::String => {
                            let ptr = func
                                .cif
                                .call::<*const std::ffi::c_char>(func.ptr, &ffi_args);
                            if ptr.is_null() {
                                NULL
                            } else {
                                string!(
                                    std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
                                )
                            }
                        }
                        DataType::Null => NULL,
                        DataType::Array(_) => {
                            error_with_catch!(ErrType::CArrayReturnTypeNotSupported);
                        }
                        t => {
                            error_with_catch!(ErrType::InvalidReturnType(t));
                        }
                    }
                };
            }
            Instr::AddFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() + r[o2].as_float()).into();
            }
            Instr::AddInt(o1, o2, dest) => {
                r[dest] = (r[o1].as_int() + r[o2].as_int()).into();
            }
            Instr::AddStr(o1, o2, dest) => {
                let d1 = r[o1];
                let d2 = r[o2];
                let left = d1.as_str(string_pool);
                let right = d2.as_str(string_pool);
                let mut s = String::with_capacity(left.len() + right.len());
                s.push_str(left);
                s.push_str(right);
                r[dest] = string!(s);
            }
            Instr::EmptyArray(arr_reg_id) => {
                let array_id = alloc_array(
                    obj_pool,
                    map_pool,
                    &mut free_arrays,
                    r,
                    &recursion_stack,
                    &mut gc_array_threshold,
                    &mut array_live,
                    &mut map_live,
                    &mut obj_gc_stack,
                );
                r[arr_reg_id] = Data::array(array_id);
            }
            Instr::CloneArray(src_reg, dest_reg, len) => {
                let src_id = r[src_reg].as_array();
                let new_id = alloc_array(
                    obj_pool,
                    map_pool,
                    &mut free_arrays,
                    r,
                    &recursion_stack,
                    &mut gc_array_threshold,
                    &mut array_live,
                    &mut map_live,
                    &mut obj_gc_stack,
                ) as usize;
                let src_ptr = obj_pool[src_id].as_ptr();
                let dst = obj_pool.get_mut(new_id);
                dst.reserve_exact(len as usize);
                unsafe {
                    dst.set_len(len as usize);
                    std::ptr::copy_nonoverlapping(src_ptr, dst.as_mut_ptr(), len as usize);
                }
                r[dest_reg] = Data::array(new_id as u32);
            }
            Instr::CloneStruct(src_reg, dest_reg) => {
                let new_id = alloc_array(
                    obj_pool,
                    map_pool,
                    &mut free_arrays,
                    r,
                    &recursion_stack,
                    &mut gc_array_threshold,
                    &mut array_live,
                    &mut map_live,
                    &mut obj_gc_stack,
                ) as usize;
                let src_reg = r[src_reg];
                let src = &obj_pool[src_reg.as_struct()];
                let len = src.len();
                unsafe {
                    let src_ptr = src.as_ptr();
                    let dst = obj_pool.get_mut(new_id);
                    dst.reserve_exact(len);
                    dst.set_len(len);
                    std::ptr::copy_nonoverlapping(src_ptr, dst.as_mut_ptr(), len);
                }
                r[dest_reg] = Data::struct_instance(src_reg.struct_type_id(), new_id as u32);
            }
            Instr::AddArray(o1, o2, dest) => {
                let arr_a_id = r[o1].as_array();
                let arr_b_id = r[o2].as_array();
                let array_id = alloc_array(
                    obj_pool,
                    map_pool,
                    &mut free_arrays,
                    r,
                    &recursion_stack,
                    &mut gc_array_threshold,
                    &mut array_live,
                    &mut map_live,
                    &mut obj_gc_stack,
                );
                let array_idx = array_id as usize;
                unsafe {
                    let array_pool_ptr = obj_pool.as_mut_ptr();
                    let arr_a = &*array_pool_ptr.add(arr_a_id);
                    let arr_b = &*array_pool_ptr.add(arr_b_id);
                    let combined = &mut *array_pool_ptr.add(array_idx);

                    combined.reserve(arr_a.len() + arr_b.len());
                    combined.extend_from_slice(arr_a);
                    combined.extend_from_slice(arr_b);
                }
                r[dest] = Data::array(array_id);
            }
            Instr::MulFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() * r[o2].as_float()).into();
            }
            Instr::MulInt(o1, o2, dest) => {
                r[dest] = (r[o1].as_int() * r[o2].as_int()).into();
            }
            Instr::DivFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() / r[o2].as_float()).into();
            }
            Instr::DivInt(o1, o2, dest) => {
                let b = r[o2].as_int();
                if b == 0 {
                    error_with_catch!(ErrType::DivisionByZero);
                }
                r[dest] = (r[o1].as_int() / b).into();
            }
            Instr::SubFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() - r[o2].as_float()).into();
            }
            Instr::SubInt(o1, o2, dest) => {
                r[dest] = (r[o1].as_int() - r[o2].as_int()).into();
            }
            Instr::ModFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() % r[o2].as_float()).into();
            }
            Instr::ModInt(o1, o2, dest) => {
                let b = r[o2].as_int();
                if b == 0 {
                    error_with_catch!(ErrType::ModuloByZero);
                }
                r[dest] = (r[o1].as_int() % b).into();
            }
            Instr::PowFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float().powf(r[o2].as_float())).into();
            }
            Instr::PowInt(o1, o2, dest) => {
                r[dest] = (r[o1].as_int().pow(r[o2].as_int() as u32)).into();
            }
            Instr::IncInt(reg) => r[reg].inc_int(),
            Instr::DecInt(reg) => r[reg].dec_int(),
            Instr::IncIntTo(src, dst) => {
                let s = r[src];
                r[dst].inc_into(s);
            }
            Instr::DecIntTo(src, dst) => {
                let s = r[src];
                r[dst].dec_into(s);
            }
            Instr::Eq(o1, o2, dest) => {
                r[dest] = (r[o1] == r[o2]).into();
            }
            Instr::ObjEq(o1, o2, dest) => {
                r[dest] = obj_eq(r[o1], r[o2], obj_pool, map_pool, string_pool).into();
            }
            Instr::NotEqJmp(o1, o2, jump_size) => {
                if r[o1] != r[o2] {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::ObjNotEqJmp(o1, o2, jump_size) => {
                if !obj_eq(r[o1], r[o2], obj_pool, map_pool, string_pool) {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::StrNotEqJmp(o1, o2, jump_size) => {
                if r[o1].as_str(string_pool) != r[o2].as_str(string_pool) {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::NotEq(o1, o2, dest) => {
                r[dest] = (r[o1] != r[o2]).into();
            }
            Instr::ObjNotEq(o1, o2, dest) => {
                r[dest] = (!obj_eq(r[o1], r[o2], obj_pool, map_pool, string_pool)).into();
            }
            Instr::StrEq(o1, o2, dest) => {
                r[dest] = (r[o1].as_str(string_pool) == r[o2].as_str(string_pool)).into();
            }
            Instr::StrNotEq(o1, o2, dest) => {
                r[dest] = (r[o1].as_str(string_pool) != r[o2].as_str(string_pool)).into();
            }
            Instr::EqJmp(o1, o2, jump_size) => {
                if r[o1] == r[o2] {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::ObjEqJmp(o1, o2, jump_size) => {
                if obj_eq(r[o1], r[o2], obj_pool, map_pool, string_pool) {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::StrEqJmp(o1, o2, jump_size) => {
                if r[o1].as_str(string_pool) == r[o2].as_str(string_pool) {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::SupFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() > r[o2].as_float()).into();
            }
            Instr::SupInt(o1, o2, dest) => {
                r[dest] = (r[o1].as_int() > r[o2].as_int()).into();
            }
            Instr::InfEqFloatJmp(o1, o2, jump_size) => {
                if r[o1].as_float() <= r[o2].as_float() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::InfEqIntJmp(o1, o2, jump_size) => {
                if r[o1].as_int() <= r[o2].as_int() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::SupEqFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() >= r[o2].as_float()).into();
            }
            Instr::SupEqInt(o1, o2, dest) => {
                r[dest] = (r[o1].as_int() >= r[o2].as_int()).into();
            }
            Instr::InfFloatJmp(o1, o2, jump_size) => {
                if r[o1].as_float() < r[o2].as_float() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::InfIntJmp(o1, o2, jump_size) => {
                if r[o1].as_int() < r[o2].as_int() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::InfIntJmpBack(o1, o2, jump_size) => {
                if r[o1].as_int() < r[o2].as_int() {
                    i -= jump_size as usize;
                    continue;
                }
            }
            Instr::InfFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() < r[o2].as_float()).into();
            }
            Instr::InfInt(o1, o2, dest) => {
                r[dest] = (r[o1].as_int() < r[o2].as_int()).into();
            }
            Instr::SupEqFloatJmp(o1, o2, jump_size) => {
                if r[o1].as_float() >= r[o2].as_float() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::SupEqIntJmp(o1, o2, jump_size) => {
                if r[o1].as_int() >= r[o2].as_int() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::InfEqFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() <= r[o2].as_float()).into();
            }
            Instr::InfEqInt(o1, o2, dest) => {
                r[dest] = (r[o1].as_int() <= r[o2].as_int()).into();
            }
            Instr::SupFloatJmp(o1, o2, jump_size) => {
                if r[o1].as_float() > r[o2].as_float() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::SupIntJmp(o1, o2, jump_size) => {
                if r[o1].as_int() > r[o2].as_int() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::BoolAnd(o1, o2, dest) => {
                r[dest] = (r[o1].as_bool() && r[o2].as_bool()).into();
            }
            Instr::BoolOr(o1, o2, dest) => {
                r[dest] = (r[o1].as_bool() || r[o2].as_bool()).into();
            }
            Instr::NegBool(src, dest) => {
                r[dest] = (!r[src].as_bool()).into();
            }
            Instr::NegFloat(tgt, dest) => {
                r[dest] = (-r[tgt].as_float()).into();
            }
            Instr::NegInt(tgt, dest) => {
                r[dest] = (-r[tgt].as_int()).into();
            }
            Instr::Print(tgt) => {
                let tgt = r[tgt];
                if tgt.is_str() {
                    writeln!(handle, "{}", tgt.as_str(string_pool)).unwrap();
                } else if tgt.is_int() {
                    writeln!(handle, "{}", tgt.as_int()).unwrap();
                } else if tgt.is_float() {
                    writeln!(handle, "{}", tgt.as_float()).unwrap();
                } else if tgt.is_bool() {
                    writeln!(handle, "{}", tgt.as_bool()).unwrap();
                } else if tgt.is_array() {
                    let array = &obj_pool[tgt.as_array()];
                    write!(handle, "[").unwrap();
                    for (idx, item) in array.iter().enumerate() {
                        if idx != 0 {
                            write!(handle, ",").unwrap();
                        }
                        write!(
                            handle,
                            "{}",
                            item.format(obj_pool, string_pool, map_pool, struct_fields, false)
                        )
                        .unwrap();
                    }
                    writeln!(handle, "]").unwrap();
                } else if tgt.is_struct() {
                    let s = &obj_pool[tgt.as_struct()];
                    let (s_name, s_fields) =
                        unsafe { struct_fields.get_unchecked(tgt.struct_type_id() as usize) };
                    write!(handle, "{s_name} {{").unwrap();
                    for (idx, item) in s.iter().enumerate() {
                        if idx != 0 {
                            write!(handle, ",").unwrap();
                        }
                        write!(
                            handle,
                            "{}:{}",
                            unsafe { s_fields.get_unchecked(idx) },
                            item.format(obj_pool, string_pool, map_pool, struct_fields, false)
                        )
                        .unwrap();
                    }
                    writeln!(handle, "}}").unwrap();
                } else if tgt.is_map() {
                    let m = &map_pool[tgt.as_map()];
                    write!(handle, "{{").unwrap();
                    for (i, (key, val)) in m.iter().enumerate() {
                        if i != 0 {
                            write!(handle, ",").unwrap();
                        }
                        write!(
                            handle,
                            "{}:{}",
                            key.format(obj_pool, string_pool, map_pool, struct_fields, false),
                            val.format(obj_pool, string_pool, map_pool, struct_fields, false),
                        )
                        .unwrap();
                    }
                    writeln!(handle, "}}").unwrap();
                }
            }
            Instr::StoreFuncArg(id) => args.push(id),
            Instr::ObjElemMov(new_elem_reg_id, array_id, idx) => {
                let arr = obj_pool.get_mut(array_id as usize);
                unsafe {
                    *arr.get_unchecked_mut(idx as usize) = r[new_elem_reg_id];
                }
            }
            Instr::SetElementObj(array_reg_id, new_elem_reg_id, idx) => {
                let array = obj_pool.get_mut(r[array_reg_id].as_array());
                let index = r[idx].as_int();
                if (index as usize) >= array.len() || index < 0 {
                    error_with_catch!(ErrType::IndexOutOfBounds(array.len(), index));
                }
                array[index as usize] = r[new_elem_reg_id];
            }
            Instr::SetElementString(string_reg_id, new_str_reg_id, idx) => {
                let index = r[idx].as_int();
                let temp_str_reg_id = r[string_reg_id];
                let source_string = temp_str_reg_id.as_str(string_pool);
                if (index as usize) >= source_string.len() || index < 0 {
                    error_with_catch!(ErrType::IndexOutOfBounds(source_string.len(), index));
                }
                let mut temp = source_string.to_owned();
                temp.remove(index as usize);
                temp.insert_str(index as usize, r[new_str_reg_id].as_str(string_pool));
                r[string_reg_id] = string!(temp);
            }
            Instr::SetFieldStruct(struct_reg_id, new_elem_reg_id, idx) => {
                let s = obj_pool.get_mut(r[struct_reg_id].as_struct());
                unsafe {
                    *s.get_unchecked_mut(idx as usize) = r[new_elem_reg_id];
                }
            }
            Instr::GetIndexArray(array_reg_id, index, dest) => {
                let idx = r[index].as_int();
                let arr_id = r[array_reg_id].as_array();
                let array = &obj_pool[arr_id];
                if (idx as usize) >= array.len() || idx < 0 {
                    error_with_catch!(ErrType::IndexOutOfBounds(array.len(), idx));
                }
                r[dest] = unsafe { *array.get_unchecked(idx as usize) };
            }
            Instr::GetFieldStruct(struct_reg_id, index, dest) => {
                let struct_id = r[struct_reg_id].as_struct();
                let s = &obj_pool[struct_id];
                r[dest] = unsafe { *s.get_unchecked(index as usize) }
            }
            Instr::GetSliceArray(array_reg_id, idx_start_id, dest_reg_id) => {
                let idx_start = r[idx_start_id].as_int();
                let idx_end = r[args.pop_unchecked()].as_int();
                let arr_id = r[array_reg_id].as_array();
                let array = &obj_pool[arr_id];
                if (idx_end as usize) > array.len()
                    || (idx_start as usize) >= array.len()
                    || idx_start > idx_end
                {
                    error_with_catch!(ErrType::SliceOutOfBounds(array.len(), idx_start, idx_end));
                }
                let new_array_id = alloc_array(
                    obj_pool,
                    map_pool,
                    &mut free_arrays,
                    r,
                    &recursion_stack,
                    &mut gc_array_threshold,
                    &mut array_live,
                    &mut map_live,
                    &mut obj_gc_stack,
                );
                unsafe {
                    if arr_id < (new_array_id as usize) {
                        let (left, right) = obj_pool.split_at_mut(new_array_id as usize);
                        right.get_unchecked_mut(0).extend_from_slice(
                            left.get_unchecked(arr_id)
                                .get_unchecked((idx_start as usize)..(idx_end as usize)),
                        );
                    } else {
                        let (left, right) = obj_pool.split_at_mut(arr_id);
                        left.get_unchecked_mut(new_array_id as usize)
                            .extend_from_slice(
                                right
                                    .get_unchecked(0)
                                    .get_unchecked((idx_start as usize)..(idx_end as usize)),
                            );
                    }
                }
                r[dest_reg_id] = Data::array(new_array_id);
            }
            // Keel currently indexes strings by byte, meaning multi-byte characters won't get properly indexed
            Instr::GetIndexString(tgt, index, dest) => {
                let idx = r[index].as_int();
                let tgt_data = r[tgt];
                let bytes = tgt_data.as_str(string_pool).as_bytes();
                if (idx as usize) >= bytes.len() {
                    error_with_catch!(ErrType::IndexOutOfBounds(bytes.len(), idx));
                }
                r[dest] = str!(unsafe {
                    std::str::from_utf8_unchecked(std::slice::from_ref(
                        bytes.get_unchecked(idx as usize),
                    ))
                });
            }
            Instr::GetSliceString(str_reg_id, idx_start, dest_reg_id) => {
                let idx_start = r[idx_start].as_int();
                let idx_end = r[args.pop_unchecked()].as_int();
                let s = r[str_reg_id].as_str(string_pool).to_smolstr();
                if (idx_end as usize) > s.len()
                    || (idx_start as usize) >= s.len()
                    || idx_start > idx_end
                {
                    error_with_catch!(ErrType::SliceOutOfBounds(s.len(), idx_start, idx_end));
                }
                r[dest_reg_id] = str!(&s[(idx_start as usize)..(idx_end as usize)]);
            }
            Instr::Push(array, element) => {
                obj_pool.get_mut(r[array].as_array()).push(r[element]);
            }
            Instr::Remove(array, idx) => {
                let arr = obj_pool.get_mut(r[array].as_array());
                let index = r[idx].as_int();
                if (index as usize) >= arr.len() || index < 0 {
                    error_with_catch!(ErrType::IndexOutOfBounds(arr.len(), index));
                }
                arr.remove(index as usize);
            }
            Instr::MapGet(map_reg_id, key_reg_id, dest_reg_id) => {
                r[dest_reg_id] = if let Some(elem) =
                    unsafe { map_pool[r[map_reg_id].as_map()].get(&r[key_reg_id]) }
                {
                    *elem
                } else {
                    cold_path();
                    error_with_catch!(ErrType::UnknownMapKey(
                        r[key_reg_id]
                            .format(obj_pool, string_pool, map_pool, struct_fields, false)
                            .as_str()
                    ));
                };
            }
            Instr::MapInsert(map_pool_id, key_reg_id, val_reg_id) => unsafe {
                map_pool[map_pool_id as usize].insert(r[key_reg_id], r[val_reg_id]);
            },
            Instr::MapInsertReg(map_reg_id, key_reg_id, val_reg_id) => unsafe {
                map_pool[r[map_reg_id].as_map()].insert(r[key_reg_id], r[val_reg_id]);
            },
            Instr::CloneMap(src_reg, dest_reg) => {
                let new_map = map_pool[r[src_reg].as_map()].clone();
                let new_id = alloc_map(
                    map_pool,
                    obj_pool,
                    &mut free_maps,
                    r,
                    &recursion_stack,
                    &mut gc_map_threshold,
                    &mut map_live,
                    &mut array_live,
                    &mut obj_gc_stack,
                );
                unsafe {
                    map_pool[new_id as usize] = new_map;
                }
                r[dest_reg] = Data::map(new_id);
            }
            Instr::CallLibFunc(LibFunc::Uppercase, source_string_reg_id, dest_reg_id) => {
                r[dest_reg_id] =
                    string!(r[source_string_reg_id].as_str(string_pool).to_uppercase());
            }
            Instr::CallLibFunc(LibFunc::Lowercase, source_string_reg_id, dest_reg_id) => {
                r[dest_reg_id] =
                    string!(r[source_string_reg_id].as_str(string_pool).to_lowercase());
            }
            Instr::CallLibFunc(LibFunc::Contains, tgt, dest) => {
                let reg = r[tgt];
                if reg.is_str() {
                    let str = reg.as_str(string_pool);
                    let temp_arg = r[args.pop_unchecked()];
                    let arg = temp_arg.as_str(string_pool);
                    // r[dest] = str.contains(arg).into();
                    r[dest] = memmem::find(str.as_bytes(), arg.as_bytes())
                        .is_some()
                        .into();
                } else if reg.is_array() {
                    let arg = r[args.pop_unchecked()];
                    r[dest] = obj_pool[reg.as_array()].contains(&arg).into();
                }
            }
            Instr::CallLibFunc(LibFunc::Trim, tgt, dest) => {
                r[dest] = str!(r[tgt].as_str(string_pool).trim());
            }
            Instr::CallLibFunc(LibFunc::TrimSequence, tgt, dest) => {
                let temp_arg = r[args.pop_unchecked()];
                let arg = temp_arg.as_str(string_pool);
                let chars: Vec<char> = arg.chars().collect();
                r[dest] = str!(r[tgt].as_str(string_pool).trim_matches(&chars[..]));
            }
            Instr::CallLibFunc(LibFunc::Find, tgt, dest) => {
                let reg = r[tgt];
                if reg.is_str() {
                    let str = reg.as_str(string_pool);
                    let temp_elem = r[args.pop_unchecked()];
                    let element = temp_elem.as_str(string_pool);
                    r[dest] = if let Some(idx) = memmem::find(str.as_bytes(), element.as_bytes()) {
                        idx as i32
                    } else {
                        cold_path();
                        -1
                    }
                    .into();
                } else if reg.is_array() {
                    let arr_id = reg.as_array();
                    let element = r[args.pop_unchecked()];
                    r[dest] =
                        if let Some(idx) = obj_pool[arr_id].iter().position(|x| x == &element) {
                            idx as i32
                        } else {
                            cold_path();
                            -1
                        }
                        .into();
                }
            }
            Instr::CallLibFunc(LibFunc::IsFloat, tgt, dest) => {
                let temp_tgt = r[tgt];
                let num = temp_tgt.as_str(string_pool);
                r[dest] = (num.parse::<i64>().is_err() && num.parse::<f64>().is_ok()).into();
            }
            Instr::CallLibFunc(LibFunc::IsInt, tgt, dest) => {
                r[dest] = r[tgt].as_str(string_pool).parse::<i64>().is_ok().into();
            }
            Instr::CallLibFunc(LibFunc::TrimLeft, tgt, dest) => {
                r[dest] = str!(r[tgt].as_str(string_pool).trim_start());
            }
            Instr::CallLibFunc(LibFunc::TrimRight, tgt, dest) => {
                r[dest] = str!(r[tgt].as_str(string_pool).trim_end());
            }
            Instr::CallLibFunc(LibFunc::TrimSequenceLeft, tgt, dest) => {
                let chars: Vec<char> = r[args.pop_unchecked()]
                    .as_str(string_pool)
                    .chars()
                    .collect();
                r[dest] = str!(r[tgt].as_str(string_pool).trim_start_matches(&chars[..]));
            }
            Instr::CallLibFunc(LibFunc::TrimSequenceRight, tgt, dest) => {
                let chars: Vec<char> = r[args.pop_unchecked()]
                    .as_str(string_pool)
                    .chars()
                    .collect();
                r[dest] = str!(r[tgt].as_str(string_pool).trim_end_matches(&chars[..]));
            }
            Instr::CallLibFunc(LibFunc::Repeat, tgt, dest) => {
                let reg = r[tgt];
                if reg.is_str() {
                    let str = reg.as_str(string_pool);
                    let repeat_count = r[args.pop_unchecked()].as_int();
                    r[dest] = string!(str.repeat(repeat_count as usize));
                } else if reg.is_array() {
                    let repeat_count = r[args.pop_unchecked()].as_int();
                    let array_id = alloc_array(
                        obj_pool,
                        map_pool,
                        &mut free_arrays,
                        r,
                        &recursion_stack,
                        &mut gc_array_threshold,
                        &mut array_live,
                        &mut map_live,
                        &mut obj_gc_stack,
                    );
                    obj_pool[array_id as usize] =
                        obj_pool[reg.as_array()].repeat(repeat_count as usize);
                    r[dest] = Data::array(array_id);
                }
            }
            Instr::CallLibFunc(LibFunc::Round, tgt, dest) => {
                r[dest] = r[tgt].as_float().round().into();
            }
            Instr::CallLibFunc(LibFunc::Abs, tgt, dest) => {
                let tgt = r[tgt];
                r[dest] = if tgt.is_float() {
                    tgt.as_float().abs().into()
                } else {
                    tgt.as_int().abs().into()
                }
            }
            Instr::CallLibFunc(LibFunc::Reverse, tgt, dest) => {
                r[dest] = string!(r[tgt].as_str(string_pool).chars().rev().collect::<String>());
            }
            Instr::CallLibFuncVoid(LibFuncVoid::Reverse, tgt, _) => {
                obj_pool.get_mut(r[tgt].as_array()).reverse();
            }
            Instr::CallLibFunc(LibFunc::SqrtFloat, tgt, dest) => {
                r[dest] = r[tgt].as_float().sqrt().into();
            }
            Instr::CallLibFunc(LibFunc::Float, tgt, dest) => {
                let reg = r[tgt];
                if reg.is_int() {
                    r[dest] = (reg.as_int() as f64).into();
                } else if reg.is_str() {
                    let str = reg.as_str(string_pool);
                    r[dest] = if let Ok(f) = lexical_core::parse::<f64>(str.as_bytes()) {
                        f.into()
                    } else {
                        error_with_catch!(ErrType::InvalidFloat);
                    }
                }
            }
            Instr::CallLibFunc(LibFunc::Int, tgt, dest) => {
                let reg = r[tgt];
                if reg.is_float() {
                    r[dest] = (reg.as_float() as i32).into();
                } else if reg.is_str() {
                    let str = reg.as_str(string_pool);
                    r[dest] = if let Ok(i) = lexical_core::parse::<i32>(str.as_bytes()) {
                        i.into()
                    } else {
                        cold_path();
                        error_with_catch!(ErrType::InvalidInt);
                    }
                }
            }
            Instr::CallLibFunc(LibFunc::Str, tgt, dest) => {
                let value = r[tgt];
                r[dest] = if value.is_int() {
                    let mut buffer = [0u8; i32::FORMATTED_SIZE_DECIMAL];
                    let digits = lexical_core::write(value.as_int(), &mut buffer);
                    str!(unsafe { str::from_utf8_unchecked(digits) })
                } else if value.is_float() {
                    str!(zmij::Buffer::new().format(value.as_float()))
                } else if value.is_bool() {
                    Data::small_str(if value.as_bool() { "true" } else { "false" })
                } else if value.is_str() {
                    value
                } else {
                    str!(
                        value
                            .format(obj_pool, string_pool, map_pool, struct_fields, false)
                            .as_str()
                    )
                };
            }
            Instr::CallLibFunc(LibFunc::Bool, tgt, dest) => {
                let temp_tgt = r[tgt];
                let str = temp_tgt.as_str(string_pool);
                r[dest] = if str == "true" {
                    TRUE
                } else if str == "false" {
                    FALSE
                } else {
                    cold_path();
                    error_with_catch!(ErrType::InvalidBool);
                }
            }
            #[cfg(target_arch = "wasm32")]
            Instr::CallLibFunc(LibFunc::Input, _, _) => wasm_error("WASM does not support input()"),
            #[cfg(not(target_arch = "wasm32"))]
            Instr::CallLibFunc(LibFunc::Input, tgt, dest) => {
                let temp_tgt = r[tgt];
                let str_msg = temp_tgt.as_str(string_pool);
                write!(handle, "{str_msg}").unwrap();
                std::io::stdout().flush().unwrap();
                let mut line = String::new();
                std::io::stdin().read_line(&mut line).unwrap();
                r[dest] = str!(line.trim_end_matches(['\n', '\r']));
            }
            Instr::CallLibFunc(LibFunc::Floor, tgt, dest) => {
                r[dest] = r[tgt].as_float().floor().into();
            }
            #[allow(unused_must_use)]
            Instr::CallLibFunc(LibFunc::TheAnswer, _, dest) => {
                writeln!(
                    handle,
                    "The answer to the Ultimate Question of Life, the Universe, and Everything is 42."
                );
                r[dest] = 42.into();
            }
            Instr::CallLibFunc(LibFunc::Len, tgt, dest) => {
                let reg = r[tgt];
                if reg.is_array() {
                    r[dest] = (obj_pool[reg.as_array()].len() as i32).into();
                } else if reg.is_str() {
                    r[dest] = (reg.as_str(string_pool).len() as i32).into();
                }
            }
            Instr::CallLibFunc(LibFunc::StartsWith, source_register, dest_register) => {
                r[dest_register] = r[source_register]
                    .as_str(string_pool)
                    .starts_with(r[args.pop_unchecked()].as_str(string_pool))
                    .into();
            }
            Instr::CallLibFunc(LibFunc::EndsWith, source_register, dest_register) => {
                r[dest_register] = r[source_register]
                    .as_str(string_pool)
                    .ends_with(r[args.pop_unchecked()].as_str(string_pool))
                    .into();
            }
            #[allow(clippy::no_effect_replace)]
            Instr::CallLibFunc(LibFunc::Replace, source_register, dest_register) => {
                r[dest_register] = string!(r[source_register].as_str(string_pool).replace(
                    r[args.pop_unchecked()].as_str(string_pool),
                    r[args.pop_unchecked()].as_str(string_pool),
                ));
            }
            Instr::CallLibFunc(LibFunc::Split, source_register, dest_register) => {
                let source = r[source_register];
                let separator = unsafe { args.pop_unchecked() };
                if source.is_str() {
                    let output_str_reg_id = alloc_array(
                        obj_pool,
                        map_pool,
                        &mut free_arrays,
                        r,
                        &recursion_stack,
                        &mut gc_array_threshold,
                        &mut array_live,
                        &mut map_live,
                        &mut obj_gc_stack,
                    );
                    let source = source.as_str(string_pool);
                    let separator_data = r[separator];
                    let separator = separator_data.as_str(string_pool);
                    let output = obj_pool.get_mut(output_str_reg_id as usize);
                    output.clear();
                    output.reserve(source.len() / 4);
                    let separator_len = separator.len();
                    let mut i = 0;
                    for part_i in memmem::find_iter(source.as_bytes(), separator.as_bytes()) {
                        let part = &source[i..part_i];
                        output.push({
                            if part.len() <= 6 {
                                Data::small_str(part)
                            } else if let Some(id) = free_strings.pop() {
                                part.clone_into(&mut string_pool[id as usize]);
                                Data::large_str_id(id as u64)
                            } else {
                                let id = string_pool.len() as u64;
                                string_pool.push(part.to_owned());
                                Data::large_str_id(id)
                            }
                        });
                        i = part_i + separator_len;
                    }
                    let part = &source[i..];
                    output.push({
                        if part.len() <= 6 {
                            Data::small_str(part)
                        } else if let Some(id) = free_strings.pop() {
                            part.clone_into(&mut string_pool[id as usize]);
                            Data::large_str_id(id as u64)
                        } else {
                            let id = string_pool.len() as u64;
                            string_pool.push(part.to_owned());
                            Data::large_str_id(id)
                        }
                    });
                    raise_string_gc_threshold(&mut gc_string_threshold, string_pool.len());
                    r[dest_register] = Data::array(output_str_reg_id);
                } else if source.is_array() {
                    let source_array_id = source.as_array();
                    let separator = r[separator];
                    let source_array = &obj_pool[source_array_id];

                    let mut split_ranges: Vec<(usize, usize)> =
                        Vec::with_capacity(source_array.len() + 1);

                    let mut start = 0;
                    for (idx, item) in source_array.iter().enumerate() {
                        if *item == separator {
                            split_ranges.push((start, idx));
                            start = idx + 1;
                        }
                    }
                    split_ranges.push((start, source_array.len()));

                    // alloc one array per range
                    let mut sub_arrays: Vec<Data> = Vec::with_capacity(split_ranges.len());
                    for (start, end) in split_ranges {
                        let dest_array_id = alloc_array(
                            obj_pool,
                            map_pool,
                            &mut free_arrays,
                            r,
                            &recursion_stack,
                            &mut gc_array_threshold,
                            &mut array_live,
                            &mut map_live,
                            &mut obj_gc_stack,
                        ) as usize;
                        unsafe {
                            if dest_array_id < source_array_id {
                                let (left, right) = obj_pool.split_at_mut(source_array_id);
                                left.get_unchecked_mut(dest_array_id).extend_from_slice(
                                    right.get_unchecked(0).get_unchecked(start..end),
                                );
                            } else {
                                let (left, right) = obj_pool.split_at_mut(dest_array_id);
                                right.get_unchecked_mut(0).extend_from_slice(
                                    left.get_unchecked(source_array_id)
                                        .get_unchecked(start..end),
                                );
                            }
                        }
                        sub_arrays.push(Data::array(dest_array_id as u32));
                    }

                    let array_id = alloc_array(
                        obj_pool,
                        map_pool,
                        &mut free_arrays,
                        r,
                        &recursion_stack,
                        &mut gc_array_threshold,
                        &mut array_live,
                        &mut map_live,
                        &mut obj_gc_stack,
                    );
                    obj_pool[array_id as usize] = sub_arrays;

                    r[dest_register] = Data::array(array_id);
                }
            }
            Instr::CallLibFunc(LibFunc::Range, max, dest) => {
                let min = args.pop().map_or(0, |reg_id| r[reg_id].as_int());
                let max = r[max].as_int();
                let output_array_id = alloc_array(
                    obj_pool,
                    map_pool,
                    &mut free_arrays,
                    r,
                    &recursion_stack,
                    &mut gc_array_threshold,
                    &mut array_live,
                    &mut map_live,
                    &mut obj_gc_stack,
                );
                let range_arr = obj_pool.get_mut(output_array_id as usize);
                range_arr.extend((min..max).map(Data::from));
                r[dest] = Data::array(output_array_id);
            }
            Instr::CallLibFunc(LibFunc::JoinStringArray, tgt, dest) => {
                let temp_separator: Option<Data> = args.pop().map(|arg| r[arg]);
                let separator = temp_separator
                    .as_ref()
                    .map_or("", |d| d.as_str(string_pool));
                let array = &obj_pool[r[tgt].as_array()];
                let total_len: usize = array
                    .iter()
                    .map(|x| x.as_str(string_pool).len())
                    .sum::<usize>()
                    + separator
                        .len()
                        .saturating_mul(array.len().saturating_sub(1));
                let mut output = String::with_capacity(total_len);
                for (i, x) in array.iter().enumerate() {
                    if i > 0 {
                        output.push_str(separator);
                    }
                    output.push_str(x.as_str(string_pool));
                }
                r[dest] = string!(output);
            }
            // -----
            // FILE SYSTEM FUNCTIONS
            // -----
            Instr::CallLibFunc(LibFunc::FsRead, path, dest_reg_id) => {
                r[dest_reg_id] = string!(match fs::read_to_string(r[path].as_str(string_pool)) {
                    Ok(p) => p,
                    Err(e) => {
                        cold_path();
                        error_with_catch!(ErrType::from(e.kind()));
                    }
                });
            }
            Instr::CallLibFunc(LibFunc::FsExists, path, dest_reg_id) => {
                r[dest_reg_id] = match fs::exists(r[path].as_str(string_pool)) {
                    Ok(b) => b.into(),
                    Err(e) => {
                        error_with_catch!(ErrType::from(e.kind()));
                    }
                }
            }
            // Overwrites a file, will create it if it doesn't exist
            Instr::CallLibFuncVoid(LibFuncVoid::FsWrite, path, contents) => {
                if let Err(e) =
                    fs::write(r[path].as_str(string_pool), r[contents].as_str(string_pool))
                {
                    error_with_catch!(ErrType::from(e.kind()));
                }
            }
            // Appends to a file, will create if it doesn't exist
            Instr::CallLibFuncVoid(LibFuncVoid::FsAppend, path, contents) => {
                match fs::OpenOptions::new()
                    .append(true)
                    .open(r[path].as_str(string_pool))
                {
                    Ok(mut f) => {
                        if let Err(e) = f.write_all(r[contents].as_str(string_pool).as_bytes()) {
                            error_with_catch!(ErrType::from(e.kind()));
                        }
                    }
                    Err(e) => {
                        error_with_catch!(ErrType::from(e.kind()));
                    }
                }
            }
            // Deletes the file located at `path`, throwing an error if it doesn't exist.
            Instr::CallLibFuncVoid(LibFuncVoid::FsDelete, path, _) => {
                if let Err(e) = fs::remove_file(r[path].as_str(string_pool)) {
                    error_with_catch!(ErrType::from(e.kind()));
                }
            }
            // Deletes the empty directory located at `path`
            Instr::CallLibFuncVoid(LibFuncVoid::FsDeleteDir, path, _) => {
                if let Err(e) = fs::remove_dir(r[path].as_str(string_pool)) {
                    error_with_catch!(ErrType::from(e.kind()));
                }
            }
            #[cfg(target_arch = "wasm32")]
            Instr::CallLibFunc(LibFunc::Argv, _, dest) => {
                r[dest] = Data::array(alloc_array(
                    obj_pool,
                    map_pool,
                    &mut free_arrays,
                    r,
                    &recursion_stack,
                    &mut gc_array_threshold,
                    &mut array_live,
                    &mut map_live,
                    &mut obj_gc_stack,
                ))
            }
            #[cfg(not(target_arch = "wasm32"))]
            Instr::CallLibFunc(LibFunc::Argv, _, dest) => {
                let array_id = alloc_array(
                    obj_pool,
                    map_pool,
                    &mut free_arrays,
                    r,
                    &recursion_stack,
                    &mut gc_array_threshold,
                    &mut array_live,
                    &mut map_live,
                    &mut obj_gc_stack,
                );
                obj_pool[array_id as usize] = std::env::args()
                    .skip(2)
                    .map(|s| string!(s))
                    .collect::<Vec<Data>>();
                r[dest] = Data::array(array_id);
            }
            Instr::CallLibFuncVoid(LibFuncVoid::Sort, tgt, _) => {
                let array = obj_pool.get_mut(r[tgt].as_array());
                if !array.is_empty() {
                    if array[0].is_int() {
                        array.sort_unstable_by_key(|x| x.as_int());
                    } else if array[0].is_float() {
                        array.sort_unstable_by(|a, b| {
                            a.as_float()
                                .partial_cmp(&b.as_float())
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                    } else if array[0].is_str() {
                        array.sort_unstable_by(|a, b| {
                            a.as_str(string_pool).cmp(b.as_str(string_pool))
                        });
                    }
                }
            }
            Instr::StartErrorCatch(jmp_size, err_reg_id) => {
                error_handles.push(ErrorCatch {
                    catch_loc: (i as u32) + (jmp_size as u32),
                    error_reg: err_reg_id,
                    call_frames_len: call_frames.len() as u32,
                    args_len: args.len() as u32,
                });
            }
            Instr::StopErrorCatch => unsafe {
                error_handles.pop_unchecked();
            },
            Instr::ThrowError(error_reg_id) => {
                error_with_catch!(ErrType::Custom(r[error_reg_id].as_str(string_pool)));
            }
            Instr::Halt(code) => {
                cold_path();

                #[cfg(not(target_arch = "wasm32"))]
                if code != 0 {
                    std::process::exit(r[code].as_int());
                }

                break;
            }
        }
        i += 1;
    }
}
