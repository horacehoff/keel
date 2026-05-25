use self::grammar::Token;
use crate::data::NULL;
use crate::errors::ErrType;
use crate::errors::lalrpop_error;
use crate::errors::throw_parser_error;
#[cfg(target_arch = "wasm32")]
use crate::errors::wasm_error;
use crate::expr::Expr;
use crate::expr::Span;
use crate::expr::contains_var_reassign;
use crate::functions::handle_functions;
use crate::instr::LibFunc;
use crate::methods::handle_method_calls;
use crate::parser_data::*;
use crate::registers::alloc_register;
use crate::registers::free_loop_scope_registers;
use crate::registers::free_register;
use crate::registers::free_scope_registers;
use crate::registers::get_last_tgt_id;
use crate::registers::get_tgt_id;
use crate::registers::is_reg_free;
use crate::registers::move_reg_to_reg;
use crate::registers::move_to_id;
use crate::type_system::check_if_returns_void;
use crate::type_system::contains_recursive_call;
#[cfg(not(target_arch = "wasm32"))]
use crate::type_system::datatype_to_c_type;
use crate::type_system::is_type_indexable;
use crate::type_system::mark_mutually_recursive;
use crate::type_system::{DataType, infer_type};
use crate::{data::Data, instr::Instr};
use inline_colorization::*;
use lalrpop_util::lalrpop_mod;
use smol_str::SmolStr;
use smol_str::ToSmolStr;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::slice;

lalrpop_mod!(pub grammar);

/// Fuses the last comparison instruction into a jump instruction (jumps when condition is false)
#[inline(always)]
fn add_cmp_false(condition_id: u16, len: &mut u16, output: &mut Vec<Instr>, jmp_backwards: bool) {
    if output.is_empty() {
        return output.push(Instr::IsFalseJmp(condition_id, *len));
    }
    *output.last_mut().unwrap() = match *output.last().unwrap() {
        Instr::InfFloat(o1, o2, o3) if o3 == condition_id => Instr::SupEqFloatJmp(o1, o2, *len),
        Instr::InfInt(o1, o2, o3) if o3 == condition_id => Instr::SupEqIntJmp(o1, o2, *len),
        Instr::InfEqFloat(o1, o2, o3) if o3 == condition_id => Instr::SupFloatJmp(o1, o2, *len),
        Instr::InfEqInt(o1, o2, o3) if o3 == condition_id => Instr::SupIntJmp(o1, o2, *len),
        Instr::SupFloat(o1, o2, o3) if o3 == condition_id => Instr::InfEqFloatJmp(o1, o2, *len),
        Instr::SupInt(o1, o2, o3) if o3 == condition_id => Instr::InfEqIntJmp(o1, o2, *len),
        Instr::SupEqFloat(o1, o2, o3) if o3 == condition_id => Instr::InfFloatJmp(o1, o2, *len),
        Instr::SupEqInt(o1, o2, o3) if o3 == condition_id => Instr::InfIntJmp(o1, o2, *len),
        Instr::Eq(o1, o2, o3) if o3 == condition_id => Instr::NotEqJmp(o1, o2, *len),
        Instr::ArrayEq(o1, o2, o3) if o3 == condition_id => Instr::ArrayNotEqJmp(o1, o2, *len),
        Instr::StrEq(o1, o2, o3) if o3 == condition_id => Instr::StrNotEqJmp(o1, o2, *len),
        Instr::NotEq(o1, o2, o3) if o3 == condition_id => Instr::EqJmp(o1, o2, *len),
        Instr::ArrayNotEq(o1, o2, o3) if o3 == condition_id => Instr::ArrayEqJmp(o1, o2, *len),
        Instr::StrNotEq(o1, o2, o3) if o3 == condition_id => Instr::StrEqJmp(o1, o2, *len),
        _ => {
            output.push(Instr::IsFalseJmp(condition_id, *len));
            return;
        }
    };
    if jmp_backwards {
        *len -= 1;
    }
}

/// Fuses the last comparison instruction into a jump instruction (jumps when condition is true)
#[inline(always)]
fn add_cmp_true(condition_id: u16, output: &mut Vec<Instr>) {
    if output.is_empty() {
        return output.push(Instr::IsTrueJmp(condition_id, 0));
    }
    let new_instr = match *output.last().unwrap() {
        Instr::InfFloat(o1, o2, o3) if o3 == condition_id => Instr::InfFloatJmp(o1, o2, 0),
        Instr::InfInt(o1, o2, o3) if o3 == condition_id => Instr::InfIntJmp(o1, o2, 0),
        Instr::InfEqFloat(o1, o2, o3) if o3 == condition_id => Instr::InfEqFloatJmp(o1, o2, 0),
        Instr::InfEqInt(o1, o2, o3) if o3 == condition_id => Instr::InfEqIntJmp(o1, o2, 0),
        Instr::SupFloat(o1, o2, o3) if o3 == condition_id => Instr::SupFloatJmp(o1, o2, 0),
        Instr::SupInt(o1, o2, o3) if o3 == condition_id => Instr::SupIntJmp(o1, o2, 0),
        Instr::SupEqFloat(o1, o2, o3) if o3 == condition_id => Instr::SupEqFloatJmp(o1, o2, 0),
        Instr::SupEqInt(o1, o2, o3) if o3 == condition_id => Instr::SupEqIntJmp(o1, o2, 0),
        Instr::Eq(o1, o2, o3) if o3 == condition_id => Instr::EqJmp(o1, o2, 0),
        Instr::ArrayEq(o1, o2, o3) if o3 == condition_id => Instr::ArrayEqJmp(o1, o2, 0),
        Instr::StrEq(o1, o2, o3) if o3 == condition_id => Instr::StrEqJmp(o1, o2, 0),
        Instr::NotEq(o1, o2, o3) if o3 == condition_id => Instr::NotEqJmp(o1, o2, 0),
        Instr::ArrayNotEq(o1, o2, o3) if o3 == condition_id => Instr::ArrayNotEqJmp(o1, o2, 0),
        Instr::StrNotEq(o1, o2, o3) if o3 == condition_id => Instr::StrNotEqJmp(o1, o2, 0),
        _ => {
            output.push(Instr::IsTrueJmp(condition_id, 0));
            return;
        }
    };
    *output.last_mut().unwrap() = new_instr;
}

/// Sets the jump size field of a jump instruction
#[inline(always)]
fn set_jmp_size(instr: &mut Instr, size: u16) {
    match instr {
        Instr::IsFalseJmp(_, jump_size)
        | Instr::IsTrueJmp(_, jump_size)
        | Instr::Jmp(jump_size)
        | Instr::SupEqFloatJmp(_, _, jump_size)
        | Instr::SupEqIntJmp(_, _, jump_size)
        | Instr::SupFloatJmp(_, _, jump_size)
        | Instr::SupIntJmp(_, _, jump_size)
        | Instr::InfEqFloatJmp(_, _, jump_size)
        | Instr::InfEqIntJmp(_, _, jump_size)
        | Instr::InfFloatJmp(_, _, jump_size)
        | Instr::InfIntJmp(_, _, jump_size)
        | Instr::InfIntJmpBack(_, _, jump_size)
        | Instr::NotEqJmp(_, _, jump_size)
        | Instr::EqJmp(_, _, jump_size)
        | Instr::ArrayNotEqJmp(_, _, jump_size)
        | Instr::ArrayEqJmp(_, _, jump_size)
        | Instr::StrNotEqJmp(_, _, jump_size)
        | Instr::StrEqJmp(_, _, jump_size) => *jump_size = size,
        _ => unreachable!(),
    }
}

/// Compiles short-circuit && and || conditions
/// bool_or_mode true indicates left side of ||, emits true jumps
/// bool_or_mode false emits false jumps
/// Returns (true_jump_idxs, false_jump_idxs)
#[allow(clippy::too_many_arguments)]
fn compile_short_circuit_condition(
    expr: &Expr,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
    offset: u16,
    single_run: bool,
    bool_or_mode: bool,
) -> (Vec<usize>, Vec<usize>) {
    match expr {
        Expr::BoolOr(left, right, _) => {
            // left side of || always uses true jump mode
            let (mut true_jumps, _) = compile_short_circuit_condition(
                left, v, ctx, state, output, offset, single_run, true,
            );
            let (right_true, right_false) = compile_short_circuit_condition(
                right,
                v,
                ctx,
                state,
                output,
                offset,
                single_run,
                bool_or_mode,
            );
            true_jumps.extend(right_true);
            (true_jumps, right_false)
        }
        Expr::BoolAnd(left, right, _) => {
            if bool_or_mode {
                // && inside left side of ||
                let id_l = get_id(left, v, ctx, state, output, None, false, offset, single_run);
                let id_r = get_id(
                    right, v, ctx, state, output, None, false, offset, single_run,
                );
                free_register(id_l, state.free_registers, v, state.const_registers);
                free_register(id_r, state.free_registers, v, state.const_registers);
                let id = alloc_register(state.registers, state.free_registers);
                output.push(Instr::BoolAnd(id_l, id_r, id));
                add_cmp_true(id, output);
                free_register(id, state.free_registers, v, state.const_registers);
                (vec![output.len() - 1], Vec::new())
            } else {
                // normal && -> if either side is false, jump past the body
                let (_, mut false_jumps) = compile_short_circuit_condition(
                    left, v, ctx, state, output, offset, single_run, false,
                );
                let (_, right_false) = compile_short_circuit_condition(
                    right, v, ctx, state, output, offset, single_run, false,
                );
                false_jumps.extend(right_false);
                (Vec::new(), false_jumps)
            }
        }
        expr => {
            let cond_id = get_id(expr, v, ctx, state, output, None, false, offset, single_run);
            if bool_or_mode {
                add_cmp_true(cond_id, output);
                free_register(cond_id, state.free_registers, v, state.const_registers);
                (vec![output.len() - 1], Vec::new())
            } else {
                add_cmp_false(cond_id, &mut 0, output, false);
                free_register(cond_id, state.free_registers, v, state.const_registers);
                (Vec::new(), vec![output.len() - 1])
            }
        }
    }
}

fn parse_loop_flow_control(
    loop_code: &mut [Instr],
    loop_id: u16,
    code_length: u16,
    for_loop: bool,
    indefinite: bool,
) {
    loop_code.iter_mut().enumerate().for_each(|(i, x)| {
        if let Instr::NotEqJmp(break_id, 0, 0) = x
            && *break_id == loop_id
        {
            if for_loop && !indefinite {
                *x = Instr::Jmp(code_length - i as u16 - 1);
            } else {
                *x = Instr::Jmp(code_length - i as u16);
            }
        } else if let Instr::EqJmp(continue_id, 0, 0) = x
            && *continue_id == loop_id
        {
            if for_loop {
                *x = Instr::Jmp(code_length - i as u16 - 3);
            } else {
                // loop blocks and while loops only have 1 trailing instruction
                *x = Instr::Jmp(code_length - i as u16 - 1);
            }
        }
    });
}

pub fn get_id(
    input: &Expr,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
    tgt_id: Option<u16>,
    var_assignment: bool,
    offset: u16,
    single_run: bool,
) -> u16 {
    let src = ctx.src;
    macro_rules! uniform_op {
        ($instr: ident,$symbol:expr, $l: expr, $r: expr, $markers: expr, $type:expr) => {{
            let (t_l, t_r) = (
                infer_type($l, v, state.fns, src, state.dyn_libs),
                infer_type($r, v, state.fns, src, state.dyn_libs),
            );
            if t_l != $type || t_r != $type {
                throw_parser_error(src, $markers, ErrType::OpError(&t_l, &t_r, $symbol))
            }
            let id_l = get_id($l, v, ctx, state, output, None, false, offset, single_run);
            let id_r = get_id($r, v, ctx, state, output, None, false, offset, single_run);
            free_register(id_l, state.free_registers, v, state.const_registers);
            free_register(id_r, state.free_registers, v, state.const_registers);
            let id =
                tgt_id.unwrap_or_else(|| alloc_register(state.registers, state.free_registers));
            output.push(Instr::$instr(id_l, id_r, id));
            id
        }};
        ($instr: ident, $instr2:ident,$symbol:expr, $l: expr, $r: expr, $markers: expr, $type1:expr, $type2:expr) => {{
            let (t_l, t_r) = (
                infer_type($l, v, state.fns, src, state.dyn_libs),
                infer_type($r, v, state.fns, src, state.dyn_libs),
            );
            if !((t_l == $type1 && t_r == $type1) || (t_l == $type2 && t_r == $type2)) {
                throw_parser_error(src, $markers, ErrType::OpError(&t_l, &t_r, $symbol))
            }
            let id_l = get_id($l, v, ctx, state, output, None, false, offset, single_run);
            let id_r = get_id($r, v, ctx, state, output, None, false, offset, single_run);
            free_register(id_l, state.free_registers, v, state.const_registers);
            free_register(id_r, state.free_registers, v, state.const_registers);
            let id =
                tgt_id.unwrap_or_else(|| alloc_register(state.registers, state.free_registers));
            output.push(if t_l == $type1 {
                Instr::$instr(id_l, id_r, id)
            } else {
                Instr::$instr2(id_l, id_r, id)
            });
            id
        }};
    }
    match input {
        Expr::Float(num) => {
            if var_assignment {
                state.registers.push((*num).into());
                return (state.registers.len() - 1) as u16;
            }
            let data = (*num).into();
            if let Some(&id) = state.const_registers.get(&data) {
                id
            } else {
                state.registers.push(data);
                let id = (state.registers.len() - 1) as u16;
                state.const_registers.insert(data, id);
                id
            }
        }
        Expr::Int(num) => {
            if var_assignment {
                state.registers.push((*num).into());
                return (state.registers.len() - 1) as u16;
            }
            let data = (*num).into();
            if let Some(&id) = state.const_registers.get(&data) {
                id
            } else {
                let id = state.registers.len() as u16;
                state.const_registers.insert(data, id);
                state.registers.push(data);
                id
            }
        }
        Expr::String(str) => {
            if var_assignment {
                state
                    .registers
                    .push(Data::p_str(str, &mut state.pools.string_pool));
                return (state.registers.len() - 1) as u16;
            }
            let data = Data::p_str(str, &mut state.pools.string_pool);
            if let Some(&id) = state.const_registers.get(&data) {
                id
            } else {
                let id = state.registers.len() as u16;
                state.const_registers.insert(data, id);
                state.registers.push(data);
                id
            }
        }
        Expr::Null => {
            if var_assignment {
                state.registers.push(NULL);
                return (state.registers.len() - 1) as u16;
            }
            if let Some(&id) = state.const_registers.get(&NULL) {
                id
            } else {
                let id = state.registers.len() as u16;
                state.const_registers.insert(NULL, id);
                state.registers.push(NULL);
                id
            }
        }
        Expr::Bool(bool) => {
            if var_assignment {
                state.registers.push((*bool).into());
                return (state.registers.len() - 1) as u16;
            }
            let data: Data = (*bool).into();
            if let Some(&id) = state.const_registers.get(&data) {
                id
            } else {
                let id = state.registers.len() as u16;
                state.const_registers.insert(data, id);
                state.registers.push(data);
                id
            }
        }
        Expr::Var(name, markers) => {
            if let Some(Variable {
                name: _,
                register_id,
                var_type: _,
            }) = v.iter().rfind(|v_temp| *name == v_temp.name)
            {
                *register_id
            } else {
                throw_parser_error(src, markers, ErrType::UnknownVariable(name))
            }
        }
        Expr::Array(elems, markers) => {
            if let Some(first) = elems.first() {
                let first_type = infer_type(first, v, state.fns, src, state.dyn_libs);
                if !elems
                    .iter()
                    .all(|x| infer_type(x, v, state.fns, src, state.dyn_libs) == first_type)
                {
                    throw_parser_error(src, markers, ErrType::ArrayWithDiffType);
                }
            }
            let array_id = {
                state.pools.array_pool.push(Vec::new());
                state.pools.array_pool.len() - 1
            };
            if elems.is_empty() && !single_run {
                let array_reg = {
                    state.registers.push(Data::array(array_id as u32));
                    state.registers.len() - 1
                } as u16;
                output.push(Instr::EmptyArray(array_reg));
                return array_reg;
            }
            if single_run {
                for elem in elems {
                    let x = compile_expr(slice::from_ref(elem), v, ctx, state, 0, single_run);
                    if !x.is_empty() {
                        let c_id = get_tgt_id(*x.last().unwrap()).unwrap();
                        state.pools.array_pool.get_mut(array_id).unwrap().push(NULL);

                        output.extend(x);
                        output.push(Instr::ArrayElemMov(
                            c_id,
                            array_id as u16,
                            (state.pools.array_pool[array_id].len() - 1) as u16,
                        ));
                    } else {
                        state
                            .pools
                            .array_pool
                            .get_mut(array_id)
                            .unwrap()
                            .push(state.registers.pop().unwrap());
                    }
                }
                state.registers.push(Data::array(array_id as u32));
                (state.registers.len() - 1) as u16
            } else {
                // Check if all elements are constant (no instructions emitted)
                let mut constant_array = true;
                let mut elem_ids: Vec<(Vec<Instr>, u16)> = Vec::with_capacity(elems.len());
                for elem in elems {
                    let x = compile_expr(slice::from_ref(elem), v, ctx, state, 0, single_run);
                    if !x.is_empty() {
                        constant_array = false;
                        let c_id = get_tgt_id(*x.last().unwrap()).unwrap();
                        state.pools.array_pool.get_mut(array_id).unwrap().push(NULL);
                        elem_ids.push((x, c_id));
                    } else {
                        let reg_id = (state.registers.len() - 1) as u16;
                        state
                            .pools
                            .array_pool
                            .get_mut(array_id)
                            .unwrap()
                            .push(state.registers.pop().unwrap());
                        elem_ids.push((Vec::new(), reg_id));
                    }
                }

                if constant_array {
                    // The template array is held by a register to prevent it from being freed by the GC
                    let template_reg = {
                        state.registers.push(Data::array(array_id as u32));
                        (state.registers.len() - 1) as u16
                    };
                    let dest_reg = {
                        state.registers.push(Data::array(0));
                        (state.registers.len() - 1) as u16
                    };
                    output.push(Instr::CloneArray(
                        template_reg,
                        dest_reg,
                        state.pools.array_pool[array_id].len() as u16,
                    ));
                    dest_reg
                } else {
                    let dest_reg = {
                        state.registers.push(Data::array(0));
                        (state.registers.len() - 1) as u16
                    };
                    output.push(Instr::EmptyArray(dest_reg));
                    for (instrs, elem_reg) in elem_ids {
                        output.extend(instrs);
                        output.push(Instr::Push(dest_reg, elem_reg));
                    }
                    dest_reg
                }
            }
        }
        Expr::Mul(l, r, markers) => {
            uniform_op!(
                MulFloat,
                MulInt,
                "*",
                l,
                r,
                markers,
                DataType::Float,
                DataType::Int
            )
        }
        Expr::Div(l, r, markers) => {
            if let Expr::Int(n) = r.as_ref()
                && *n == 0
            {
                throw_parser_error(src, markers, ErrType::DivisionByZero);
            }
            let id = uniform_op!(
                DivFloat,
                DivInt,
                "/",
                l,
                r,
                markers,
                DataType::Float,
                DataType::Int
            );
            if matches!(output.last(), Some(Instr::DivInt(..))) {
                state
                    .instr_src
                    .push((*output.last().unwrap(), *markers, ctx.current_src_file));
            }
            id
        }
        Expr::Add(l, r, markers) => {
            let t_l = infer_type(l, v, state.fns, src, state.dyn_libs);
            let t_r = infer_type(r, v, state.fns, src, state.dyn_libs);
            if t_l != t_r
                || !matches!(
                    t_l,
                    DataType::String | DataType::Array(_) | DataType::Float | DataType::Int
                )
            {
                throw_parser_error(src, markers, ErrType::OpError(&t_l, &t_r, "+"));
            }
            // var+1 or 1+var use the dedicated IncInt/IncIntTo instructions
            if t_l == DataType::Int
                && let Some(Expr::Var(src_name, _)) = {
                    if matches!(r.as_ref(), Expr::Int(1)) {
                        Some(l.as_ref())
                    } else if matches!(l.as_ref(), Expr::Int(1)) {
                        Some(r.as_ref())
                    } else {
                        None
                    }
                }
                && let Some(src_var) = v.iter().rfind(|x| x.name == *src_name)
            {
                let src_id = src_var.register_id;
                let id =
                    tgt_id.unwrap_or_else(|| alloc_register(state.registers, state.free_registers));
                output.push(if src_id == id {
                    Instr::IncInt(id)
                } else {
                    Instr::IncIntTo(src_id, id)
                });
                return id;
            }
            let id_l = get_id(l, v, ctx, state, output, None, false, offset, single_run);
            let id_r = get_id(r, v, ctx, state, output, None, false, offset, single_run);
            free_register(id_l, state.free_registers, v, state.const_registers);
            free_register(id_r, state.free_registers, v, state.const_registers);
            let id = if let Some(tgt_register_id) = tgt_id {
                tgt_register_id
            } else {
                alloc_register(state.registers, state.free_registers)
            };
            if matches!(t_l, DataType::Array(_)) {
                output.push(Instr::AddArray(id_l, id_r, id));
            } else if t_l == DataType::String {
                output.push(Instr::AddStr(id_l, id_r, id));
            } else if t_l == DataType::Float {
                output.push(Instr::AddFloat(id_l, id_r, id));
            } else {
                output.push(Instr::AddInt(id_l, id_r, id));
            }
            id
        }
        Expr::Sub(l, r, markers) => {
            let t_l = infer_type(l, v, state.fns, src, state.dyn_libs);
            let t_r = infer_type(r, v, state.fns, src, state.dyn_libs);
            if !((t_l == DataType::Float && t_r == DataType::Float)
                || (t_l == DataType::Int && t_r == DataType::Int))
            {
                throw_parser_error(src, markers, ErrType::OpError(&t_l, &t_r, "-"));
            }
            // var-1 uses the dedicated DecInt/DecIntTo instructions
            if t_l == DataType::Int
                && matches!(r.as_ref(), Expr::Int(1))
                && let Expr::Var(src_name, _) = l.as_ref()
                && let Some(src_var) = v.iter().rfind(|x| x.name == *src_name)
            {
                let src_id = src_var.register_id;
                let id =
                    tgt_id.unwrap_or_else(|| alloc_register(state.registers, state.free_registers));
                output.push(if src_id == id {
                    Instr::DecInt(id)
                } else {
                    Instr::DecIntTo(src_id, id)
                });
                return id;
            }
            let id_l = get_id(l, v, ctx, state, output, None, false, offset, single_run);
            let id_r = get_id(r, v, ctx, state, output, None, false, offset, single_run);
            free_register(id_l, state.free_registers, v, state.const_registers);
            free_register(id_r, state.free_registers, v, state.const_registers);
            let id = if let Some(tgt_register_id) = tgt_id {
                tgt_register_id
            } else {
                alloc_register(state.registers, state.free_registers)
            };
            output.push(if t_l == DataType::Float {
                Instr::SubFloat(id_l, id_r, id)
            } else {
                Instr::SubInt(id_l, id_r, id)
            });
            id
        }
        Expr::Mod(l, r, markers) => {
            if let Expr::Int(n) = r.as_ref()
                && *n == 0
            {
                throw_parser_error(src, markers, ErrType::ModuloByZero);
            }
            let id = uniform_op!(
                ModFloat,
                ModInt,
                "%",
                l,
                r,
                markers,
                DataType::Float,
                DataType::Int
            );
            if matches!(output.last(), Some(Instr::ModInt(..))) {
                state
                    .instr_src
                    .push((*output.last().unwrap(), *markers, ctx.current_src_file));
            }
            id
        }
        Expr::Pow(l, r, markers) => {
            uniform_op!(
                PowFloat,
                PowInt,
                "^",
                l,
                r,
                markers,
                DataType::Float,
                DataType::Int
            )
        }
        Expr::Eq(l, r) => {
            let l_type = infer_type(l, v, state.fns, src, state.dyn_libs);
            let r_type = infer_type(r, v, state.fns, src, state.dyn_libs);
            let is_array =
                matches!(l_type, DataType::Array(_)) && matches!(r_type, DataType::Array(_));
            let is_string = l_type == DataType::String || r_type == DataType::String;
            let id_l = get_id(l, v, ctx, state, output, None, false, offset, single_run);
            let id_r = get_id(r, v, ctx, state, output, None, false, offset, single_run);
            free_register(id_l, state.free_registers, v, state.const_registers);
            free_register(id_r, state.free_registers, v, state.const_registers);
            let id = if let Some(tgt_register_id) = tgt_id {
                tgt_register_id
            } else {
                alloc_register(state.registers, state.free_registers)
            };
            output.push(if is_array {
                Instr::ArrayEq(id_l, id_r, id)
            } else if is_string {
                Instr::StrEq(id_l, id_r, id)
            } else {
                Instr::Eq(id_l, id_r, id)
            });
            id
        }
        Expr::NotEq(l, r) => {
            let l_type = infer_type(l, v, state.fns, src, state.dyn_libs);
            let r_type = infer_type(r, v, state.fns, src, state.dyn_libs);
            let is_array =
                matches!(l_type, DataType::Array(_)) && matches!(r_type, DataType::Array(_));
            let is_string = l_type == DataType::String || r_type == DataType::String;
            let id_l = get_id(l, v, ctx, state, output, None, false, offset, single_run);
            let id_r = get_id(r, v, ctx, state, output, None, false, offset, single_run);
            free_register(id_l, state.free_registers, v, state.const_registers);
            free_register(id_r, state.free_registers, v, state.const_registers);
            let id = if let Some(tgt_register_id) = tgt_id {
                tgt_register_id
            } else {
                alloc_register(state.registers, state.free_registers)
            };
            if is_array {
                output.push(Instr::ArrayNotEq(id_l, id_r, id));
            } else if is_string {
                output.push(Instr::StrNotEq(id_l, id_r, id));
            } else {
                output.push(Instr::NotEq(id_l, id_r, id));
            }
            id
        }
        Expr::Sup(l, r, markers) => {
            uniform_op!(
                SupFloat,
                SupInt,
                ">",
                l,
                r,
                markers,
                DataType::Float,
                DataType::Int
            )
        }
        Expr::SupEq(l, r, markers) => {
            uniform_op!(
                SupEqFloat,
                SupEqInt,
                ">=",
                l,
                r,
                markers,
                DataType::Float,
                DataType::Int
            )
        }
        Expr::Inf(l, r, markers) => {
            uniform_op!(
                InfFloat,
                InfInt,
                "<",
                l,
                r,
                markers,
                DataType::Float,
                DataType::Int
            )
        }
        Expr::InfEq(l, r, markers) => {
            uniform_op!(
                InfEqFloat,
                InfEqInt,
                "<=",
                l,
                r,
                markers,
                DataType::Float,
                DataType::Int
            )
        }
        Expr::BoolAnd(l, r, markers) => {
            uniform_op!(BoolAnd, "&&", l, r, markers, DataType::Bool)
        }
        Expr::BoolOr(l, r, markers) => {
            uniform_op!(BoolOr, "||", l, r, markers, DataType::Bool)
        }
        Expr::Neg(l, markers) => {
            let operand_type = infer_type(l, v, state.fns, src, state.dyn_libs);
            let id_l = get_id(l, v, ctx, state, output, None, false, offset, single_run);
            free_register(id_l, state.free_registers, v, state.const_registers);
            let id = if let Some(tgt_register_id) = tgt_id {
                tgt_register_id
            } else {
                alloc_register(state.registers, state.free_registers)
            };
            if operand_type == DataType::Float {
                output.push(Instr::NegFloat(id_l, id))
            } else if operand_type == DataType::Int {
                output.push(Instr::NegInt(id_l, id))
            } else {
                throw_parser_error(src, markers, ErrType::InvalidOp(&operand_type, "-"));
            }
            id
        }
        Expr::BoolNeg(l, markers) => {
            let operand_type = infer_type(l, v, state.fns, src, state.dyn_libs);
            let id_l = get_id(l, v, ctx, state, output, None, false, offset, single_run);
            free_register(id_l, state.free_registers, v, state.const_registers);
            let id = if let Some(tgt_register_id) = tgt_id {
                tgt_register_id
            } else {
                alloc_register(state.registers, state.free_registers)
            };
            if operand_type != DataType::Bool {
                throw_parser_error(src, markers, ErrType::InvalidOp(&operand_type, "!"));
            }
            output.push(Instr::NegBool(id_l, id));
            id
        }
        Expr::InlineCondition(main_condition, code, markers) => {
            let return_id = alloc_register(state.registers, state.free_registers);

            // get first code limit (after which there are only else(if) blocks)
            let main_code_limit = code
                .iter()
                .position(|x| matches!(x, Expr::ElseIfBlock(_, _) | Expr::ElseBlock(_)))
                .unwrap_or(code.len());

            let condition_blocks_count = code.len() - main_code_limit;
            let mut cmp_markers: Vec<usize> = Vec::with_capacity(condition_blocks_count);
            let mut jmp_markers: Vec<usize> = Vec::with_capacity(condition_blocks_count);
            let mut condition_markers: Vec<usize> = Vec::with_capacity(condition_blocks_count);

            // parse the main condition
            let condition_id = get_id(
                main_condition,
                v,
                ctx,
                state,
                output,
                None,
                false,
                offset,
                single_run,
            );
            add_cmp_false(condition_id, &mut 0, output, false);
            cmp_markers.push(output.len() - 1);

            let v_len = v.len();
            let regs_before = state.registers.len() as u16;
            // parse the main code block
            let cond_code = compile_expr(
                &code[0..main_code_limit],
                v,
                ctx,
                state,
                offset + output.len() as u16,
                single_run,
            );
            v.truncate(v_len);
            free_scope_registers(
                regs_before,
                &cond_code,
                state.free_registers,
                v,
                state.const_registers,
            );
            let is_empty = cond_code.is_empty();
            output.extend(cond_code);
            output.push(Instr::Mov(
                if is_empty {
                    (state.registers.len() - 1) as u16
                } else {
                    get_last_tgt_id(output).unwrap()
                },
                return_id,
            ));
            if main_code_limit != code.len() {
                output.push(Instr::Jmp(0));
                jmp_markers.push(output.len() - 1);
            }

            let mut else_exists = false;
            for elem in &code[main_code_limit..] {
                if let Expr::ElseIfBlock(condition, code) = elem {
                    condition_markers.push(output.len());
                    let condition_id = get_id(
                        condition, v, ctx, state, output, None, false, offset, single_run,
                    );
                    add_cmp_false(condition_id, &mut 0, output, false);
                    free_register(condition_id, state.free_registers, v, state.const_registers);
                    cmp_markers.push(output.len() - 1);
                    let v_len = v.len();
                    let regs_before = state.registers.len() as u16;
                    let cond_code = compile_expr(
                        code,
                        v,
                        ctx,
                        state,
                        offset + output.len() as u16,
                        single_run,
                    );
                    v.truncate(v_len);
                    free_scope_registers(
                        regs_before,
                        &cond_code,
                        state.free_registers,
                        v,
                        state.const_registers,
                    );
                    let is_empty = cond_code.is_empty();
                    output.extend(cond_code);
                    output.push(Instr::Mov(
                        if is_empty {
                            (state.registers.len() - 1) as u16
                        } else {
                            get_last_tgt_id(output).unwrap()
                        },
                        return_id,
                    ));
                    output.push(Instr::Jmp(0));
                    jmp_markers.push(output.len() - 1);
                } else if let Expr::ElseBlock(code) = elem {
                    else_exists = true;
                    condition_markers.push(output.len());
                    let v_len = v.len();
                    let regs_before = state.registers.len() as u16;
                    let cond_code = compile_expr(
                        code,
                        v,
                        ctx,
                        state,
                        offset + output.len() as u16,
                        single_run,
                    );
                    v.truncate(v_len);
                    free_scope_registers(
                        regs_before,
                        &cond_code,
                        state.free_registers,
                        v,
                        state.const_registers,
                    );
                    let is_empty = cond_code.is_empty();
                    output.extend(cond_code);
                    output.push(Instr::Mov(
                        if is_empty {
                            (state.registers.len() - 1) as u16
                        } else {
                            get_last_tgt_id(output).unwrap()
                        },
                        return_id,
                    ));
                }
            }
            if !else_exists {
                throw_parser_error(src, markers, ErrType::InvalidConditionalExpression);
            }

            for y in jmp_markers {
                let diff = output.len() - y;
                output[y] = Instr::Jmp(diff as u16);
            }
            for (i, y) in cmp_markers.iter().enumerate() {
                let diff = if i >= condition_markers.len() {
                    output.len() - 1 - y
                } else {
                    condition_markers[i] - y
                };
                if let Some(
                    Instr::IsFalseJmp(_, jump_size)
                    | Instr::SupEqFloatJmp(_, _, jump_size)
                    | Instr::SupEqIntJmp(_, _, jump_size)
                    | Instr::SupFloatJmp(_, _, jump_size)
                    | Instr::SupIntJmp(_, _, jump_size)
                    | Instr::InfEqFloatJmp(_, _, jump_size)
                    | Instr::InfEqIntJmp(_, _, jump_size)
                    | Instr::InfFloatJmp(_, _, jump_size)
                    | Instr::InfIntJmp(_, _, jump_size)
                    | Instr::NotEqJmp(_, _, jump_size)
                    | Instr::ArrayNotEqJmp(_, _, jump_size)
                    | Instr::EqJmp(_, _, jump_size)
                    | Instr::ArrayEqJmp(_, _, jump_size),
                ) = output.get_mut(*y)
                {
                    *jump_size = diff as u16;
                }
            }
            free_register(condition_id, state.free_registers, v, state.const_registers);
            return_id
        }
        Expr::FunctionCall(args, namespace, markers, args_indexes) => handle_functions(
            output,
            v,
            ctx,
            state,
            args,
            namespace,
            markers,
            args_indexes,
            offset,
            single_run,
        )
        .unwrap_or_else(|| {
            get_last_tgt_id(output).unwrap_or_else(|| (state.registers.len() - 1) as u16)
        }),
        other => {
            let output_code = compile_expr(
                slice::from_ref(other),
                v,
                ctx,
                state,
                offset + output.len() as u16,
                single_run,
            );
            if !output_code.is_empty() {
                output.extend(output_code);
                get_last_tgt_id(output).unwrap_or((state.registers.len() - 1) as u16)
            } else {
                (state.registers.len() - 1) as u16
            }
        }
    }
}

#[inline(always)]
pub fn compile_expr(
    input: &[Expr],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    offset: u16,
    single_run: bool,
) -> Vec<Instr> {
    let mut output: Vec<Instr> = Vec::with_capacity(input.len());

    let src = ctx.src;
    let block_id = ctx.block_id;
    let is_parsing_recursive = ctx.is_parsing_recursive;
    let current_src_file = ctx.current_src_file;

    for (idx, x) in input.iter().enumerate() {
        match x {
            // if number / bool / str, just push it to the registers, and the caller will grab the last index
            Expr::Float(num) => state.registers.push((*num).into()),
            Expr::Int(num) => state.registers.push((*num).into()),
            Expr::Bool(bool) => state.registers.push((*bool).into()),
            Expr::Null => state.registers.push(NULL),
            Expr::String(str) => state
                .registers
                .push(Data::p_str(str, &mut state.pools.string_pool)),
            Expr::Var(name, markers) => {
                if let Some(Variable {
                    name: _,
                    register_id,
                    var_type: _,
                }) = v.iter().find(|v_temp| *name == v_temp.name)
                {
                    output.push(Instr::Mov(
                        *register_id,
                        alloc_register(state.registers, state.free_registers),
                    ));
                } else {
                    throw_parser_error(src, markers, ErrType::UnknownVariable(name))
                }
            }
            Expr::Array(elems, markers) => {
                let first_type = infer_type(&elems[0], v, state.fns, src, state.dyn_libs);
                if !elems
                    .iter()
                    .all(|x| infer_type(x, v, state.fns, src, state.dyn_libs) == first_type)
                {
                    throw_parser_error(src, markers, ErrType::ArrayWithDiffType);
                }
                // create a new blank array
                let array_id = {
                    state.pools.array_pool.push(Vec::new());
                    state.pools.array_pool.len() - 1
                };
                // process each array element
                for elem in elems {
                    let x = compile_expr(slice::from_ref(elem), v, ctx, state, 0, single_run);
                    // if there are no instructions, then that means the element has been pushed to the registers, so pop it and push it directly to the array
                    if x.is_empty() {
                        state
                            .pools
                            .array_pool
                            .get_mut(array_id)
                            .unwrap()
                            .push(state.registers.pop().unwrap());
                    } else {
                        // if there are instructions, then push everything, add a null to the array, and then add an instruction to move the element to the array at runtime with ArrayMov
                        let c_id = get_tgt_id(*x.last().unwrap()).unwrap();
                        output.extend(x);
                        state.pools.array_pool.get_mut(array_id).unwrap().push(NULL);
                        output.push(Instr::ArrayElemMov(
                            c_id,
                            array_id as u16,
                            (state.pools.array_pool[array_id].len() - 1) as u16,
                        ));
                    }
                }
                state.registers.push(Data::array(array_id as u32));
            }
            // array[index]
            Expr::ArrayGetIndex(array, index, markers) => {
                let infered = infer_type(array, v, state.fns, src, state.dyn_libs);
                if !is_type_indexable(&infered) {
                    throw_parser_error(src, markers, ErrType::NotIndexable(&infered));
                }

                let id = get_id(
                    array,
                    v,
                    ctx,
                    state,
                    &mut output,
                    None,
                    false,
                    offset,
                    single_run,
                );

                let index_inferred = infer_type(index, v, state.fns, src, state.dyn_libs);
                if index_inferred != DataType::Int {
                    throw_parser_error(src, markers, ErrType::InvalidIndexType(&index_inferred));
                }
                let index_id = get_id(
                    index,
                    v,
                    ctx,
                    state,
                    &mut output,
                    None,
                    false,
                    offset,
                    single_run,
                );
                free_register(index_id, state.free_registers, v, state.const_registers);
                let dest_reg_id = alloc_register(state.registers, state.free_registers);

                let to_push = if infered == DataType::String {
                    Instr::GetIndexString(id, index_id, dest_reg_id)
                } else {
                    Instr::GetIndexArray(id, index_id, dest_reg_id)
                };
                state.instr_src.push((to_push, *markers, current_src_file));
                output.push(to_push);
            }
            // array[start..end]
            Expr::ArrayGetSlice(array, idx_start, idx_end, markers) => {
                let infered = infer_type(array, v, state.fns, src, state.dyn_libs);
                if !is_type_indexable(&infered) {
                    throw_parser_error(src, markers, ErrType::NotIndexable(&infered));
                }
                let id = get_id(
                    array,
                    v,
                    ctx,
                    state,
                    &mut output,
                    None,
                    false,
                    offset,
                    single_run,
                );
                let idx_start_inferred = infer_type(idx_start, v, state.fns, src, state.dyn_libs);
                if idx_start_inferred != DataType::Int {
                    throw_parser_error(
                        src,
                        markers,
                        ErrType::InvalidIndexType(&idx_start_inferred),
                    );
                }
                let idx_start_id = get_id(
                    idx_start,
                    v,
                    ctx,
                    state,
                    &mut output,
                    None,
                    false,
                    offset,
                    single_run,
                );
                let idx_end_inferred = infer_type(idx_end, v, state.fns, src, state.dyn_libs);
                if idx_end_inferred != DataType::Int {
                    throw_parser_error(src, markers, ErrType::InvalidIndexType(&idx_end_inferred));
                }
                let idx_end_id = get_id(
                    idx_end,
                    v,
                    ctx,
                    state,
                    &mut output,
                    None,
                    false,
                    offset,
                    single_run,
                );
                output.push(Instr::StoreFuncArg(idx_end_id));
                free_register(idx_start_id, state.free_registers, v, state.const_registers);
                free_register(idx_end_id, state.free_registers, v, state.const_registers);
                let dest_reg_id = alloc_register(state.registers, state.free_registers);
                let to_push = if infered == DataType::String {
                    Instr::GetSliceString(id, idx_start_id, dest_reg_id)
                } else {
                    Instr::GetSliceArray(id, idx_start_id, dest_reg_id)
                };
                state.instr_src.push((to_push, *markers, current_src_file));
                output.push(to_push);
            }
            // x[y] = z;
            Expr::ArrayModify(array, index, value, index_markers, elem_markers) => {
                let array_type = infer_type(array, v, state.fns, src, state.dyn_libs);
                if !is_type_indexable(&array_type) {
                    throw_parser_error(src, index_markers, ErrType::NotIndexable(&array_type));
                }
                // Get the id of the source array/string (may be a nested GetIndex)
                let id = get_id(
                    array,
                    v,
                    ctx,
                    state,
                    &mut output,
                    None,
                    false,
                    offset,
                    single_run,
                );

                let final_id = get_id(
                    index,
                    v,
                    ctx,
                    state,
                    &mut output,
                    None,
                    false,
                    offset,
                    single_run,
                );

                let elem_type = infer_type(value, v, state.fns, src, state.dyn_libs);
                let elem_id = get_id(
                    value,
                    v,
                    ctx,
                    state,
                    &mut output,
                    None,
                    false,
                    offset,
                    single_run,
                );
                free_register(elem_id, state.free_registers, v, state.const_registers);
                if {
                    if let DataType::Array(Some(array_type)) = &array_type
                        && array_type.as_ref() != &elem_type
                    {
                        true
                    } else {
                        false
                    }
                } || (array_type == DataType::String && elem_type != DataType::String)
                {
                    throw_parser_error(
                        src,
                        elem_markers,
                        ErrType::CannotPushTypeToArray(&elem_type, &array_type),
                    );
                }

                let to_push = if array_type == DataType::String {
                    Instr::SetElementString(id, elem_id, final_id)
                } else {
                    Instr::SetElementArray(id, elem_id, final_id)
                };
                state
                    .instr_src
                    .push((to_push, *index_markers, current_src_file));
                output.push(to_push);
                free_register(id, state.free_registers, v, state.const_registers);
            }
            Expr::Condition(main_condition, code, _) => {
                // get first code limit (after which there are only else(if) blocks)
                let main_code_limit = code
                    .iter()
                    .position(|x| matches!(x, Expr::ElseIfBlock(_, _) | Expr::ElseBlock(_)))
                    .unwrap_or(code.len());

                let condition_blocks_count = code.len() - main_code_limit;
                // Each entry is the list of false-jump instruction indices for one condition block.
                let mut conditional_false_jmp_idxs: Vec<Vec<usize>> =
                    Vec::with_capacity(condition_blocks_count + 1);
                let mut jmp_instr_idx: Vec<usize> = Vec::with_capacity(condition_blocks_count);
                let mut condition_markers: Vec<usize> = Vec::with_capacity(condition_blocks_count);

                // Compile the main condition
                let (true_jump_idxs, false_jump_idxs) = compile_short_circuit_condition(
                    main_condition,
                    v,
                    ctx,
                    state,
                    &mut output,
                    offset,
                    single_run,
                    false,
                );
                conditional_false_jmp_idxs.push(false_jump_idxs);

                // Modify true jump instructions to point to body_start
                let body_start = output.len();
                for j in true_jump_idxs {
                    set_jmp_size(&mut output[j], (body_start - j) as u16);
                }

                let v_len = v.len();
                // parse the main code block
                let cond_code = compile_expr(
                    &code[0..main_code_limit],
                    v,
                    ctx,
                    state,
                    offset + output.len() as u16,
                    single_run,
                );
                v.truncate(v_len);
                output.extend(cond_code);
                if main_code_limit != code.len() {
                    output.push(Instr::Jmp(0));
                    jmp_instr_idx.push(output.len() - 1);
                }

                for elem in &code[main_code_limit..] {
                    if let Expr::ElseIfBlock(condition, code) = elem {
                        condition_markers.push(output.len());
                        let condition_id = get_id(
                            condition,
                            v,
                            ctx,
                            state,
                            &mut output,
                            None,
                            false,
                            offset,
                            single_run,
                        );
                        free_register(condition_id, state.free_registers, v, state.const_registers);
                        add_cmp_false(condition_id, &mut 0, &mut output, false);
                        conditional_false_jmp_idxs.push(vec![output.len() - 1]);
                        let v_len = v.len();
                        let cond_code = compile_expr(
                            code,
                            v,
                            ctx,
                            state,
                            offset + output.len() as u16,
                            single_run,
                        );
                        v.truncate(v_len);
                        output.extend(cond_code);
                        output.push(Instr::Jmp(0));
                        jmp_instr_idx.push(output.len() - 1);
                    } else if let Expr::ElseBlock(code) = elem {
                        condition_markers.push(output.len());
                        let v_len = v.len();
                        let cond_code = compile_expr(
                            code,
                            v,
                            ctx,
                            state,
                            offset + output.len() as u16,
                            single_run,
                        );
                        v.truncate(v_len);
                        output.extend(cond_code);
                    }
                }

                for y in jmp_instr_idx {
                    let diff = output.len() - y;
                    output[y] = Instr::Jmp(diff as u16);
                }
                // Fix all false-jump instructions for each condition block
                for (cm_idx, false_idxs) in conditional_false_jmp_idxs.iter().enumerate() {
                    let target = if cm_idx < condition_markers.len() {
                        condition_markers[cm_idx]
                    } else {
                        output.len()
                    };
                    for &y in false_idxs {
                        set_jmp_size(&mut output[y], (target - y) as u16);
                    }
                }
            }
            Expr::WhileBlock(condition, code) => {
                let output_len_before = output.len();

                let (true_jump_idxs, false_jump_idxs) = compile_short_circuit_condition(
                    condition,
                    v,
                    ctx,
                    state,
                    &mut output,
                    offset,
                    single_run,
                    false,
                );

                let body_start = output.len();
                for j in true_jump_idxs {
                    set_jmp_size(&mut output[j], (body_start - j) as u16);
                }

                // parse the code block, clone the vars to avoid overriding anything
                let v_len = v.len();
                let loop_id = block_id + 1;

                let mut cond_code =
                    compile_expr(code, v, ctx, state, offset + output.len() as u16, false);
                v.truncate(v_len);

                let exit = output.len() + cond_code.len() + 1;
                for j in false_jump_idxs {
                    set_jmp_size(&mut output[j], (exit - j) as u16);
                }

                let cond_len = (output.len() - output_len_before) as u16;
                let body_len = cond_code.len() as u16;
                let len = cond_len + body_len; // full span used by JmpBack
                // Break/Continue offsets are relative to cond_code, so pass body_len+1 (body remaining + JmpBack)
                parse_loop_flow_control(&mut cond_code, loop_id, body_len + 1, false, false);
                output.extend(cond_code);
                output.push(Instr::JmpBack(len));
            }
            Expr::ForLoop(var_name, array_code, markers) => {
                let real_var = var_name.as_str() != "_";

                // parse the array, get its id (the target array is the first Expr in array_code)
                let array = array_code.first().unwrap();
                let code = &array_code[1..];
                let array_type = infer_type(array, v, state.fns, src, state.dyn_libs);
                let array = get_id(
                    array,
                    v,
                    ctx,
                    state,
                    &mut output,
                    None,
                    false,
                    offset,
                    single_run,
                );

                let array_len_id = alloc_register(state.registers, state.free_registers);

                output.push(Instr::CallLibFunc(LibFunc::Len, array, array_len_id));

                // set up the id of the index variable (0..len)
                let index_id = if single_run {
                    state.registers.push(0.into());
                    (state.registers.len() - 1) as u16
                } else {
                    let id = alloc_register(state.registers, state.free_registers);
                    output.push(Instr::SetInt(id, 0));
                    id
                };

                // do the 'i < len' condition, set up the condition's id (true/false)
                let condition_id = alloc_register(state.registers, state.free_registers);

                output.push(Instr::InfInt(index_id, array_len_id, condition_id));

                // set up the variable for the current element (for current_element_id in ... {}) => current_element_id = array[index]
                let current_element_id = if real_var {
                    alloc_register(state.registers, state.free_registers)
                } else {
                    0
                };

                let v_len = v.len();

                let is_str = array_type == DataType::String;

                if real_var {
                    v.push(Variable {
                        name: var_name.clone(),
                        register_id: current_element_id,
                        var_type: match array_type {
                            DataType::String => DataType::String,
                            DataType::Array(a_type) => a_type.map_or(DataType::Null, |t| *t),
                            t => throw_parser_error(src, markers, ErrType::IsNotAnIterator(&t)),
                        },
                    });
                }
                let loop_id = block_id + 1;

                // accounts for the GetIndexArray/GetIndexString instruction
                let pending = if real_var { 1 } else { 0 };

                let regs_before = state.registers.len() as u16;
                let mut cond_code = compile_expr(
                    code,
                    v,
                    ctx,
                    state,
                    offset + output.len() as u16 + pending,
                    false,
                );
                // Clean up variables
                v.truncate(v_len);
                free_loop_scope_registers(
                    regs_before,
                    &cond_code,
                    state.free_registers,
                    v,
                    state.const_registers,
                );

                // add the condition ('i < len') jumping logic
                let mut len = (cond_code.len() + 3) as u16 + pending;
                add_cmp_false(condition_id, &mut len, &mut output, true);

                // make the current_element_id register actually hold the element's value
                if real_var {
                    if is_str {
                        output.push(Instr::GetIndexString(array, index_id, current_element_id));
                    } else {
                        output.push(Instr::GetIndexArray(array, index_id, current_element_id));
                    }
                }
                parse_loop_flow_control(&mut cond_code, loop_id, len, true, false);
                // then add the condition code
                output.extend(cond_code);
                // add 1 to the index (i+=1) so that the next loop iteration will have the next element in the array
                output.push(Instr::IncInt(index_id));

                // jump back to the loop if still inside of it
                output.push(Instr::JmpBack(len));

                if single_run {
                    free_register(array_len_id, state.free_registers, v, state.const_registers);
                    free_register(index_id, state.free_registers, v, state.const_registers);
                    free_register(condition_id, state.free_registers, v, state.const_registers);
                    if real_var {
                        free_register(
                            current_element_id,
                            state.free_registers,
                            v,
                            state.const_registers,
                        );
                    }
                }
            }
            Expr::IntForLoop(var_name, start_elem, end_elem, code, markers1, markers2) => {
                // IntForLoop is compiled to:
                // ----
                // (1) if i >= end_elem jump out
                // (2) loop_body
                // (3) i += 1
                // (4) if i < end_elem jump back to body
                // ----
                //
                //
                // Check start and elem type
                let t1 = infer_type(start_elem, v, state.fns, src, state.dyn_libs);
                let t2 = infer_type(end_elem, v, state.fns, src, state.dyn_libs);
                if t1 != DataType::Int {
                    throw_parser_error(src, markers1, ErrType::InvalidType(DataType::Int, &t1));
                }
                if t2 != DataType::Int {
                    throw_parser_error(src, markers2, ErrType::InvalidType(DataType::Int, &t2));
                }
                let elem_id = if single_run {
                    get_id(
                        start_elem,
                        v,
                        ctx,
                        state,
                        &mut output,
                        None,
                        false,
                        offset,
                        single_run,
                    )
                } else {
                    let start_elem_id = get_id(
                        start_elem,
                        v,
                        ctx,
                        state,
                        &mut output,
                        None,
                        false,
                        offset,
                        single_run,
                    );
                    let start_val = state.registers[start_elem_id as usize];
                    let elem_id = alloc_register(state.registers, state.free_registers);
                    if state.const_registers.values().any(|&v| v == start_elem_id)
                        && start_val.is_int()
                    {
                        output.push(Instr::SetInt(elem_id, start_val.as_int()));
                    } else {
                        output.push(Instr::Mov(start_elem_id, elem_id));
                    }
                    elem_id
                };
                let end_elem_id = get_id(
                    end_elem,
                    v,
                    ctx,
                    state,
                    &mut output,
                    None,
                    false,
                    offset,
                    single_run,
                );

                // elem_id is a fresh mutable register -> remove from const_registers just in case
                state.const_registers.retain(|_, &mut v| v != elem_id);

                let v_len = v.len();
                v.push(Variable {
                    name: var_name.clone(),
                    register_id: elem_id,
                    var_type: DataType::Int,
                });
                let loop_id = block_id + 1;

                // (1) if i >= end_elem jump out -> push placeholder first so that compile_expr sees the correct offset
                let jmp_idx = output.len();
                output.push(Instr::SupEqIntJmp(elem_id, end_elem_id, 0));

                let regs_before = state.registers.len() as u16;
                let compiled_loop_code =
                    compile_expr(code, v, ctx, state, offset + output.len() as u16, false);
                free_loop_scope_registers(
                    regs_before,
                    &compiled_loop_code,
                    state.free_registers,
                    v,
                    state.const_registers,
                );
                let compiled_loop_code_len = compiled_loop_code.len() as u16;

                // (2) loop_body
                output.extend(compiled_loop_code);

                // (3) i+= 1
                output.push(Instr::IncInt(elem_id));

                // (4) if i < end_elem jump back to body
                output.push(Instr::InfIntJmpBack(
                    elem_id,
                    end_elem_id,
                    compiled_loop_code_len + 1,
                ));

                let exit_size = (output.len() - jmp_idx) as u16;
                output[jmp_idx] = Instr::SupEqIntJmp(elem_id, end_elem_id, exit_size);

                parse_loop_flow_control(
                    &mut output[jmp_idx + 1..],
                    loop_id,
                    exit_size,
                    true,
                    false,
                );
                v.truncate(v_len);

                if single_run {
                    free_register(end_elem_id, state.free_registers, v, state.const_registers);
                    free_register(elem_id, state.free_registers, v, state.const_registers);
                }
            }
            Expr::LoopBlock(code) => {
                let loop_id = block_id + 1;
                let v_len = v.len();
                let regs_before = state.registers.len() as u16;
                let mut compiled = compile_expr(code, v, ctx, state, output.len() as u16, false);
                v.truncate(v_len);
                free_loop_scope_registers(
                    regs_before,
                    &compiled,
                    state.free_registers,
                    v,
                    state.const_registers,
                );
                let code_length = compiled.len() as u16;
                parse_loop_flow_control(&mut compiled, loop_id, code_length + 1, false, true);
                output.extend(compiled);
                output.push(Instr::JmpBack(code_length));
            }
            Expr::VarDeclare(x, y) => {
                let var_type = infer_type(y, v, state.fns, src, state.dyn_libs);

                let var_id = if single_run {
                    get_id(
                        y,
                        v,
                        ctx,
                        state,
                        &mut output,
                        None,
                        true,
                        offset,
                        single_run,
                    )
                } else {
                    let src_id = get_id(
                        y,
                        v,
                        ctx,
                        state,
                        &mut output,
                        None,
                        false,
                        offset,
                        single_run,
                    );
                    if contains_var_reassign(x, &input[idx + 1..]) {
                        let mutable_id = alloc_register(state.registers, state.free_registers);
                        move_reg_to_reg(
                            &mut output,
                            src_id,
                            mutable_id,
                            state.registers[src_id as usize],
                        );
                        mutable_id
                    } else {
                        src_id
                    }
                };

                v.push(Variable {
                    name: x.clone(),
                    register_id: var_id,
                    var_type,
                });
            }
            Expr::VarAssign(name, y, markers) => {
                let var_type = infer_type(y, v, state.fns, src, state.dyn_libs);
                let var_pos = v.iter().rposition(|x| x.name == *name).unwrap_or_else(|| {
                    throw_parser_error(src, markers, ErrType::UnknownVariable(name));
                });
                let id = v[var_pos].register_id;

                if var_type == DataType::Int {
                    // (is_inc, src_var_name)
                    let inc_dec: Option<(bool, &str)> = match y.as_ref() {
                        // var+1/1+var use the dedicated IncInt/IncIntTo instructions
                        Expr::Add(l, r, _) => {
                            let src = if matches!(r.as_ref(), Expr::Int(1)) {
                                Some(l.as_ref())
                            } else if matches!(l.as_ref(), Expr::Int(1)) {
                                Some(r.as_ref())
                            } else {
                                None
                            };
                            src.and_then(|e| {
                                if let Expr::Var(src_name, _) = e {
                                    v.iter()
                                        .rfind(|x| x.name == *src_name)
                                        .filter(|x| x.var_type == DataType::Int)
                                        .map(|_| (true, src_name.as_str()))
                                } else {
                                    None
                                }
                            })
                        }
                        // var-1 uses the dedicated DecInt/DecIntTo instructions
                        Expr::Sub(l, r, _) => {
                            if matches!(r.as_ref(), Expr::Int(1)) {
                                if let Expr::Var(src_name, _) = l.as_ref() {
                                    v.iter()
                                        .rfind(|x| x.name == *src_name)
                                        .filter(|x| x.var_type == DataType::Int)
                                        .map(|_| (false, src_name.as_str()))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    if let Some((is_inc, src_name)) = inc_dec {
                        let src_id = v.iter().rfind(|x| x.name == src_name).unwrap().register_id;
                        output.push(if src_id == id {
                            if is_inc {
                                Instr::IncInt(id)
                            } else {
                                Instr::DecInt(id)
                            }
                        } else {
                            if is_inc {
                                Instr::IncIntTo(src_id, id)
                            } else {
                                Instr::DecIntTo(src_id, id)
                            }
                        });
                        continue;
                    }
                }

                let output_len = output.len();
                let obj_id = get_id(
                    y,
                    v,
                    ctx,
                    state,
                    &mut output,
                    Some(id),
                    false,
                    offset,
                    single_run,
                );
                if output.len() != output_len {
                    move_to_id(&mut output, id);
                } else if state.const_registers.values().any(|&v| v == obj_id) {
                    move_reg_to_reg(&mut output, obj_id, id, state.registers[obj_id as usize]);
                } else {
                    output.push(Instr::Mov(obj_id, id));
                }
                if is_reg_free(v, obj_id, name) {
                    free_register(obj_id, state.free_registers, v, state.const_registers);
                }
                v[var_pos].var_type = var_type;
            }

            Expr::FunctionCall(args, namespace, markers, args_indexes) => {
                let output_id = handle_functions(
                    &mut output,
                    v,
                    ctx,
                    state,
                    args,
                    namespace,
                    markers,
                    args_indexes,
                    offset,
                    single_run,
                );
                if let Some(id) = output_id {
                    free_register(id, state.free_registers, v, state.const_registers);
                }
            }
            Expr::ObjFunctionCall(obj, args, namespace, obj_markers, fn_markers, args_indexes) => {
                handle_method_calls(
                    &mut output,
                    v,
                    ctx,
                    state,
                    obj,
                    args,
                    namespace,
                    obj_markers,
                    fn_markers,
                    args_indexes,
                    offset,
                    single_run,
                );
            }
            Expr::FunctionDecl(x, y, markers) => {
                if state
                    .fns
                    .iter()
                    .any(|func| &func.name == x.first().unwrap())
                {
                    throw_parser_error(src, markers, ErrType::FunctionAlreadyExists(&x[0]));
                }
                state.fns.push(Function {
                    name: x.first().unwrap().clone(),
                    args: x.into_iter().skip(1).cloned().collect(),
                    code: y.clone(),
                    impls: Vec::new(),
                    is_recursive: contains_recursive_call(y, x.first().unwrap()),
                    id: state.fn_registers.len() as u16,
                    returns_void: check_if_returns_void(y),
                    src_file: current_src_file,
                    return_type_cache: Vec::new(),
                });
                state.fn_registers.push(Vec::new());
            }
            Expr::ReturnVal(val) => {
                if let Some(x) = &**val {
                    let id = get_id(
                        x,
                        v,
                        ctx,
                        state,
                        &mut output,
                        None,
                        false,
                        offset,
                        single_run,
                    );
                    if is_parsing_recursive {
                        output.push(Instr::RecursiveReturn(id));
                    } else {
                        output.push(Instr::Return(id));
                    }
                }
            }
            Expr::Break => output.push(Instr::NotEqJmp(block_id + 1, 0, 0)),
            Expr::Continue => output.push(Instr::EqJmp(block_id + 1, 0, 0)),
            Expr::EvalBlock(code) => {
                let v_len = v.len();
                output.extend(compile_expr(
                    code,
                    v,
                    ctx,
                    state,
                    output.len() as u16,
                    single_run,
                ));
                v.truncate(v_len);
            }
            other => {
                get_id(
                    other,
                    v,
                    ctx,
                    state,
                    &mut output,
                    None,
                    false,
                    offset,
                    single_run,
                );
            }
        }
    }
    output
}

#[cfg(target_os = "macos")]
const DYLIB_EXT: &str = "dylib";
#[cfg(target_os = "linux")]
const DYLIB_EXT: &str = "so";
#[cfg(target_os = "windows")]
const DYLIB_EXT: &str = "dll";

/// Recursively collects functions, dyn libs, and imported files
fn parse_toplevel(
    code: Vec<Expr>,
    file_path: &Path,
    src_file_idx: u16,
    use_line_markers: (&str, &str),
    fns: &mut Vec<Function>,
    fn_registers: &mut Vec<Vec<u16>>,
    dyn_libs: &mut Vec<Dynamiclib>,
    dyn_lib_fns: &mut Vec<DynamicLibFn>,
    sources: &mut Vec<(SmolStr, String)>,
    visited_files: &mut HashSet<PathBuf>,
    dyn_fn_id: &mut u16,
) {
    for expr in code {
        match expr {
            Expr::FunctionDecl(namespace, fn_code, markers) => {
                if let Some(func) = fns.iter().find(|f| f.name == namespace[0]) {
                    let func_file = &sources[func.src_file as usize].0;
                    throw_parser_error(
                        use_line_markers,
                        &markers,
                        ErrType::DuplicateFunctionInImport(&namespace[0], func_file.as_str()),
                    );
                }
                fn_registers.push(Vec::new());
                let is_recursive = contains_recursive_call(&fn_code, &namespace[0]);
                let returns_void = check_if_returns_void(&fn_code);
                fns.push(Function {
                    name: namespace[0].to_smolstr(),
                    args: namespace[1..].into(),
                    code: fn_code,
                    impls: Vec::new(),
                    is_recursive,
                    id: (fn_registers.len() - 1) as u16,
                    returns_void,
                    src_file: src_file_idx,
                    return_type_cache: Vec::new(),
                });
            }
            #[cfg(target_arch = "wasm32")]
            Expr::ImportDylib(_, _, _) => {
                wasm_error("WASM does not support loading dynamic libraries")
            }
            #[cfg(not(target_arch = "wasm32"))]
            Expr::ImportDylib(path, fn_signatures, markers) => {
                let fns = fn_signatures
                        .iter()
                        .map(|(fn_name, fn_args, fn_return_type)| {
                            let return_val = FnSignature {
                                name: fn_name.clone(),
                                args: fn_args.clone(),
                                return_type: fn_return_type.clone(),
                                id: *dyn_fn_id,
                            };
                            let arg_types: Vec<_> = fn_args.iter().map(datatype_to_c_type).collect();
                            let return_type = datatype_to_c_type(fn_return_type);
                            let cif = libffi::middle::Cif::new(arg_types.clone(), return_type.clone());
                            // If the extension is omitted, the extension is chosen based on the target OS
                            let path = if std::path::Path::new(path.as_str()).extension().is_none() {
                                format_args!("{path}.{DYLIB_EXT}").to_smolstr()
                            } else {
                                path.clone()
                            };
                            let lib = unsafe {
                                libloading::Library::new(path.as_str()).unwrap_or_else(|e| {
                                    throw_parser_error(
                                        use_line_markers,
                                        &markers,
                                        ErrType::Custom(
                                            format_args!(
                                                "Cannot load dynamic library \"{path}\": {e}"
                                            )
                                            .to_smolstr(),
                                        ),
                                    )
                                })
                            };
                            let ptr = unsafe {
                                libffi::middle::CodePtr(
                                    lib.get::<*const ()>(fn_name.as_bytes())
                                        .unwrap_or_else(|e| {
                                            throw_parser_error(
                                                use_line_markers,
                                                &markers,
                                                ErrType::Custom(
                                                    format_args!(
                                                        "Cannot find symbol \"{fn_name}\" in \"{path}\": {e}"
                                                    )
                                                    .to_smolstr(),
                                                ),
                                            )
                                        })
                                        .try_as_raw_ptr()
                                        .unwrap(),
                                )
                            };

                            let mut types = vec![fn_return_type.clone()];
                            types.extend(fn_args.clone());

                            dyn_lib_fns.push(DynamicLibFn {
                                types: Box::from(types),
                                _lib: lib,
                                ptr,
                                cif,
                            });
                            *dyn_fn_id += 1;
                            return_val
                        })
                        .collect();
                dyn_libs.push(Dynamiclib {
                    name: std::path::PathBuf::from(path.as_str())
                        .file_prefix()
                        .and_then(|s| s.to_str())
                        .unwrap_or(path.as_str())
                        .to_smolstr(),
                    fns,
                });
            }
            #[cfg(target_arch = "wasm32")]
            Expr::ImportFile(_, _) => wasm_error("WASM does not support importing files"),
            #[cfg(not(target_arch = "wasm32"))]
            Expr::ImportFile(path, markers) => {
                let file_path = file_path
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join(path.as_str())
                    .canonicalize()
                    .unwrap_or_else(|_| {
                        let current_src = &sources[src_file_idx as usize];
                        throw_parser_error(
                            (current_src.0.as_str(), current_src.1.as_str()),
                            &markers,
                            ErrType::CannotReadImportedFile(path.as_str()),
                        );
                    });
                if visited_files.contains(&file_path) {
                    let current_src = &sources[src_file_idx as usize];
                    throw_parser_error(
                        (current_src.0.as_str(), current_src.1.as_str()),
                        &markers,
                        ErrType::CircularImport(file_path.to_str().unwrap_or(path.as_str())),
                    );
                }
                visited_files.insert(file_path.clone());
                let file_contents = std::fs::read_to_string(&file_path).unwrap_or_else(|_| {
                    let current_src = &sources[src_file_idx as usize];
                    throw_parser_error(
                        (current_src.0.as_str(), current_src.1.as_str()),
                        &markers,
                        ErrType::CannotReadImportedFile(path.as_str()),
                    );
                });
                let file_name: SmolStr = file_path.to_str().unwrap_or(path.as_str()).into();
                sources.push((file_name.clone(), file_contents.clone()));

                // Parse the imported file's contents
                let file_code: Vec<Expr> = grammar::FileParser::new()
                    .parse(
                        (file_name.as_str(), file_contents.as_str()),
                        file_contents.as_str(),
                    )
                    .unwrap_or_else(|x| {
                        lalrpop_error::<Token<'_>>(x, file_contents.as_str(), file_name.as_str())
                    });

                let import_src: (&str, &str) = (file_name.as_str(), file_contents.as_str());
                parse_toplevel(
                    file_code,
                    &file_path,
                    (sources.len() - 1) as u16,
                    import_src,
                    fns,
                    fn_registers,
                    dyn_libs,
                    dyn_lib_fns,
                    sources,
                    visited_files,
                    dyn_fn_id,
                );
            }
            _ => {}
        }
    }
}

pub fn parse(
    contents: &str,
    filename: &str,
    debug: bool,
) -> (
    Vec<Instr>,
    Vec<Data>,
    Pools,
    Vec<(Instr, Span, u16)>,
    Vec<Vec<u16>>,
    Vec<DynamicLibFn>,
    usize,
    usize,
    Vec<(SmolStr, String)>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    let now = std::time::Instant::now();

    let code: Vec<Expr> = grammar::FileParser::new()
        .parse((filename, contents), contents)
        .unwrap_or_else(|x| lalrpop_error::<Token<'_>>(x, contents, filename));

    #[cfg(not(target_arch = "wasm32"))]
    if debug {
        println!("PARSING TIME: {:.2?}", now.elapsed());
    }

    let mut variables: Vec<Variable> = Vec::new();
    let mut registers: Vec<Data> = Vec::new();
    let mut pools: Pools = Pools {
        array_pool: Vec::with_capacity(20),
        string_pool: Vec::with_capacity(20),
    };
    let mut instr_src: Vec<(Instr, Span, u16)> = Vec::new();
    let mut fn_registers: Vec<Vec<u16>> = Vec::new();
    let mut functions: Vec<Function> = Vec::new();
    let mut dyn_libs: Vec<Dynamiclib> = Vec::new();
    let mut dyn_fn_id: u16 = 0;
    let mut dyn_lib_fns: Vec<DynamicLibFn> = Vec::new();
    let mut allocated_arg_count = 0;
    let mut allocated_call_depth = 0;
    let mut const_registers = HashMap::new();
    let mut free_registers = Vec::new();

    // sources[0] = main file
    let mut sources: Vec<(SmolStr, String)> = vec![(SmolStr::from(filename), contents.to_string())];
    let main_path = PathBuf::from(filename)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(filename));
    let mut visited: HashSet<PathBuf> = HashSet::new();
    visited.insert(main_path.clone());

    let main_src: (&str, &str) = (filename, contents);
    parse_toplevel(
        code,
        &main_path,
        0,
        main_src,
        &mut functions,
        &mut fn_registers,
        &mut dyn_libs,
        &mut dyn_lib_fns,
        &mut sources,
        &mut visited,
        &mut dyn_fn_id,
    );

    // Detect mutual / indirect recursion cycles
    mark_mutually_recursive(&mut functions);

    let ctx = Ctx {
        block_id: 0,
        src: (filename, contents),
        is_parsing_recursive: false,
        current_src_file: 0,
    };
    let mut state = State {
        registers: &mut registers,
        fns: &mut functions,
        pools: &mut pools,
        instr_src: &mut instr_src,
        fn_registers: &mut fn_registers,
        dyn_libs: &mut dyn_libs,
        allocated_arg_count: &mut allocated_arg_count,
        allocated_call_depth: &mut allocated_call_depth,
        const_registers: &mut const_registers,
        free_registers: &mut free_registers,
        sources: &mut sources,
    };
    let mut instructions = compile_expr(
        &state.fns
            .iter()
            .find(|func| func.name == "main")
            .unwrap_or_else(|| {
                #[cfg(target_arch = "wasm32")]
                wasm_error("Cannot find main function");

                eprintln!(
                    "--------------\n{color_red}KEEL RUNTIME ERROR:{color_reset}\nCannot find {color_bright_blue}{style_bold}main{style_reset}{color_reset} function\n--------------",
                );
                std::process::exit(1);
            })
            .code
            .clone(),
        &mut variables,
        ctx,
        &mut state,
        0,
        true
    );
    instructions.push(Instr::Halt(0));
    for x in fn_registers.iter_mut() {
        x.sort();
        x.dedup();
    }
    if debug {
        crate::display::print_debug(&instructions, &registers, &pools);
    }
    (
        instructions,
        registers,
        pools,
        instr_src,
        fn_registers,
        dyn_lib_fns,
        allocated_arg_count,
        allocated_call_depth,
        sources,
    )
}
