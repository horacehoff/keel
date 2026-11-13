use crate::compiler::program::DylibFn;
use crate::compiler::program::ErrorCatch;
use crate::compiler::program::Program;
use crate::compiler::program::Struct;
use crate::compiler::type_system::DataType;
use crate::compiler::type_system::VmType;
use crate::data::Data;
use crate::data::FALSE;
use crate::data::NULL;
use crate::data::TRUE;
use crate::errors::ErrType;
use crate::errors::ErrorCtx;
use crate::errors::throw_error;
#[cfg(target_arch = "wasm32")]
use crate::errors::wasm_error;
use crate::fs;
use crate::instr::Instr;
use crate::vm::comparison::get_array_position;
use crate::vm::comparison::get_heap_array_position;
pub use crate::vm::pools::Pool;
use comparison::obj_eq;
use core::hint::assert_unchecked;
use core::hint::cold_path;
use core::ops::Index;
use core::ops::IndexMut;
use gc::Gc;
use lexical_core::FormattedSize;
use memchr::memmem;
pub use pools::MapPool;
pub use pools::ObjectPool;
pub use pools::StringPool;
use seq_macro::seq;
use stack::Stack;
#[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
use std::io::IsTerminal;
use std::io::Write;
use unchecked_ops::UncheckedVecOps;

mod comparison;
mod ffi;
pub mod gc;
mod pools;
mod stack;
mod unchecked_ops;

#[inline(always)]
const fn jump(ip: *const Instr, offset: u16) -> *const Instr {
    ip.wrapping_offset(offset as i16 as isize)
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Frame(*mut Data);

impl Index<u16> for Frame {
    type Output = Data;
    #[inline(always)]
    fn index(&self, index: u16) -> &Self::Output {
        unsafe { &*self.0.add(index as usize) }
    }
}
impl IndexMut<u16> for Frame {
    #[inline(always)]
    fn index_mut(&mut self, index: u16) -> &mut Data {
        unsafe { &mut *self.0.add(index as usize) }
    }
}

#[cold]
#[inline(never)]
fn error_with_catch(
    err: &ErrType,
    ip: *const Instr,
    ctx: *mut VmCtx<'static>,
) -> (*const Instr, Frame) {
    let ctx = unsafe { &mut *ctx };
    if ctx.error_handles.is_empty() {
        flush_output(&mut ctx.output);
        throw_error(ctx.err_ctx, unsafe { ip.offset_from(ctx.instr_base) } as usize, err);
    }
    let err_handle = ctx.error_handles.pop_unchecked();
    let mut r = Frame(err_handle.frame_ptr);
    r[err_handle.error_reg] = Data::string(
        err.kind().into(),
        &ctx.obj_pool,
        &ctx.map_pool,
        &mut ctx.str_pool,
        ctx.stack.roots(r),
        &mut ctx.gc,
    );
    (unsafe { ctx.instr_base.add(err_handle.catch_loc as usize) }, r)
}

const OUTPUT_FLUSH_THRESHOLD: usize = 64 * 1024;

#[inline(never)]
fn flush_output(output: &mut String) {
    #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
    {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(output.as_bytes()).unwrap();
        stdout.flush().unwrap();
    }
    #[cfg(any(target_arch = "wasm32", feature = "embed"))]
    crate::captured_output::print(output);
    output.clear();
}

#[allow(clippy::struct_field_names)]
struct VmCtx<'a> {
    stack: Stack,
    output: String,
    gc: Gc,
    #[cfg(not(target_arch = "wasm32"))]
    ffi_args: Vec<libffi::middle::Arg<'a>>,
    dyn_lib_args: Vec<u64>,
    keep_alive: Vec<Box<[u8]>>,
    error_handles: Vec<ErrorCatch>,
    obj_pool: ObjectPool,
    map_pool: MapPool,
    str_pool: StringPool,
    err_ctx: &'a ErrorCtx<'a>,
    dyn_libs: &'a [DylibFn],
    structs: &'a [Struct<'a>],
    types: &'a [DataType],
    flush_threshold: usize,
    instr_base: *const Instr,
}

#[cfg(test)]
type VmReturnType = Vec<Data>;
#[cfg(not(test))]
type VmReturnType = ();

pub fn execute(program: &mut Program) -> VmReturnType {
    let obj_pool = core::mem::replace(&mut program.pools.obj_pool, Pool(Vec::new()));
    let map_pool = core::mem::replace(&mut program.pools.map_pool, Pool(Vec::new()));
    let str_pool = core::mem::replace(&mut program.pools.str_pool, Pool(Vec::new()));
    let Program {
        ref instructions,
        ref initial_stack,
        top_level_base,
        max_frame_regs,
        ref err_ctx,
        dylib_fns: ref dyn_libs,
        ref structs,
        ref types,
        ..
    } = *program;

    let stack = Stack::new(initial_stack, max_frame_regs as usize);
    let r = Frame(unsafe { stack.ptr.add(top_level_base as usize) });

    let mut ctx = VmCtx {
        stack,
        output: String::with_capacity(OUTPUT_FLUSH_THRESHOLD),
        gc: Gc::new(&obj_pool, &map_pool, &str_pool),
        #[cfg(not(target_arch = "wasm32"))]
        // Args converted from Data to libffi args are stored here
        ffi_args: Vec::new(),
        dyn_lib_args: Vec::new(),
        keep_alive: Vec::new(),
        error_handles: Vec::new(),
        obj_pool,
        map_pool,
        str_pool,
        err_ctx,
        dyn_libs,
        structs,
        types,
        #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
        flush_threshold: if std::io::stdout().is_terminal() {0} else {OUTPUT_FLUSH_THRESHOLD},
        #[cfg(any(target_arch = "wasm32", feature = "embed"))]
        flush_threshold: OUTPUT_FLUSH_THRESHOLD,
        instr_base: instructions.as_ptr(),
    };

    execute_at(instructions.as_ptr(), r, (&raw mut ctx).cast());

    #[cfg(not(any(test, target_arch = "wasm32", feature = "embed")))]
    {
        core::mem::forget(ctx.obj_pool);
        core::mem::forget(ctx.map_pool);
        core::mem::forget(ctx.str_pool);
    }
    #[cfg(test)]
    {
        program.pools.obj_pool = ctx.obj_pool;
        program.pools.map_pool = ctx.map_pool;
        program.pools.str_pool = ctx.str_pool;
        unsafe { core::slice::from_raw_parts(ctx.stack.ptr, initial_stack.len()) }.to_vec()
    }
}

static INSTR_TABLE: [extern "rust-preserve-none" fn(*const Instr, Frame, *mut VmCtx<'static>);
    Instr::Halt(0).discriminant() as usize + 1] = seq!(N in 0..114 {[#(execute_instr::<N>,)*]});

#[inline(always)]
extern "rust-preserve-none" fn execute_at(ip: *const Instr, r: Frame, ctx: *mut VmCtx<'static>) {
    unsafe {
        let handler = *INSTR_TABLE.get_unchecked((*ip).discriminant() as usize);
        become handler(ip, r, ctx)
    }
}

#[inline(never)]
fn parse_float(s: &str) -> Option<f64> {
    lexical_core::parse::<f64>(s.as_bytes()).ok()
}

extern "rust-preserve-none" fn execute_instr<const DISCRIMINANT: u8>(
    mut ip: *const Instr,
    mut r: Frame,
    ctx: *mut VmCtx<'static>,
) {
    let VmCtx {
        stack,
        output,
        gc,
        #[cfg(not(target_arch = "wasm32"))]
        ffi_args,
        dyn_lib_args,
        keep_alive,
        error_handles,
        obj_pool,
        map_pool,
        str_pool,
        dyn_libs,
        structs,
        types,
        flush_threshold,
        instr_base,
        ..
    } = unsafe { &mut *ctx };

    macro_rules! string {
        ($e: expr) => {
            Data::string($e.into(), obj_pool, map_pool, str_pool, stack.roots(r), gc)
        };
    }

    macro_rules! concat_string {
        ($s1: expr, $s2: expr) => {{
            let s1: Data = $s1;
            let s2: Data = $s2;
            let s1_str = s1.as_str(str_pool);
            let s1_len = s1_str.len();
            let s2_str = s2.as_str(str_pool);
            let s2_len = s2_str.len();
            let len = s1_len + s2_len;
            if len <= 6 {
                Data(s1.0 | ((s2.0 & crate::data::PAYLOAD_MASK) << (8 * s1_len)))
            } else {
                let id = gc.alloc_string_slot(obj_pool, map_pool, str_pool, stack.roots(r));
                let slot = str_pool[id as usize].as_mut_vec();
                if slot.capacity() < len {
                    *slot = Vec::with_capacity(len.next_power_of_two().max(16));
                }
                let buf = slot.as_mut_ptr();
                core::ptr::copy_nonoverlapping(s1_str.as_ptr(), buf, s1_len);
                core::ptr::copy_nonoverlapping(s2_str.as_ptr(), buf.add(s1_len), s2_len);
                slot.set_len(len);
                Data::large_str_id(u64::from(id))
            }
        }};
    }

    macro_rules! error_with_catch {
        ($err:expr) => {
            cold_path();
            (ip, r) = error_with_catch(&$err, ip, ctx);
            become execute_at(ip, r, ctx);
        };
    }

    macro_rules! jump {
        ($jump_size: expr) => {{
            ip = jump(ip, $jump_size);
            become execute_at(ip, r, ctx);
        }};
        ($condition: expr, $jump_size: expr) => {
            if $condition {
                jump!($jump_size);
            }
        };
    }

    unsafe {
        assert_unchecked((*ip).discriminant() == DISCRIMINANT);
        match *ip {
            Instr::Jmp(size) => jump!(size),
            Instr::Mov(tgt, dest) => r[dest] = r[tgt],
            Instr::SetInt(dest, n) => r[dest] = Data::int(n),
            Instr::CallFunc { target, frame_offset } => {
                let new_frame = Frame(r.0.add(frame_offset as usize));
                if new_frame.0 > stack.frame_limit.0 {
                    error_with_catch!(ErrType::StackOverflow);
                }
                r = new_frame;
                // The return point is always placed in the target function's first register
                r[0] = Data::return_point(ip.offset_from(*instr_base) as u32, frame_offset);
                ip = instr_base.add(target as usize);
                become execute_at(ip, r, ctx);
            }
            Instr::Return(src) => {
                let (return_addr, frame_offset) = r[0].as_return_point();
                r[0] = r[src];
                r = Frame(r.0.sub(frame_offset as usize));
                ip = instr_base.add(return_addr as usize);
            }
            Instr::CopyConstants { count_src, dst } => {
                let count = count_src >> 24;
                let src = stack.ptr.add((count_src & 0xFF_FFFF) as usize);
                let dst = r.0.add(dst as usize);
                // Copies a fixed 8 slots to avoid memcpy most of the time
                core::ptr::copy_nonoverlapping(src, dst, 8);
                if count > 8 {
                    cold_path();
                    core::ptr::copy_nonoverlapping(src.add(8), dst.add(8), count as usize - 8);
                }
            }
            Instr::LoadGlobal { dest_reg, reg } => {
                r[dest_reg] = *stack.ptr.add(reg as usize);
            }
            Instr::StoreGlobal { src_reg, reg } => {
                *stack.ptr.add(reg as usize) = r[src_reg];
            }
            #[cfg(target_arch = "wasm32")]
            Instr::CallDynamicLibFunc { .. } => std::hint::unreachable_unchecked(),
            #[cfg(not(target_arch = "wasm32"))]
            Instr::CallDynamicLibFunc { fn_id, dst, first_arg } => {
                let func = dyn_libs.get_unchecked(fn_id as usize);
                let args_len = func.args_len;
                ffi_args.clear();
                ffi_args.reserve_exact(args_len);
                keep_alive.clear();

                dyn_lib_args.reserve_exact(args_len);
                let args_ptr = dyn_lib_args.as_mut_ptr();
                for idx in 0..args_len {
                    let data = r[first_arg + idx as u16];
                    let data_ffi = match func.get_argument_type(idx) {
                        VmType::Int => data.as_int() as u64,
                        VmType::Float => data.as_float().to_bits(),
                        VmType::Bool => u64::from(data.as_bool()),
                        VmType::String => {
                            if let Ok(b) = std::ffi::CString::new(data.as_str(str_pool)) {
                                let bytes = b.into_bytes_with_nul().into_boxed_slice();
                                let ptr = bytes.as_ptr() as u64;
                                keep_alive.push(bytes);
                                ptr
                            } else {
                                error_with_catch!(ErrType::NullByteInString);
                            }
                        }
                        VmType::Array => {
                            ffi::array_to_c_ptr(data, obj_pool, str_pool, keep_alive) as u64
                        }
                        _ => {
                            debug_assert!(data.is_struct());
                            // can only be a struct at this point
                            let b = ffi::keel_struct_to_c_struct(
                                data.as_struct(),
                                obj_pool,
                                str_pool,
                                keep_alive,
                            )
                            .into_boxed_slice();
                            let ptr = &raw const *b;
                            keep_alive.push(b);
                            ffi_args.push_unchecked(libffi::middle::Arg::new(&*ptr));
                            continue;
                        }
                    };
                    args_ptr.add(idx).write(data_ffi);
                    ffi_args.push_unchecked(libffi::middle::Arg::new(&*args_ptr.add(idx)));
                }

                // Call the function, and convert the result back into Data
                r[dst] = {
                    match func.get_return_type() {
                        VmType::Int => Data::int(func.cif.call::<i32>(func.ptr, ffi_args)),
                        VmType::Float => Data::float(func.cif.call::<f64>(func.ptr, ffi_args)),
                        VmType::String => {
                            let ptr = func.cif.call::<*const core::ffi::c_char>(func.ptr, ffi_args);
                            if ptr.is_null() {
                                cold_path();
                                NULL
                            } else {
                                string!(core::ffi::CStr::from_ptr(ptr).to_string_lossy())
                            }
                        }
                        VmType::Bool => func.cif.call::<bool>(func.ptr, ffi_args).into(),
                        VmType::Struct { struct_id } => {
                            let struct_fields = &structs.get_unchecked(struct_id as usize).fields;
                            let (struct_size, _, field_offsets) =
                                ffi::get_struct_size_datatype(struct_fields, structs);
                            let mut return_buf: Vec<u8> = vec![0; struct_size];
                            func.cif.call_return_into(
                                func.ptr,
                                ffi_args,
                                libffi::middle::ret(&mut return_buf[..]),
                            );

                            let new_struct_id =
                                gc.alloc_array(obj_pool, map_pool, str_pool, stack.roots(r));
                            let new_struct = Data::struct_instance(struct_id, new_struct_id);
                            gc.pin(new_struct);
                            ffi::c_struct_to_keel_struct(
                                &return_buf,
                                &field_offsets,
                                new_struct_id,
                                obj_pool,
                                map_pool,
                                str_pool,
                                struct_fields,
                                stack.roots(r),
                                gc,
                                structs,
                            );
                            gc.unpin();
                            new_struct
                        }
                        _ => {
                            func.cif.call::<usize>(func.ptr, ffi_args);
                            NULL
                        }
                    }
                };
            }
            Instr::AddFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() + r[o2].as_float()).into();
            }
            Instr::AddInt(o1, o2, dest) => {
                r[dest] = Data::int(r[o1].as_int().wrapping_add(r[o2].as_int()));
            }
            Instr::AddStr(o1, o2, dst) => r[dst] = concat_string!(r[o1], r[o2]),
            Instr::AddStrInPlace(dst, src) => {
                let src = r[src];
                let dst_str = r[dst];
                if dst_str.is_large_str() {
                    let dst_id = dst_str.get_str_pool_id();
                    if src.is_large_str() && src.get_str_pool_id() == dst_id {
                        str_pool[dst_id].as_mut_vec().extend_from_within(..);
                    } else {
                        let s = src.as_str(str_pool).as_bytes();
                        str_pool[dst_id].as_mut_vec().extend_from_slice(s);
                    }
                } else {
                    r[dst] = concat_string!(dst_str, src);
                }
            }
            Instr::EmptyArray(arr_reg_id) => {
                let array_id = gc.alloc_array(obj_pool, map_pool, str_pool, stack.roots(r));
                r[arr_reg_id] = Data::array(array_id);
            }
            Instr::CloneObj(src_reg, dest_reg, len) => {
                let src_id = r[src_reg].as_array();
                let new_id = gc.alloc_array(obj_pool, map_pool, str_pool, stack.roots(r)) as usize;
                let src_ptr = obj_pool[src_id].as_ptr();
                let dst = &mut obj_pool[new_id];
                dst.reserve_exact(len as usize);
                dst.set_len(len as usize);
                core::ptr::copy_nonoverlapping(src_ptr, dst.as_mut_ptr(), len as usize);

                r[dest_reg] = Data(r[src_reg].tag() | new_id as u64);
            }
            Instr::AddArray(o1, o2, dest) => {
                let array1_id = r[o1].as_array();
                let array2_id = r[o2].as_array();
                let output_array_id = gc.alloc_array(obj_pool, map_pool, str_pool, stack.roots(r));
                let array_idx = output_array_id as usize;
                let array_pool_ptr = obj_pool.as_mut_ptr();
                let arr_a = &*array_pool_ptr.add(array1_id);
                let arr_b = &*array_pool_ptr.add(array2_id);
                let combined = &mut *array_pool_ptr.add(array_idx);

                combined.reserve(arr_a.len() + arr_b.len());
                combined.extend_from_slice(arr_a);
                combined.extend_from_slice(arr_b);
                r[dest] = Data::array(output_array_id);
            }
            Instr::AddIntImm { src, dst, imm } => r[dst] = r[src].add_int_imm(imm as i16),
            Instr::MulFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() * r[o2].as_float()).into();
            }
            Instr::MulInt(o1, o2, dest) => {
                r[dest] = Data::int(r[o1].as_int().wrapping_mul(r[o2].as_int()));
            }
            Instr::DivFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() / r[o2].as_float()).into();
            }
            Instr::DivInt(o1, o2, dest) => {
                let b = r[o2].as_int();
                if b == 0 {
                    error_with_catch!(ErrType::DivisionByZero);
                }
                r[dest] = Data::int(r[o1].as_int().wrapping_div(b));
            }
            Instr::SubFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() - r[o2].as_float()).into();
            }
            Instr::SubInt(o1, o2, dest) => {
                r[dest] = Data::int(r[o1].as_int().wrapping_sub(r[o2].as_int()));
            }
            Instr::ModFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() % r[o2].as_float()).into();
            }
            Instr::ModInt(o1, o2, dest) => {
                let b = r[o2].as_int();
                if b == 0 {
                    error_with_catch!(ErrType::ModuloByZero);
                }
                r[dest] = Data::int(r[o1].as_int().wrapping_rem(b));
            }
            Instr::PowFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float().powf(r[o2].as_float())).into();
            }
            Instr::PowInt(o1, o2, dest) => {
                let e = r[o2].as_int();
                if e < 0 {
                    error_with_catch!(ErrType::IntegerNegativeExponent);
                }
                r[dest] = Data::int(r[o1].as_int().wrapping_pow(e as u32));
            }
            Instr::Eq(o1, o2, dest) => {
                r[dest] = (r[o1] == r[o2]).into();
            }
            Instr::ObjEq(o1, o2, dest) => {
                r[dest] = obj_eq(r[o1], r[o2], obj_pool, map_pool, str_pool).into();
            }
            Instr::NotEqJmp(o1, o2, jump_size) => jump!(r[o1] != r[o2], jump_size),
            Instr::ObjNotEqJmp(o1, o2, jump_size) => {
                jump!(!obj_eq(r[o1], r[o2], obj_pool, map_pool, str_pool), jump_size);
            }
            Instr::NotEq(o1, o2, dest) => {
                r[dest] = (r[o1] != r[o2]).into();
            }
            Instr::ObjNotEq(o1, o2, dest) => {
                r[dest] = (!obj_eq(r[o1], r[o2], obj_pool, map_pool, str_pool)).into();
            }
            Instr::EqJmp(o1, o2, jump_size) => jump!(r[o1] == r[o2], jump_size),
            Instr::ObjEqJmp(o1, o2, jump_size) => {
                jump!(obj_eq(r[o1], r[o2], obj_pool, map_pool, str_pool), jump_size);
            }
            Instr::InfEqFloatJmp(o1, o2, jump_size) => {
                jump!(r[o1].as_float() <= r[o2].as_float(), jump_size);
            }
            Instr::InfEqIntJmp(o1, o2, jump_size) => {
                jump!(r[o1].as_int() <= r[o2].as_int(), jump_size);
            }
            Instr::InfFloatJmp(o1, o2, jump_size) => {
                jump!(r[o1].as_float() < r[o2].as_float(), jump_size);
            }
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            Instr::NotInfFloatJmp(o1, o2, jump_size) => {
                jump!(!(r[o1].as_float() < r[o2].as_float()), jump_size);
            }
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            Instr::NotInfEqFloatJmp(o1, o2, jump_size) => {
                jump!(!(r[o1].as_float() <= r[o2].as_float()), jump_size);
            }
            Instr::InfIntJmp(o1, o2, jump_size) => {
                if r[o1].as_int() < r[o2].as_int() {
                    ip = jump(ip, jump_size);
                    become execute_at(ip, r, ctx);
                }
            }
            Instr::IncIntInfJmp { counter_reg, limit_reg, jump_size } => {
                let next = r[counter_reg].add_int_imm(1);
                r[counter_reg] = next;
                jump!(next.as_int() < r[limit_reg].as_int(), jump_size);
            }
            Instr::EqIntImmJmp(src, imm, jump_size) => {
                jump!(r[src].as_int() == i32::from(imm as i16), jump_size);
            }
            Instr::NotEqIntImmJmp(src, imm, jump_size) => {
                jump!(r[src].as_int() != i32::from(imm as i16), jump_size);
            }
            Instr::InfIntImmJmp(src, imm, jump_size) => {
                jump!(r[src].as_int() < i32::from(imm as i16), jump_size);
            }
            Instr::SupEqIntImmJmp(src, imm, jump_size) => {
                jump!(r[src].as_int() >= i32::from(imm as i16), jump_size);
            }
            Instr::InfFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() < r[o2].as_float()).into();
            }
            Instr::InfInt(o1, o2, dest) => {
                r[dest] = (r[o1].as_int() < r[o2].as_int()).into();
            }
            Instr::InfEqFloat(o1, o2, dest) => {
                r[dest] = (r[o1].as_float() <= r[o2].as_float()).into();
            }
            Instr::InfEqInt(o1, o2, dest) => {
                r[dest] = (r[o1].as_int() <= r[o2].as_int()).into();
            }
            Instr::BoolAnd(o1, o2, dest) => {
                debug_assert!(r[o1].is_bool() && r[o2].is_bool());
                r[dest] = Data(r[o1].0 & r[o2].0);
            }
            Instr::BoolOr(o1, o2, dest) => {
                debug_assert!(r[o1].is_bool() && r[o2].is_bool());
                r[dest] = Data(r[o1].0 | r[o2].0);
            }
            Instr::Print(tgt) => {
                let value = r[tgt];
                if !value.format_scalar_to(output, str_pool, true) {
                    value.format_to(output, obj_pool, str_pool, map_pool, structs, false);
                    output.push('\n');
                }
                if output.len() >= *flush_threshold {
                    flush_output(output);
                }
            }
            Instr::SetElementObj(array_reg_id, new_elem_reg_id, idx) => {
                let array = &mut obj_pool[r[array_reg_id].as_array()];
                let index = r[idx].as_int();
                if (index as usize) >= array.len() {
                    error_with_catch!(ErrType::IndexOutOfBounds(array.len(), index));
                }
                *array.get_unchecked_mut(index as usize) = r[new_elem_reg_id];
            }
            Instr::SetElementString(string_reg_id, new_str_reg_id, idx) => {
                let index = r[idx].as_int();
                let source_string = r[string_reg_id].as_str(str_pool);
                if (index as usize) >= source_string.len() {
                    error_with_catch!(ErrType::IndexOutOfBounds(source_string.len(), index));
                }
                let mut temp: String = source_string.to_owned();
                temp.replace_range(
                    index as usize..=index as usize,
                    r[new_str_reg_id].as_str(str_pool),
                );
                r[string_reg_id] = string!(temp);
            }
            Instr::SetFieldObj(struct_reg_id, new_elem_reg_id, idx) => {
                let s = &mut obj_pool[r[struct_reg_id].as_array()];
                *s.get_unchecked_mut(idx as usize) = r[new_elem_reg_id];
            }
            Instr::GetIndexArray(array_reg_id, index, dest) => {
                let idx = r[index].as_int();
                let array = &obj_pool[r[array_reg_id].as_array()];
                if (idx as usize) >= array.len() {
                    error_with_catch!(ErrType::IndexOutOfBounds(array.len(), idx));
                }
                r[dest] = *array.get_unchecked(idx as usize);
            }
            Instr::GetNestedIndexArray(..) => {
                let word = ip.cast::<u64>().read_unaligned() >> 16;
                let idx = r[(word >> 12 & 0xFFF) as u16].as_int();
                let inner_idx = r[(word >> 24 & 0xFFF) as u16].as_int();
                let outer = &obj_pool[r[(word & 0xFFF) as u16].as_array()];
                if (idx as usize) >= outer.len() {
                    error_with_catch!(ErrType::IndexOutOfBounds(outer.len(), idx));
                }
                let inner = &obj_pool[outer.get_unchecked(idx as usize).as_array()];
                if (inner_idx as usize) >= inner.len() {
                    error_with_catch!(ErrType::IndexOutOfBounds(inner.len(), inner_idx));
                }
                r[(word >> 36) as u16] = *inner.get_unchecked(inner_idx as usize);
            }
            Instr::GetFieldStruct(struct_reg_id, index, dest) => {
                let struct_id = r[struct_reg_id].as_struct();
                let s = &obj_pool[struct_id];
                r[dest] = *s.get_unchecked(index as usize);
            }
            Instr::GetSliceArray(array_reg_id, idx_start_id, dest_reg_id) => {
                let idx_start = r[idx_start_id].as_int();
                let idx_end = r[idx_start_id + 1].as_int();
                let arr_id = r[array_reg_id].as_array();
                let array = &obj_pool[arr_id];
                if (idx_end as usize) > array.len()
                    || (idx_start as usize) > array.len()
                    || idx_start > idx_end
                {
                    error_with_catch!(ErrType::SliceOutOfBounds(array.len(), idx_start, idx_end));
                }
                let new_array_id = gc.alloc_array(obj_pool, map_pool, str_pool, stack.roots(r));
                obj_pool.extend_within(
                    arr_id,
                    new_array_id as usize,
                    idx_start as usize,
                    idx_end as usize,
                );
                r[dest_reg_id] = Data::array(new_array_id);
            }
            // Keel currently indexes strings by byte, meaning multi-byte characters won't get properly indexed
            Instr::GetIndexString(tgt, index, dest) => {
                let idx = r[index].as_int();
                let tgt_data = r[tgt];
                let bytes = tgt_data.as_str(str_pool).as_bytes();
                if (idx as usize) >= bytes.len() {
                    error_with_catch!(ErrType::IndexOutOfBounds(bytes.len(), idx));
                }
                r[dest] = Data::small_str(core::str::from_utf8_unchecked(core::slice::from_ref(
                    bytes.get_unchecked(idx as usize),
                )));
            }
            Instr::GetSliceString(str_reg_id, idx_start, dest_reg_id) => {
                let idx_end = r[idx_start + 1].as_int();
                let idx_start = r[idx_start].as_int();
                let s = r[str_reg_id].as_str(str_pool);
                if (idx_end as usize) > s.len()
                    || (idx_start as usize) > s.len()
                    || idx_start > idx_end
                {
                    error_with_catch!(ErrType::SliceOutOfBounds(s.len(), idx_start, idx_end));
                }
                r[dest_reg_id] = string!(s.get_unchecked((idx_start as usize)..(idx_end as usize)));
            }
            Instr::Push(array, element) => {
                obj_pool[r[array].as_array()].push(r[element]);
            }
            Instr::Remove(array, idx) => {
                let arr = &mut obj_pool[r[array].as_array()];
                let index = r[idx].as_int();
                // This also catches index < 0, if it's negative it will wrap when cast to usize
                if (index as usize) >= arr.len() {
                    error_with_catch!(ErrType::IndexOutOfBounds(arr.len(), index));
                }
                arr.remove(index as usize);
            }
            Instr::MapGet(map_reg_id, key_reg_id, dest_reg_id) => {
                r[dest_reg_id] = if let Some(elem) =
                    map_pool[r[map_reg_id].as_map()].get(&r[key_reg_id])
                {
                    *elem
                } else {
                    error_with_catch!(ErrType::UnknownMapKey(
                        r[key_reg_id].format(obj_pool, str_pool, map_pool, structs, false).as_str()
                    ));
                };
            }
            Instr::MapInsertReg(map_reg_id, key_reg_id, val_reg_id) => {
                map_pool[r[map_reg_id].as_map()].insert(r[key_reg_id], r[val_reg_id]);
            }
            Instr::CloneMap(src_reg, dest_reg) => {
                let new_id = gc.alloc_map(obj_pool, map_pool, str_pool, stack.roots(r));
                let ptr = map_pool.as_mut_ptr();
                (*ptr.add(new_id as usize)).clone_from(&*ptr.add(r[src_reg].as_map()));
                r[dest_reg] = Data::map(new_id);
            }
            Instr::MapRemove { map_reg_id, key_reg_id, dest_reg_id } => {
                r[dest_reg_id] =
                    Data::bool(map_pool[r[map_reg_id].as_map()].remove(&r[key_reg_id]).is_some());
            }
            Instr::Uppercase(source_string_reg_id, dest_reg_id) => {
                r[dest_reg_id] = string!(r[source_string_reg_id].as_str(str_pool).to_uppercase());
            }
            Instr::Lowercase(source_string_reg_id, dest_reg_id) => {
                r[dest_reg_id] = string!(r[source_string_reg_id].as_str(str_pool).to_lowercase());
            }
            Instr::Contains { src, needle, dst } => {
                let collection = r[src];
                let needle = r[needle];
                r[dst] = Data::bool(if collection.is_string() {
                    memmem::find(
                        collection.as_str(str_pool).as_bytes(),
                        needle.as_str(str_pool).as_bytes(),
                    )
                    .is_some()
                } else {
                    debug_assert!(collection.is_array());
                    let array = &obj_pool[collection.as_array()];
                    if needle.is_heap() {
                        get_heap_array_position(array, needle, obj_pool, map_pool, str_pool)
                            .is_some()
                    } else {
                        array.contains(&needle)
                    }
                });
            }
            Instr::Trim(tgt, dest) => {
                r[dest] = string!(r[tgt].as_str(str_pool).trim());
            }
            Instr::TrimSequence { src, seq, dst } => {
                let arg = r[seq].as_str(str_pool);
                r[dst] = string!(r[src].as_str(str_pool).trim_matches(|c| arg.contains(c)));
            }
            Instr::Find { src, needle, dst } => {
                let collection = r[src];
                let element = r[needle];
                r[dst] = {
                    if let Some(idx) = if collection.is_string() {
                        memmem::find(
                            collection.as_str(str_pool).as_bytes(),
                            element.as_str(str_pool).as_bytes(),
                        )
                    } else {
                        get_array_position(
                            &obj_pool[collection.as_array()],
                            element,
                            obj_pool,
                            map_pool,
                            str_pool,
                        )
                    } {
                        Data::int(idx as i32)
                    } else {
                        cold_path();
                        Data::int(-1)
                    }
                }
            }
            Instr::IsFloat(tgt, dest) => {
                let num = r[tgt].as_str(str_pool);
                r[dest] = (num.parse::<i64>().is_err() && num.parse::<f64>().is_ok()).into();
            }
            Instr::IsInt(tgt, dest) => {
                r[dest] = r[tgt].as_str(str_pool).parse::<i64>().is_ok().into();
            }
            Instr::TrimLeft(tgt, dest) => {
                r[dest] = string!(r[tgt].as_str(str_pool).trim_start());
            }
            Instr::TrimRight(tgt, dest) => {
                r[dest] = string!(r[tgt].as_str(str_pool).trim_end());
            }
            Instr::TrimSequenceLeft { src, seq, dst } => {
                let arg = r[seq].as_str(str_pool);
                r[dst] = string!(r[src].as_str(str_pool).trim_start_matches(|c| arg.contains(c)));
            }
            Instr::TrimSequenceRight { src, seq, dst } => {
                let arg = r[seq].as_str(str_pool);
                r[dst] = string!(r[src].as_str(str_pool).trim_end_matches(|c| arg.contains(c)));
            }
            Instr::Repeat { src, count, dst } => {
                let reg = r[src];
                let repeat_count = r[count].as_int().max(0) as usize;
                r[dst] = if reg.is_string() {
                    string!(reg.as_str(str_pool).repeat(repeat_count))
                } else {
                    let array_id = gc.alloc_array(obj_pool, map_pool, str_pool, stack.roots(r));
                    obj_pool.repeat_within(reg.as_array(), array_id as usize, repeat_count);
                    Data::array(array_id)
                };
            }
            Instr::Round(tgt, dest) => {
                r[dest] = r[tgt].as_float().round().into();
            }
            Instr::Abs(tgt, dest) => {
                let tgt = r[tgt];
                r[dest] = if tgt.is_float() {
                    tgt.as_float().abs().into()
                } else {
                    Data::int(tgt.as_int().abs())
                }
            }
            Instr::ReverseString(tgt, dest) => {
                r[dest] = string!(r[tgt].as_str(str_pool).chars().rev().collect::<String>());
            }
            Instr::ReverseArray(src) => {
                obj_pool[r[src].as_array()].reverse();
            }
            Instr::SqrtFloat(tgt, dest) => {
                r[dest] = r[tgt].as_float().sqrt().into();
            }
            Instr::Float(tgt, dest) => {
                let reg = r[tgt];
                r[dest] = Data::float(if reg.is_int() {
                    f64::from(reg.as_int())
                } else if let Some(f) = parse_float(reg.as_str(str_pool)) {
                    f
                } else {
                    error_with_catch!(ErrType::InvalidFloat);
                });
            }
            Instr::Int(tgt, dest) => {
                let reg = r[tgt];
                r[dest] = Data::int(if reg.is_float() {
                    reg.as_float() as i32
                } else if let Ok(i) = lexical_core::parse::<i32>(reg.as_str(str_pool).as_bytes()) {
                    i
                } else {
                    error_with_catch!(ErrType::InvalidInt);
                });
            }
            Instr::Str(tgt, dest) => {
                let value = r[tgt];
                r[dest] = if value.is_int() {
                    let mut buffer = [0u8; i32::FORMATTED_SIZE_DECIMAL];
                    let digits = lexical_core::write(value.as_int(), &mut buffer);
                    string!(str::from_utf8_unchecked(digits))
                } else if value.is_float() {
                    string!(zmij::Buffer::new().format(value.as_float()))
                } else if value.is_bool() {
                    Data::small_str(if value.as_bool() { "true" } else { "false" })
                } else {
                    string!(value.format(obj_pool, str_pool, map_pool, structs, false))
                };
            }
            Instr::Bool(tgt, dest) => {
                let str = r[tgt].as_str(str_pool);
                r[dest] = if str == "true" {
                    TRUE
                } else if str == "false" {
                    FALSE
                } else {
                    error_with_catch!(ErrType::InvalidBool);
                }
            }
            #[cfg(target_arch = "wasm32")]
            Instr::Input(_, _) => {
                flush_output(output);
                wasm_error("WASM does not support input()")
            }
            #[cfg(not(target_arch = "wasm32"))]
            Instr::Input(tgt, dest) => {
                output.push_str(r[tgt].as_str(str_pool));
                flush_output(output);
                let mut line = String::new();
                std::io::stdin().read_line(&mut line).unwrap();
                r[dest] = string!(line.trim_end_matches(['\n', '\r']));
            }
            Instr::Floor(tgt, dest) => {
                r[dest] = r[tgt].as_float().floor().into();
            }
            Instr::TheAnswer(dst) => {
                cold_path();
                output.push_str("The answer to the Ultimate Question of Life, the Universe, and Everything is 42.");
                flush_output(output);
                r[dst] = Data::int(42);
            }
            Instr::Len(tgt, dest) => {
                let reg = r[tgt];
                r[dest] = Data::int(if reg.is_array() {
                    obj_pool[reg.as_array()].len()
                } else if reg.is_string() {
                    reg.as_str(str_pool).len()
                } else {
                    // can only be a map at this point
                    map_pool[reg.as_map()].len()
                } as i32);
            }
            Instr::StartsWith { src, prefix, dst } => {
                r[dst] = r[src].as_str(str_pool).starts_with(r[prefix].as_str(str_pool)).into();
            }
            Instr::EndsWith { src, suffix, dst } => {
                r[dst] = r[src].as_str(str_pool).ends_with(r[suffix].as_str(str_pool)).into();
            }
            #[allow(clippy::no_effect_replace)]
            Instr::Replace(src, old, dst) => {
                r[dst] = string!(
                    r[src]
                        .as_str(str_pool)
                        .replace(r[old].as_str(str_pool), r[old + 1].as_str(str_pool))
                );
            }
            Instr::Split { src, sep, dst } => {
                let source = r[src];
                let separator = r[sep];
                let array_id = gc.alloc_array(obj_pool, map_pool, str_pool, stack.roots(r));
                gc.pin(Data::array(array_id));
                let mut start_idx = 0;
                if source.is_string() {
                    let source = source.as_str(str_pool);
                    let separator = separator.as_str(str_pool);
                    let separator_len = separator.len();
                    for part_i in memmem::find_iter(source.as_bytes(), separator.as_bytes()) {
                        let part = string!(source.get_unchecked(start_idx..part_i));
                        obj_pool[array_id as usize].push(part);
                        start_idx = part_i + separator_len;
                    }
                    let part = string!(source.get_unchecked(start_idx..));
                    obj_pool[array_id as usize].push(part);
                } else {
                    // it can only be an array in this case
                    let source_array_id = source.as_array();
                    let len = obj_pool[source_array_id].len();

                    for idx in 0..=len {
                        if idx == len || *obj_pool[source_array_id].get_unchecked(idx) == separator
                        {
                            let dest_array_id =
                                gc.alloc_array(obj_pool, map_pool, str_pool, stack.roots(r))
                                    as usize;
                            obj_pool.extend_within(source_array_id, dest_array_id, start_idx, idx);
                            obj_pool[array_id as usize].push(Data::array(dest_array_id as u32));
                            start_idx = idx + 1;
                        }
                    }
                }
                gc.unpin();
                r[dst] = Data::array(array_id);
            }
            Instr::Range(min, max, dest) => {
                let min = r[min].as_int();
                let max = r[max].as_int();
                let output_array_id = gc.alloc_array(obj_pool, map_pool, str_pool, stack.roots(r));
                obj_pool[output_array_id as usize].extend((min..max).map(Data::int));
                r[dest] = Data::array(output_array_id);
            }
            Instr::JoinStringArray(tgt, sep, dest) => {
                let separator = r[sep].as_str(str_pool);
                let array = &obj_pool[r[tgt].as_array()];
                let total_len: usize =
                    array.iter().map(|x| x.as_str(str_pool).len()).sum::<usize>()
                        + separator.len().saturating_mul(array.len().saturating_sub(1));
                let mut output = String::with_capacity(total_len);
                for (i, x) in array.iter().enumerate() {
                    if i > 0 {
                        output.push_str(separator);
                    }
                    output.push_str(x.as_str(str_pool));
                }
                r[dest] = string!(output);
            }
            // -----
            // FILE SYSTEM FUNCTIONS
            // -----
            Instr::FsRead(path, dest_reg_id) => {
                r[dest_reg_id] = string!(match fs::read_to_string(r[path].as_str(str_pool)) {
                    Ok(p) => p,
                    Err(e) => {
                        error_with_catch!(ErrType::from(e.kind()));
                    }
                });
            }
            Instr::FsExists(path, dest_reg_id) => {
                r[dest_reg_id] = match fs::exists(r[path].as_str(str_pool)) {
                    Ok(b) => b.into(),
                    Err(e) => {
                        error_with_catch!(ErrType::from(e.kind()));
                    }
                }
            }
            // Overwrites a file, will create it if it doesn't exist
            Instr::FsWrite(path, contents) => {
                if let Err(e) = fs::write(r[path].as_str(str_pool), r[contents].as_str(str_pool)) {
                    error_with_catch!(ErrType::from(e.kind()));
                }
            }
            // Appends to a file, will create if it doesn't exist
            Instr::FsAppend(path, contents) => {
                match fs::OpenOptions::new().append(true).open(r[path].as_str(str_pool)) {
                    Ok(mut f) => {
                        if let Err(e) = f.write_all(r[contents].as_str(str_pool).as_bytes()) {
                            error_with_catch!(ErrType::from(e.kind()));
                        }
                    }
                    Err(e) => {
                        error_with_catch!(ErrType::from(e.kind()));
                    }
                }
            }
            // Deletes the file located at `path`, throwing an error if it doesn't exist.
            Instr::FsDelete(path) => {
                if let Err(e) = fs::remove_file(r[path].as_str(str_pool)) {
                    error_with_catch!(ErrType::from(e.kind()));
                }
            }
            // Deletes the empty directory located at `path`
            Instr::FsDeleteDir(path) => {
                if let Err(e) = fs::remove_dir(r[path].as_str(str_pool)) {
                    error_with_catch!(ErrType::from(e.kind()));
                }
            }
            #[cfg(target_arch = "wasm32")]
            Instr::Argv(dst) => {
                r[dst] = Data::array(gc.alloc_array(obj_pool, map_pool, str_pool, stack.roots(r)))
            }
            #[cfg(not(target_arch = "wasm32"))]
            Instr::Argv(dst) => {
                let array_id = gc.alloc_array(obj_pool, map_pool, str_pool, stack.roots(r));
                gc.pin(Data::array(array_id));
                for arg in std::env::args().skip(1) {
                    let arg_str = string!(arg);
                    obj_pool[array_id as usize].push(arg_str);
                }
                gc.unpin();
                r[dst] = Data::array(array_id);
            }
            Instr::Sort(src) => {
                let array = &mut obj_pool[r[src].as_array()];
                if !array.is_empty() {
                    'sort: {
                        let first_elem = array.get_unchecked(0);
                        let t: u8 = if first_elem.is_int() {
                            0
                        } else if first_elem.is_float() {
                            1
                        } else if first_elem.is_string() {
                            2
                        } else {
                            break 'sort;
                        };
                        array.sort_unstable_by(|a, b| {
                            if t == 0 {
                                a.as_int().cmp(&b.as_int())
                            } else if t == 1 {
                                a.as_float().total_cmp(&b.as_float())
                            } else {
                                a.as_str(str_pool).cmp(b.as_str(str_pool))
                            }
                        });
                    }
                }
            }
            Instr::StartErrorCatch(jmp_size, err_reg_id) => {
                error_handles.push(ErrorCatch {
                    catch_loc: ip.offset_from(*instr_base) as u32 + u32::from(jmp_size),
                    frame_ptr: r.0,
                    error_reg: err_reg_id,
                });
            }
            Instr::StopErrorCatch => {
                error_handles.pop_unchecked();
            }
            Instr::ThrowError(error_reg_id) => {
                let error_data = r[error_reg_id];
                error_with_catch!(ErrType::Custom(error_data.as_str(str_pool)));
            }
            Instr::IsNotTypeJmp { src_reg_id, type_idx, jump_size } => {
                jump!(
                    !r[src_reg_id].matches_datatype(
                        types.get_unchecked(type_idx as usize),
                        obj_pool,
                        map_pool,
                    ),
                    jump_size
                );
            }
            Instr::IsType(src, type_idx, dest) => {
                r[dest] = r[src]
                    .matches_datatype(types.get_unchecked(type_idx as usize), obj_pool, map_pool)
                    .into();
            }
            Instr::Halt(code) => {
                cold_path();

                flush_output(output);

                #[cfg(not(target_arch = "wasm32"))]
                if code != u16::MAX {
                    std::process::exit(r[code].as_int());
                }

                return;
            }
        }
        become execute_at(ip.add(1), r, ctx);
    }
}
