use crate::Data;
use crate::DynamicLibFn;
use crate::FALSE;
use crate::Instr;
use crate::LibFunc;
use crate::NULL;
use crate::alloc_array;
use crate::data::TRUE;
use crate::errors::ErrType;
use crate::errors::ErrorCtx;
use crate::errors::throw_error;
use crate::format_data;
use crate::fs;
use crate::instr::LibFuncVoid;
use crate::parser_data::Pools;
use crate::string_gc::raise_string_gc_threshold;
use crate::type_system::DataType;
use smol_str::ToSmolStr;
use std::hint::cold_path;
use std::io::Write;

pub type ArrayPool = Vec<Vec<Data>>;
pub type StringPool = Vec<String>;

/// Converts a Keel array to a C pointer for libffi
fn array_to_c_ptr(
    data: Data,
    elem_type: &DataType,
    array_pool: &ArrayPool,
    string_pool: &StringPool,
    // boxed so the address doesn't move
    keep_alive: &mut Vec<Box<[u8]>>,
) -> u64 {
    let elems = unsafe { array_pool.get_unchecked(data.as_array()) };

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
                ptrs.push(array_to_c_ptr(*e, inner, array_pool, string_pool, keep_alive) as usize);
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

struct CallFrame {
    return_addr: u32,
    return_reg: u16,
    callsite_id: u16,
}

#[allow(unused_unsafe)]
pub fn execute(
    instructions: &[Instr],
    registers: &mut [Data],
    Pools {
        array_pool,
        string_pool,
    }: &mut Pools,
    err_ctx: &ErrorCtx,
    fn_registers: &[Vec<u16>],
    dyn_libs: &[DynamicLibFn],
    allocated_arg_count: usize,
    allocated_call_depth: usize,
) {
    let mut i: usize = 0;

    let mut args: Vec<u16> = Vec::with_capacity(allocated_arg_count);
    let mut call_frames: Vec<CallFrame> = Vec::with_capacity(allocated_call_depth);
    let mut recursion_stack: Vec<Data> = Vec::with_capacity(allocated_call_depth * registers.len());

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();

    let mut free_arrays: Vec<u32> = Vec::with_capacity(array_pool.len());
    let mut free_strings: Vec<u16> = Vec::with_capacity(string_pool.len());
    let mut array_live: Vec<bool> = Vec::new();
    let mut string_live: Vec<bool> = Vec::new();

    let mut dyn_lib_args: Vec<u64> = Vec::new();
    let mut keep_alive: Vec<Box<[u8]>> = Vec::new();
    let mut array_gc_stack: Vec<usize> = Vec::new();

    let mut gc_string_threshold: u32 = 256;
    let mut gc_array_threshold: u32 = 256;

    macro_rules! str {
        ($e: expr) => {
            Data::str(
                $e,
                array_pool,
                string_pool,
                registers,
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
                array_pool,
                string_pool,
                registers,
                &recursion_stack,
                &mut free_strings,
                &mut gc_string_threshold,
                &mut string_live,
            )
        };
    }

    macro_rules! r {
        ($i:expr) => {
            unsafe { *registers.get_unchecked($i as usize) }
        };
    }
    macro_rules! w {
        ($i:expr) => {
            unsafe { registers.get_unchecked_mut($i as usize) }
        };
    }

    loop {
        match unsafe { *instructions.get_unchecked(i) } {
            Instr::Jmp(size) => {
                i += size as usize;
                continue;
            }
            Instr::JmpBack(size) => {
                i -= size as usize;
                continue;
            }
            Instr::Mov(tgt, dest) => *w!(dest) = r!(tgt),
            Instr::SetInt(dest, n) => *w!(dest) = n.into(),
            Instr::SetBool(dest, b) => *w!(dest) = b.into(),
            Instr::CallFunc(new_loc, return_id) => {
                call_frames.push(CallFrame {
                    return_addr: i as u32,
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
                i = unsafe {
                    let new_len = call_frames.len() - 1;
                    let ptr = call_frames.as_mut_ptr().add(new_len);
                    call_frames.set_len(new_len);
                    ptr.read().return_addr as usize
                };
            }
            Instr::SaveFrame(relative_func_loc, return_register, callsite_id) => {
                call_frames.push(CallFrame {
                    return_addr: (i + relative_func_loc as usize) as u32,
                    return_reg: return_register,
                    callsite_id,
                });
                recursion_stack.extend(
                    unsafe { fn_registers.get_unchecked(callsite_id as usize) }
                        .iter()
                        .map(|&r| r!(r)),
                );
            }
            Instr::Return(tgt) => {
                // Pop the latest call frame, set the return value and jump back to the callsite
                let call_frame = unsafe {
                    let new_len = call_frames.len() - 1;
                    let ptr = call_frames.as_mut_ptr().add(new_len);
                    call_frames.set_len(new_len);
                    ptr.read()
                };
                i = call_frame.return_addr as usize;
                *w!(call_frame.return_reg) = r!(tgt);
            }
            Instr::RecursiveReturn(tgt) => {
                let call_frame = unsafe {
                    let new_len = call_frames.len() - 1;
                    let ptr = call_frames.as_mut_ptr().add(new_len);
                    call_frames.set_len(new_len);
                    ptr.read()
                };
                let temp = r!(tgt);
                let regs = unsafe { fn_registers.get_unchecked(call_frame.callsite_id as usize) };
                let base = recursion_stack.len() - regs.len();
                for (reg, &saved) in regs
                    .iter()
                    .zip(unsafe { recursion_stack.get_unchecked(base..) })
                {
                    *w!(*reg) = saved;
                }
                unsafe {
                    recursion_stack.set_len(base);
                }
                i = call_frame.return_addr as usize;
                *w!(call_frame.return_reg) = temp;
            }
            Instr::IsFalseJmp(cond_id, size) => {
                if r!(cond_id) == FALSE {
                    i += size as usize;
                    continue;
                }
            }
            Instr::IsTrueJmp(cond_id, size) => {
                if r!(cond_id) == TRUE {
                    i += size as usize;
                    continue;
                }
            }
            Instr::CallDynamicLibFunc(fn_id, dest) => {
                let func = &dyn_libs[fn_id as usize];
                let args_len = args.len();
                dyn_lib_args.clear();
                keep_alive.clear();

                for (idx, &register_id) in args.iter().enumerate() {
                    let data = r!(register_id);
                    dyn_lib_args.push({
                        match func.get_nth_arg_type(idx) {
                            DataType::Int => data.as_int() as u64,
                            DataType::Float => data.as_float().to_bits(),
                            DataType::String => {
                                let bytes = std::ffi::CString::new(data.as_str(string_pool))
                                    .unwrap_or_else(|_| {
                                        cold_path();
                                        throw_error(
                                            err_ctx,
                                            &instructions[i],
                                            ErrType::Custom(
                                                "String passed to C contains an interior null byte"
                                                    .into(),
                                            ),
                                        )
                                    })
                                    .into_bytes_with_nul() // builds the null-terminated string
                                    .into_boxed_slice();
                                let ptr = bytes.as_ptr() as u64;
                                keep_alive.push(bytes);
                                ptr
                            }
                            DataType::Array(Some(inner)) => array_to_c_ptr(
                                data,
                                inner,
                                array_pool,
                                string_pool,
                                &mut keep_alive,
                            ),
                            t => {
                                cold_path();
                                throw_error(
                                    err_ctx,
                                    &instructions[i],
                                    ErrType::Custom(
                                        format_args!("Invalid argument type: {t:?}").to_smolstr(),
                                    ),
                                )
                            }
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
                *w!(dest) = unsafe {
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
                            cold_path();
                            throw_error(
                                err_ctx,
                                &instructions[i],
                                ErrType::Custom(
                                    "Array return types are not supported: C does not convey the length of a returned array".into(),
                                ),
                            )
                        }
                        t => {
                            cold_path();
                            throw_error(
                                err_ctx,
                                &instructions[i],
                                ErrType::Custom(
                                    format_args!("Invalid return type: {t:?}").to_smolstr(),
                                ),
                            )
                        }
                    }
                };
            }
            Instr::AddFloat(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_float() + r!(o2).as_float()).into();
            }
            Instr::AddInt(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_int() + r!(o2).as_int()).into();
            }
            Instr::AddStr(o1, o2, dest) => {
                let _d1 = r!(o1);
                let _d2 = r!(o2);
                let l = _d1.as_str(string_pool);
                let r = _d2.as_str(string_pool);
                let mut s = String::with_capacity(l.len() + r.len());
                s.push_str(l);
                s.push_str(r);
                *w!(dest) = string!(s);
            }
            Instr::EmptyArray(arr_reg_id) => {
                let array_id = alloc_array(
                    array_pool,
                    &mut free_arrays,
                    registers,
                    &recursion_stack,
                    &mut gc_array_threshold,
                    &mut array_live,
                    &mut array_gc_stack,
                );
                *w!(arr_reg_id) = Data::array(array_id);
            }
            Instr::CloneArray(src_reg, dest_reg, len) => {
                let src_id = r!(src_reg).as_array();
                let new_id = alloc_array(
                    array_pool,
                    &mut free_arrays,
                    registers,
                    &recursion_stack,
                    &mut gc_array_threshold,
                    &mut array_live,
                    &mut array_gc_stack,
                ) as usize;
                unsafe {
                    let src_ptr = array_pool.get_unchecked(src_id).as_ptr();
                    let dst = array_pool.get_unchecked_mut(new_id);
                    dst.reserve_exact(len as usize);
                    dst.set_len(len as usize);
                    std::ptr::copy_nonoverlapping(src_ptr, dst.as_mut_ptr(), len as usize);
                }
                *w!(dest_reg) = Data::array(new_id as u32);
            }
            Instr::AddArray(o1, o2, dest) => {
                let arr_a_id = r!(o1).as_array();
                let arr_b_id = r!(o2).as_array();
                let array_id = alloc_array(
                    array_pool,
                    &mut free_arrays,
                    registers,
                    &recursion_stack,
                    &mut gc_array_threshold,
                    &mut array_live,
                    &mut array_gc_stack,
                );
                let array_idx = array_id as usize;
                unsafe {
                    let array_pool_ptr = array_pool.as_mut_ptr();
                    let arr_a = &*array_pool_ptr.add(arr_a_id);
                    let arr_b = &*array_pool_ptr.add(arr_b_id);
                    let combined = &mut *array_pool_ptr.add(array_idx);

                    combined.reserve(arr_a.len() + arr_b.len());
                    combined.extend_from_slice(arr_a);
                    combined.extend_from_slice(arr_b);
                }
                *w!(dest) = Data::array(array_id);
            }
            Instr::MulFloat(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_float() * r!(o2).as_float()).into();
            }
            Instr::MulInt(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_int() * r!(o2).as_int()).into();
            }
            Instr::DivFloat(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_float() / r!(o2).as_float()).into();
            }
            Instr::DivInt(o1, o2, dest) => {
                let b = r!(o2).as_int();
                if b == 0 {
                    cold_path();
                    throw_error(
                        err_ctx,
                        &Instr::DivInt(o1, o2, dest),
                        ErrType::DivisionByZero,
                    );
                }
                *w!(dest) = (r!(o1).as_int() / b).into();
            }
            Instr::SubFloat(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_float() - r!(o2).as_float()).into();
            }
            Instr::SubInt(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_int() - r!(o2).as_int()).into();
            }
            Instr::ModFloat(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_float() % r!(o2).as_float()).into();
            }
            Instr::ModInt(o1, o2, dest) => {
                let b = r!(o2).as_int();
                if b == 0 {
                    cold_path();
                    throw_error(err_ctx, &Instr::ModInt(o1, o2, dest), ErrType::ModuloByZero);
                }
                *w!(dest) = (r!(o1).as_int() % b).into();
            }
            Instr::PowFloat(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_float().powf(r!(o2).as_float())).into();
            }
            Instr::PowInt(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_int().pow(r!(o2).as_int() as u32)).into();
            }
            Instr::IncInt(reg) => w!(reg).inc_int(),
            Instr::DecInt(reg) => w!(reg).dec_int(),
            Instr::IncIntTo(src, dst) => {
                let s = r!(src);
                w!(dst).inc_into(s);
            }
            Instr::DecIntTo(src, dst) => {
                let s = r!(src);
                w!(dst).dec_into(s);
            }
            Instr::Eq(o1, o2, dest) => {
                *w!(dest) = (r!(o1) == r!(o2)).into();
            }
            Instr::ArrayEq(o1, o2, dest) => {
                *w!(dest) = unsafe {
                    (array_pool.get_unchecked(r!(o1).as_array())
                        == array_pool.get_unchecked(r!(o2).as_array()))
                    .into()
                };
            }
            Instr::NotEqJmp(o1, o2, jump_size) => {
                if r!(o1) != r!(o2) {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::ArrayNotEqJmp(o1, o2, jump_size) => {
                if unsafe {
                    array_pool.get_unchecked(r!(o1).as_array())
                        != array_pool.get_unchecked(r!(o2).as_array())
                } {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::StrNotEqJmp(o1, o2, jump_size) => {
                if r!(o1).as_str(&string_pool) != r!(o2).as_str(&string_pool) {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::NotEq(o1, o2, dest) => {
                *w!(dest) = (r!(o1) != r!(o2)).into();
            }
            Instr::ArrayNotEq(o1, o2, dest) => {
                *w!(dest) = unsafe {
                    (array_pool.get_unchecked(r!(o1).as_array())
                        != array_pool.get_unchecked(r!(o2).as_array()))
                    .into()
                };
            }
            Instr::StrEq(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_str(&string_pool) == r!(o2).as_str(&string_pool)).into();
            }
            Instr::StrNotEq(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_str(&string_pool) != r!(o2).as_str(&string_pool)).into();
            }
            Instr::EqJmp(o1, o2, jump_size) => {
                if r!(o1) == r!(o2) {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::ArrayEqJmp(o1, o2, jump_size) => {
                if unsafe {
                    array_pool.get_unchecked(r!(o1).as_array())
                        == array_pool.get_unchecked(r!(o2).as_array())
                } {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::StrEqJmp(o1, o2, jump_size) => {
                if r!(o1).as_str(&string_pool) == r!(o2).as_str(&string_pool) {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::SupFloat(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_float() > r!(o2).as_float()).into();
            }
            Instr::SupInt(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_int() > r!(o2).as_int()).into();
            }
            Instr::InfEqFloatJmp(o1, o2, jump_size) => {
                if r!(o1).as_float() <= r!(o2).as_float() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::InfEqIntJmp(o1, o2, jump_size) => {
                if r!(o1).as_int() <= r!(o2).as_int() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::SupEqFloat(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_float() >= r!(o2).as_float()).into();
            }
            Instr::SupEqInt(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_int() >= r!(o2).as_int()).into();
            }
            Instr::InfFloatJmp(o1, o2, jump_size) => {
                if r!(o1).as_float() < r!(o2).as_float() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::InfIntJmp(o1, o2, jump_size) => {
                if r!(o1).as_int() < r!(o2).as_int() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::InfIntJmpBack(o1, o2, jump_size) => {
                if r!(o1).as_int() < r!(o2).as_int() {
                    i -= jump_size as usize;
                    continue;
                }
            }
            Instr::InfFloat(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_float() < r!(o2).as_float()).into();
            }
            Instr::InfInt(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_int() < r!(o2).as_int()).into();
            }
            Instr::SupEqFloatJmp(o1, o2, jump_size) => {
                if r!(o1).as_float() >= r!(o2).as_float() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::SupEqIntJmp(o1, o2, jump_size) => {
                if r!(o1).as_int() >= r!(o2).as_int() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::InfEqFloat(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_float() <= r!(o2).as_float()).into();
            }
            Instr::InfEqInt(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_int() <= r!(o2).as_int()).into();
            }
            Instr::SupFloatJmp(o1, o2, jump_size) => {
                if r!(o1).as_float() > r!(o2).as_float() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::SupIntJmp(o1, o2, jump_size) => {
                if r!(o1).as_int() > r!(o2).as_int() {
                    i += jump_size as usize;
                    continue;
                }
            }
            Instr::BoolAnd(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_bool() && r!(o2).as_bool()).into();
            }
            Instr::BoolOr(o1, o2, dest) => {
                *w!(dest) = (r!(o1).as_bool() || r!(o2).as_bool()).into();
            }
            Instr::NegBool(src, dest) => {
                *w!(dest) = (!r!(src).as_bool()).into();
            }
            Instr::NegFloat(tgt, dest) => {
                *w!(dest) = (-r!(tgt).as_float()).into();
            }
            Instr::NegInt(tgt, dest) => {
                *w!(dest) = (-r!(tgt).as_int()).into();
            }
            Instr::Print(target) => {
                let tgt = r!(target);
                if tgt.is_str() {
                    writeln!(handle, "{}", tgt.as_str(string_pool)).unwrap();
                } else if tgt.is_int() {
                    writeln!(handle, "{}", tgt.as_int()).unwrap();
                } else if tgt.is_float() {
                    writeln!(handle, "{}", tgt.as_float()).unwrap();
                } else if tgt.is_bool() {
                    writeln!(handle, "{}", tgt.as_bool()).unwrap();
                } else if tgt.is_array() {
                    let array = unsafe { array_pool.get_unchecked(tgt.as_array()) };
                    write!(handle, "[").unwrap();
                    for (idx, item) in array.iter().enumerate() {
                        if idx != 0 {
                            write!(handle, ",").unwrap();
                        }
                        write!(
                            handle,
                            "{}",
                            format_data(item, array_pool, string_pool, false)
                        )
                        .unwrap();
                    }
                    writeln!(handle, "]").unwrap();
                }
            }
            Instr::StoreFuncArg(id) => args.push(id),
            Instr::ArrayElemMov(new_elem_reg_id, array_id, idx) => unsafe {
                let arr = array_pool.get_unchecked_mut(array_id as usize);
                *arr.get_unchecked_mut(idx as usize) = r!(new_elem_reg_id);
            },
            Instr::SetElementArray(array_reg_id, new_elem_reg_id, idx) => {
                let array = unsafe { array_pool.get_unchecked_mut(r!(array_reg_id).as_array()) };
                let index = r!(idx).as_int() as usize;
                if index >= array.len() {
                    cold_path();
                    throw_error(
                        err_ctx,
                        &instructions[i],
                        ErrType::IndexOutOfBounds(array.len(), index),
                    );
                }
                array[index] = r!(new_elem_reg_id);
            }
            Instr::SetElementString(string_reg_id, new_str_reg_id, idx) => {
                let index = r!(idx).as_int() as usize;
                let temp_str_reg_id = r!(string_reg_id);
                let source_string = temp_str_reg_id.as_str(string_pool);
                if index >= source_string.len() {
                    cold_path();
                    throw_error(
                        err_ctx,
                        &instructions[i],
                        ErrType::IndexOutOfBounds(source_string.len(), index),
                    );
                }
                let mut temp = source_string.to_string();
                temp.remove(index);
                temp.insert_str(index, r!(new_str_reg_id).as_str(string_pool));
                *w!(string_reg_id) = string!(temp);
            }
            // takes tgt from  registers, index is index, dest is registers index destination
            Instr::GetIndexArray(array_reg_id, index, dest) => {
                let idx = r!(index).as_int() as usize;
                let arr_id = r!(array_reg_id).as_array();
                let array = unsafe { array_pool.get_unchecked(arr_id) };
                if idx >= array.len() {
                    cold_path();
                    throw_error(
                        err_ctx,
                        &instructions[i],
                        ErrType::IndexOutOfBounds(array.len(), idx),
                    );
                }
                *w!(dest) = unsafe { *array.get_unchecked(idx) };
            }
            // Keel currently indexes strings by byte, meaning multi-byte characters won't get properly indexed
            Instr::GetIndexString(tgt, index, dest) => {
                let idx = r!(index).as_int() as usize;
                let tgt_data = r!(tgt);
                let bytes = tgt_data.as_str(string_pool).as_bytes();
                if idx >= bytes.len() {
                    cold_path();
                    throw_error(
                        err_ctx,
                        &instructions[i],
                        ErrType::IndexOutOfBounds(bytes.len(), idx),
                    );
                }
                *w!(dest) = str!(unsafe {
                    std::str::from_utf8_unchecked(std::slice::from_ref(bytes.get_unchecked(idx)))
                });
            }
            Instr::Push(array, element) => unsafe {
                array_pool
                    .get_unchecked_mut(r!(array).as_array())
                    .push(r!(element));
            },
            Instr::Remove(array, idx) => {
                let arr = unsafe { array_pool.get_unchecked_mut(r!(array).as_array()) };
                let index = r!(idx).as_int() as usize;
                if index >= arr.len() {
                    cold_path();
                    throw_error(
                        err_ctx,
                        &instructions[i],
                        ErrType::IndexOutOfBounds(arr.len(), index),
                    );
                }
                arr.remove(index);
            }
            Instr::CallLibFunc(LibFunc::Uppercase, source_string_reg_id, dest_reg_id) => {
                *w!(dest_reg_id) =
                    string!(r!(source_string_reg_id).as_str(string_pool).to_uppercase());
            }
            Instr::CallLibFunc(LibFunc::Lowercase, source_string_reg_id, dest_reg_id) => {
                *w!(dest_reg_id) =
                    string!(r!(source_string_reg_id).as_str(string_pool).to_lowercase());
            }
            Instr::CallLibFunc(LibFunc::Contains, tgt, dest) => {
                let reg = r!(tgt);
                if reg.is_str() {
                    let str = reg.as_str(string_pool);
                    let temp_arg = r!(args.pop().unwrap_unchecked());
                    let arg = temp_arg.as_str(string_pool);
                    *w!(dest) = str.contains(arg).into();
                } else if reg.is_array() {
                    let arg = r!(args.pop().unwrap_unchecked());
                    *w!(dest) = unsafe { array_pool.get_unchecked(reg.as_array()) }
                        .contains(&arg)
                        .into();
                }
            }
            Instr::CallLibFunc(LibFunc::Trim, tgt, dest) => {
                *w!(dest) = str!(r!(tgt).as_str(string_pool).trim());
            }
            Instr::CallLibFunc(LibFunc::TrimSequence, tgt, dest) => {
                let temp_arg = r!(args.pop().unwrap_unchecked());
                let arg = temp_arg.as_str(string_pool);
                let chars: Vec<char> = arg.chars().collect();
                *w!(dest) = str!(r!(tgt).as_str(string_pool).trim_matches(&chars[..]));
            }
            Instr::CallLibFunc(LibFunc::Find, tgt, dest) => {
                let reg = r!(tgt);
                if reg.is_str() {
                    let str = reg.as_str(string_pool);
                    let temp_elem = r!(args.pop().unwrap_unchecked());
                    let element = temp_elem.as_str(string_pool);
                    *w!(dest) = if let Some(idx) = str.find(element) {
                        idx as i32
                    } else {
                        -1
                    }
                    .into();
                } else if reg.is_array() {
                    let arr_id = reg.as_array();
                    let element = r!(args.pop().unwrap_unchecked());
                    *w!(dest) = if let Some(idx) = unsafe { array_pool.get_unchecked(arr_id) }
                        .iter()
                        .position(|x| x == &element)
                    {
                        idx as i32
                    } else {
                        -1
                    }
                    .into()
                }
            }
            Instr::CallLibFunc(LibFunc::IsFloat, tgt, dest) => {
                let temp_tgt = r!(tgt);
                let num = temp_tgt.as_str(string_pool);
                *w!(dest) = (num.parse::<i64>().is_err() && num.parse::<f64>().is_ok()).into();
            }
            Instr::CallLibFunc(LibFunc::IsInt, tgt, dest) => {
                *w!(dest) = r!(tgt).as_str(string_pool).parse::<i64>().is_ok().into()
            }
            Instr::CallLibFunc(LibFunc::TrimLeft, tgt, dest) => {
                *w!(dest) = str!(r!(tgt).as_str(string_pool).trim_start());
            }
            Instr::CallLibFunc(LibFunc::TrimRight, tgt, dest) => {
                *w!(dest) = str!(r!(tgt).as_str(string_pool).trim_end());
            }
            Instr::CallLibFunc(LibFunc::TrimSequenceLeft, tgt, dest) => {
                let chars: Vec<char> = r!(args.pop().unwrap_unchecked())
                    .as_str(string_pool)
                    .chars()
                    .collect();
                *w!(dest) = str!(r!(tgt).as_str(string_pool).trim_start_matches(&chars[..]));
            }
            Instr::CallLibFunc(LibFunc::TrimSequenceRight, tgt, dest) => {
                let chars: Vec<char> = r!(args.pop().unwrap_unchecked())
                    .as_str(string_pool)
                    .chars()
                    .collect();
                *w!(dest) = str!(r!(tgt).as_str(string_pool).trim_end_matches(&chars[..]));
            }
            Instr::CallLibFunc(LibFunc::Repeat, tgt, dest) => {
                let reg = r!(tgt);
                if reg.is_str() {
                    let str = reg.as_str(string_pool);
                    let repeat_count = r!(args.pop().unwrap_unchecked()).as_int();
                    *w!(dest) = string!(str.repeat(repeat_count as usize));
                } else if reg.is_array() {
                    let repeat_count = r!(args.pop().unwrap_unchecked()).as_int();
                    let array_id = alloc_array(
                        array_pool,
                        &mut free_arrays,
                        registers,
                        &recursion_stack,
                        &mut gc_array_threshold,
                        &mut array_live,
                        &mut array_gc_stack,
                    );
                    unsafe {
                        *array_pool.get_unchecked_mut(array_id as usize) = array_pool
                            .get_unchecked(reg.as_array())
                            .repeat(repeat_count as usize);
                    }
                    *w!(dest) = Data::array(array_id);
                }
            }
            Instr::CallLibFunc(LibFunc::Round, tgt, dest) => {
                *w!(dest) = r!(tgt).as_float().round().into();
            }
            Instr::CallLibFunc(LibFunc::Abs, tgt, dest) => {
                let tgt = r!(tgt);
                *w!(dest) = if tgt.is_float() {
                    tgt.as_float().abs().into()
                } else {
                    tgt.as_int().abs().into()
                }
            }
            Instr::CallLibFunc(LibFunc::Reverse, tgt, dest) => {
                *w!(dest) = string!(
                    r!(tgt)
                        .as_str(string_pool)
                        .chars()
                        .rev()
                        .collect::<String>()
                );
            }
            Instr::CallLibFuncVoid(LibFuncVoid::Reverse, tgt, _) => {
                unsafe { array_pool.get_unchecked_mut(r!(tgt).as_array()) }.reverse();
            }
            Instr::CallLibFunc(LibFunc::SqrtFloat, tgt, dest) => {
                *w!(dest) = r!(tgt).as_float().sqrt().into()
            }
            Instr::CallLibFunc(LibFunc::Float, tgt, dest) => {
                let reg = r!(tgt);
                if reg.is_int() {
                    *w!(dest) = (reg.as_int() as f64).into();
                } else if reg.is_str() {
                    let str = reg.as_str(string_pool);
                    *w!(dest) = (str.parse::<f64>().unwrap_or_else(|_| {
                        cold_path();
                        throw_error(err_ctx, &instructions[i], ErrType::FloatParsingError);
                    }))
                    .into();
                }
            }
            Instr::CallLibFunc(LibFunc::Int, tgt, dest) => {
                let reg = r!(tgt);
                if reg.is_float() {
                    *w!(dest) = (reg.as_float() as i32).into();
                } else if reg.is_str() {
                    let str = reg.as_str(string_pool);
                    *w!(dest) = (str.parse::<i32>().unwrap_or_else(|e| {
                        cold_path();
                        throw_error(err_ctx, &instructions[i], (*e.kind()).into());
                    }))
                    .into();
                }
            }
            Instr::CallLibFunc(LibFunc::Str, tgt, dest) => {
                let value = r!(tgt);
                *w!(dest) = if value.is_str() {
                    value
                } else if value.is_int() {
                    string!(value.as_int().to_string())
                } else if value.is_float() {
                    string!(value.as_float().to_string())
                } else if value.is_bool() {
                    str!(if value.as_bool() { "true" } else { "false" })
                } else {
                    str!(&format_data(&value, array_pool, string_pool, false))
                };
            }
            Instr::CallLibFunc(LibFunc::Bool, tgt, dest) => {
                let temp_tgt = r!(tgt);
                let str = temp_tgt.as_str(string_pool);
                *w!(dest) = (str.parse::<bool>().unwrap_or_else(|_| {
                    cold_path();
                    throw_error(err_ctx, &instructions[i], ErrType::BoolParsingError);
                }))
                .into();
            }
            Instr::CallLibFunc(LibFunc::Input, tgt, dest) => {
                let temp_tgt = r!(tgt);
                let str_msg = temp_tgt.as_str(string_pool);
                write!(handle, "{str_msg}").unwrap();
                std::io::stdout().flush().unwrap();
                let mut line = String::new();
                std::io::stdin().read_line(&mut line).unwrap();
                *w!(dest) = str!(line.trim_end_matches(['\n', '\r']));
            }
            Instr::CallLibFunc(LibFunc::Floor, tgt, dest) => {
                *w!(dest) = r!(tgt).as_float().floor().into()
            }
            #[allow(unused_must_use)]
            Instr::CallLibFunc(LibFunc::TheAnswer, _, dest) => {
                writeln!(
                    handle,
                    "The answer to the Ultimate Question of Life, the Universe, and Everything is 42."
                );
                *w!(dest) = 42.into();
            }
            Instr::CallLibFunc(LibFunc::Len, tgt, dest) => {
                let reg = r!(tgt);
                if reg.is_array() {
                    *w!(dest) =
                        (unsafe { array_pool.get_unchecked(reg.as_array()) }.len() as i32).into()
                } else if reg.is_str() {
                    *w!(dest) = (reg.as_str(string_pool).len() as i32).into()
                }
            }
            Instr::CallLibFunc(LibFunc::StartsWith, source_register, dest_register) => {
                *w!(dest_register) = r!(source_register)
                    .as_str(string_pool)
                    .starts_with(r!(args.pop().unwrap_unchecked()).as_str(string_pool))
                    .into();
            }
            Instr::CallLibFunc(LibFunc::EndsWith, source_register, dest_register) => {
                *w!(dest_register) = r!(source_register)
                    .as_str(string_pool)
                    .ends_with(r!(args.pop().unwrap_unchecked()).as_str(string_pool))
                    .into();
            }
            #[allow(clippy::no_effect_replace)]
            Instr::CallLibFunc(LibFunc::Replace, source_register, dest_register) => {
                *w!(dest_register) = string!(r!(source_register).as_str(string_pool).replace(
                    r!(args.pop().unwrap_unchecked()).as_str(string_pool),
                    r!(args.pop().unwrap_unchecked()).as_str(string_pool),
                ));
            }
            Instr::CallLibFunc(LibFunc::Split, source_register, dest_register) => {
                let source = r!(source_register);
                let separator = unsafe { args.pop().unwrap_unchecked() };
                if source.is_str() {
                    let output_str_reg_id = alloc_array(
                        array_pool,
                        &mut free_arrays,
                        registers,
                        &recursion_stack,
                        &mut gc_array_threshold,
                        &mut array_live,
                        &mut array_gc_stack,
                    );
                    let source = source.as_str(string_pool);
                    let separator_data = r!(separator);
                    let separator = separator_data.as_str(string_pool);
                    let output_len = if separator.is_empty() {
                        source.len() + 2
                    } else {
                        source.matches(separator).count() + 1
                    };
                    let output =
                        unsafe { array_pool.get_unchecked_mut(output_str_reg_id as usize) };
                    output.clear();
                    output.reserve(output_len);
                    for part in source.split(separator) {
                        output.push({
                            if part.len() <= 6 {
                                Data::small_str(part)
                            } else if let Some(id) = free_strings.pop() {
                                string_pool[id as usize] = part.to_string();
                                Data::large_str_id(id as u64)
                            } else {
                                let id = string_pool.len() as u64;
                                string_pool.push(part.to_string());
                                Data::large_str_id(id)
                            }
                        });
                    }
                    raise_string_gc_threshold(&mut gc_string_threshold, string_pool.len());
                    *w!(dest_register) = Data::array(output_str_reg_id);
                } else if source.is_array() {
                    let source_array_id = source.as_array();
                    let separator = r!(separator);
                    let source_array = unsafe { array_pool.get_unchecked(source_array_id) };

                    let mut split_ranges: Vec<(usize, usize)> =
                        Vec::with_capacity(source_array.len());

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
                            array_pool,
                            &mut free_arrays,
                            registers,
                            &recursion_stack,
                            &mut gc_array_threshold,
                            &mut array_live,
                            &mut array_gc_stack,
                        ) as usize;
                        if dest_array_id < source_array_id {
                            let (left, right) = array_pool.split_at_mut(source_array_id);
                            left[dest_array_id].extend_from_slice(&right[0][start..end]);
                        } else {
                            let (left, right) = array_pool.split_at_mut(dest_array_id);
                            right[0].extend_from_slice(&left[source_array_id][start..end]);
                        }
                        sub_arrays.push(Data::array(dest_array_id as u32));
                    }

                    let array_id = alloc_array(
                        array_pool,
                        &mut free_arrays,
                        registers,
                        &recursion_stack,
                        &mut gc_array_threshold,
                        &mut array_live,
                        &mut array_gc_stack,
                    );
                    *unsafe { array_pool.get_unchecked_mut(array_id as usize) } = sub_arrays;

                    *w!(dest_register) = Data::array(array_id);
                }
            }
            Instr::CallLibFunc(LibFunc::Range, max, dest) => {
                let min = if let Some(reg_id) = args.pop() {
                    r!(reg_id).as_int()
                } else {
                    0
                };
                let max = r!(max).as_int();
                let output_array_id = alloc_array(
                    array_pool,
                    &mut free_arrays,
                    registers,
                    &recursion_stack,
                    &mut gc_array_threshold,
                    &mut array_live,
                    &mut array_gc_stack,
                );
                let range_arr = unsafe { array_pool.get_unchecked_mut(output_array_id as usize) };
                range_arr.clear();
                range_arr.extend((min..max).map(Data::from));
                *w!(dest) = Data::array(output_array_id);
            }
            Instr::CallLibFunc(LibFunc::JoinStringArray, tgt, dest) => {
                let temp_separator: Option<Data> = args.pop().map(|arg| r!(arg));
                let separator = temp_separator
                    .as_ref()
                    .map_or("", |d| d.as_str(string_pool));
                let array = unsafe { array_pool.get_unchecked(r!(tgt).as_array()) };
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
                *w!(dest) = string!(output);
            }
            // -----
            // FILE SYSTEM FUNCTIONS
            // -----
            Instr::CallLibFunc(LibFunc::FsRead, path, dest_reg_id) => {
                *w!(dest_reg_id) = string!(
                    fs::read_to_string(r!(path).as_str(string_pool)).unwrap_or_else(|e| {
                        cold_path();
                        throw_error(err_ctx, &instructions[i], e.kind().into())
                    })
                );
            }
            Instr::CallLibFunc(LibFunc::FsExists, path, dest_reg_id) => {
                *w!(dest_reg_id) = fs::exists(r!(path).as_str(string_pool))
                    .unwrap_or_else(|e| {
                        cold_path();
                        throw_error(err_ctx, &instructions[i], e.kind().into())
                    })
                    .into()
            }
            // Overwrites a file, will create it if it doesn't exist
            Instr::CallLibFuncVoid(LibFuncVoid::FsWrite, path, contents) => {
                fs::write(
                    r!(path).as_str(string_pool),
                    r!(contents).as_str(string_pool),
                )
                .unwrap_or_else(|e| {
                    cold_path();
                    throw_error(err_ctx, &instructions[i], e.kind().into())
                });
            }
            // Appends to a file, will create if it doesn't exist
            Instr::CallLibFuncVoid(LibFuncVoid::FsAppend, path, contents) => {
                fs::OpenOptions::new()
                    .append(true)
                    .open(r!(path).as_str(string_pool))
                    .unwrap_or_else(|e| {
                        cold_path();
                        throw_error(err_ctx, &instructions[i], e.kind().into())
                    })
                    .write_all(r!(contents).as_str(string_pool).as_bytes())
                    .unwrap_or_else(|e| {
                        cold_path();
                        throw_error(err_ctx, &instructions[i], e.kind().into())
                    });
            }
            // Deletes the file located at `path`, throwing an error if it doesn't exist.
            Instr::CallLibFuncVoid(LibFuncVoid::FsDelete, path, _) => {
                fs::remove_file(r!(path).as_str(string_pool)).unwrap_or_else(|e| {
                    cold_path();
                    throw_error(err_ctx, &instructions[i], e.kind().into())
                });
            }
            // Deletes the empty directory located at `path`
            Instr::CallLibFuncVoid(LibFuncVoid::FsDeleteDir, path, _) => {
                fs::remove_dir(r!(path).as_str(string_pool)).unwrap_or_else(|e| {
                    cold_path();
                    throw_error(err_ctx, &instructions[i], e.kind().into())
                });
            }
            Instr::CallLibFunc(LibFunc::Argv, _, dest) => {
                let array_id = alloc_array(
                    array_pool,
                    &mut free_arrays,
                    registers,
                    &recursion_stack,
                    &mut gc_array_threshold,
                    &mut array_live,
                    &mut array_gc_stack,
                );
                *unsafe { array_pool.get_unchecked_mut(array_id as usize) } = std::env::args()
                    .skip(2)
                    .map(|s| string!(s))
                    .collect::<Vec<Data>>();
                *w!(dest) = Data::array(array_id);
            }
            Instr::CallLibFuncVoid(LibFuncVoid::Sort, tgt, _) => {
                let array = unsafe { array_pool.get_unchecked_mut(r!(tgt).as_array()) };
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
            Instr::Halt(code) => {
                cold_path();
                if code != 0 {
                    std::process::exit(r!(code).as_int());
                }
                break;
            }
        }
        i += 1;
    }
}
