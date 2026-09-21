use crate::compiler::compiler_data::InstrSrc;
use crate::compiler::compiler_data::Source;
use crate::compiler::compiler_data::StructField;
use crate::compiler::compiler_errors::error_cannot_find_dynlib_symbol;
use crate::compiler::compiler_errors::error_cannot_load_dynlib;
use crate::compiler::compiler_errors::error_cannot_push_type_to_array;
use crate::compiler::compiler_errors::error_cannot_read_file;
use crate::compiler::compiler_errors::error_division_by_zero;
use crate::compiler::compiler_errors::error_duplicate_map_key;
use crate::compiler::compiler_errors::error_global_already_defined;
use crate::compiler::compiler_errors::error_invalid_index_type;
use crate::compiler::compiler_errors::error_invalid_type;
use crate::compiler::compiler_errors::error_map_diff_types;
use crate::compiler::compiler_errors::error_not_literal_map_key;
use crate::compiler::compiler_errors::error_range_invalid_type;
use crate::compiler::compiler_errors::error_struct_already_defined;
use crate::compiler::compiler_errors::error_type_not_indexable;
use crate::compiler::compiler_errors::error_unknown_namespace;
#[cfg(not(target_arch = "wasm32"))]
use crate::compiler::expr::DylibFnExpr;
#[cfg(not(target_arch = "wasm32"))]
use crate::compiler::expr::DylibImportExpr;
use crate::compiler::expr::FunctionDeclarationArgumentExpr;
use crate::compiler::expr::FunctionDeclarationExpr;
use crate::compiler::expr::IfBlockExpr;
use crate::compiler::expr::IntForLoopExpr;
use crate::compiler::expr::MatchArm;
use crate::compiler::expr::MatchExpr;
use crate::compiler::expr::Pattern;
use crate::compiler::expr::PatternConstructor;
use crate::compiler::expr::QualifiedName;
use crate::compiler::expr::StructFieldAssignmentExpr;
use crate::compiler::expr::StructFieldExpr;
use crate::compiler::expr::VariableDeclarationExpr;
use crate::compiler::functions::user_functions::compile_function_impl;
use crate::compiler::registers::move_value_to;
use crate::compiler::type_system::var_type_is_compatible;
use crate::data::FALSE;
use crate::data::NULL;
use crate::data::TRUE;
use crate::errors::BLUE;
use crate::errors::BOLD;
use crate::errors::ErrorCtx;
use crate::errors::RED;
use crate::errors::RESET;
use crate::hformat;
use crate::instr::LibFunc;
use crate::parser;
use crate::vm::Pool;
use crate::vm::RegisterFile;
use crate::{data::Data, instr::Instr};
use bumpalo::Bump;
use compiler_data::Ctx;
use compiler_data::Dylib;
use compiler_data::DylibFn;
use compiler_data::FnSignature;
use compiler_data::Function;
use compiler_data::Pools;
use compiler_data::State;
use compiler_data::Struct;
use compiler_data::Variable;
use expr::Expr;
use expr::Span;
use expr::code_modifies_variable;
use fixedbitset::FixedBitSet;
use functions::compile_function_call;
use indexmap::IndexMap;
use methods::compile_method_call;
use registers::move_reg_to_reg;
use rustc_hash::FxBuildHasher;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use std::cell::LazyCell;
use std::collections::HashMap;
use std::hint::unreachable_unchecked;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use type_system::DataType;
use type_system::TypeExpr;
use type_system::check_if_returns_void;
use type_system::collect_direct_fn_calls;
use type_system::struct_field_type_matches;

#[cfg(not(target_arch = "wasm32"))]
use libloading::Library;

#[cfg(target_arch = "wasm32")]
use crate::errors::wasm_error;
pub mod compiler_data;
mod compiler_errors;
pub mod type_system;

pub mod expr;

#[path = "functions/functions.rs"]
mod functions;
#[path = "functions/methods.rs"]
mod methods;

mod registers;

pub trait UnwrapId {
    fn unwrap_id(self) -> u16;
}

impl UnwrapId for Option<u16> {
    #[inline(always)]
    fn unwrap_id(self) -> u16 {
        debug_assert!(self.is_some());
        unsafe { self.unwrap_unchecked() }
    }
}

/// Fuses the last comparison instruction into a jump instruction (jumps when condition is false)
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
        Instr::ObjEq(o1, o2, o3) if o3 == condition_id => Instr::ObjNotEqJmp(o1, o2, *len),
        Instr::StrEq(o1, o2, o3) if o3 == condition_id => Instr::StrNotEqJmp(o1, o2, *len),
        Instr::NotEq(o1, o2, o3) if o3 == condition_id => Instr::EqJmp(o1, o2, *len),
        Instr::ObjNotEq(o1, o2, o3) if o3 == condition_id => Instr::ObjEqJmp(o1, o2, *len),
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
        Instr::ObjEq(o1, o2, o3) if o3 == condition_id => Instr::ObjEqJmp(o1, o2, 0),
        Instr::StrEq(o1, o2, o3) if o3 == condition_id => Instr::StrEqJmp(o1, o2, 0),
        Instr::NotEq(o1, o2, o3) if o3 == condition_id => Instr::NotEqJmp(o1, o2, 0),
        Instr::ObjNotEq(o1, o2, o3) if o3 == condition_id => Instr::ObjNotEqJmp(o1, o2, 0),
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
const fn set_jmp_size(instr: &mut Instr, size: u16) {
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
        | Instr::ObjNotEqJmp(_, _, jump_size)
        | Instr::ObjEqJmp(_, _, jump_size)
        | Instr::StrNotEqJmp(_, _, jump_size)
        | Instr::StrEqJmp(_, _, jump_size) => *jump_size = size,
        _ => unsafe { unreachable_unchecked() },
    }
}

/// Compiles short-circuit && and || conditions
/// bool_or_mode true indicates left side of ||, emits true jumps
/// bool_or_mode false emits false jumps
/// Returns (true_jump_idxs, false_jump_idxs)
#[must_use]
fn compile_short_circuit_condition<'arena>(
    condition: &'arena Expr,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
    bool_or_mode: bool,
) -> (Vec<usize>, Vec<usize>) {
    match condition {
        Expr::BoolOr(left, right, _, _) => {
            // left side of || always uses true jump mode
            let (mut true_jumps, _) =
                compile_short_circuit_condition(left, ctx, state, output, true);
            let (right_true, right_false) =
                compile_short_circuit_condition(right, ctx, state, output, bool_or_mode);
            true_jumps.extend(right_true);
            (true_jumps, right_false)
        }
        Expr::BoolAnd(left, right, _, _) => {
            if bool_or_mode {
                // && inside left side of ||
                let id_l = left.compile(ctx, state, output, None, false, true).unwrap_id();
                let id_r = right.compile(ctx, state, output, None, false, true).unwrap_id();
                state.free_reg(id_l);
                state.free_reg(id_r);
                let id = state.alloc_reg();
                output.push(Instr::BoolAnd(id_l, id_r, id));
                add_cmp_true(id, output);
                state.free_reg(id);
                (vec![output.len() - 1], Vec::new())
            } else {
                // normal && -> if either side is false, jump past the body
                let (_, mut false_jumps) =
                    compile_short_circuit_condition(left, ctx, state, output, false);
                let (_, right_false) =
                    compile_short_circuit_condition(right, ctx, state, output, false);
                false_jumps.extend(right_false);
                (Vec::new(), false_jumps)
            }
        }
        expr => {
            let cond_id = expr.compile(ctx, state, output, None, false, true).unwrap_id();
            if bool_or_mode {
                add_cmp_true(cond_id, output);
                state.free_reg(cond_id);
                (vec![output.len() - 1], Vec::new())
            } else {
                add_cmp_false(cond_id, &mut 0, output, false);
                state.free_reg(cond_id);
                (Vec::new(), vec![output.len() - 1])
            }
        }
    }
}

#[must_use]
fn compile_const_condition(condition: &Expr) -> Option<bool> {
    match condition {
        Expr::Bool(b) => Some(*b),
        Expr::BoolNeg(b, _, _) => compile_const_condition(b).map(|b| !b),
        _ => None,
    }
}

#[must_use]
fn compile_condition<'arena>(
    condition: &'arena Expr,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> (Option<bool>, Vec<usize>, Vec<usize>) {
    if let Some(b) = compile_const_condition(condition) {
        return (Some(b), Vec::new(), Vec::new());
    }

    if matches!(condition, Expr::BoolAnd(_, _, _, _) | Expr::BoolOr(_, _, _, _)) {
        let (true_jump_idxs, false_jmp_idxes) =
            compile_short_circuit_condition(condition, ctx, state, output, false);
        return (None, true_jump_idxs, false_jmp_idxes);
    }

    let cond_id = condition.compile(ctx, state, output, None, false, true).unwrap_id();
    if state.const_registers.get(&TRUE) == Some(&cond_id) {
        return (Some(true), Vec::new(), Vec::new());
    } else if state.const_registers.get(&FALSE) == Some(&cond_id) {
        return (Some(false), Vec::new(), Vec::new());
    }
    add_cmp_false(cond_id, &mut 0, output, false);
    state.free_reg(cond_id);
    (None, Vec::new(), vec![output.len() - 1])
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

#[must_use]
fn compile_array_literal<'arena>(
    array_items: &'arena [Expr],
    spans: &[Span],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    if let Some(first) = array_items.first() {
        let first_type = first.infer_type(ctx, state);
        if let Some(failing_elem_idx) =
            array_items.iter().skip(1).position(|x| x.infer_type(ctx, state) != first_type)
        {
            let failing_elem_type = array_items[failing_elem_idx + 1].infer_type(ctx, state);
            let failing_elem_span = spans[failing_elem_idx + 2];
            compiler_errors::error_array_diff_types(
                ctx.file_idx,
                state.sources,
                spans[1],
                &first_type,
                failing_elem_span,
                &failing_elem_type,
            )
        }
    }
    let array_id = {
        state.pools.obj_pool.push(Vec::with_capacity(array_items.len()));
        state.pools.obj_pool.len() - 1
    };
    if array_items.is_empty() && !ctx.single_run {
        let array_reg = state.new_reg(Data::array(array_id as u32));
        output.push(Instr::EmptyArray(array_reg));
        return array_reg;
    }
    if ctx.single_run {
        for elem in array_items {
            let id = elem.compile(ctx, state, output, None, false, true).unwrap_id();
            if elem.is_constant_literal() {
                state.pools.obj_pool.get_mut(array_id).push(state.registers[id as usize]);
            } else {
                output.push(Instr::ObjElemMov(
                    id,
                    array_id as u16,
                    state.pools.obj_pool[array_id].len() as u16,
                ));
                state.pools.obj_pool.get_mut(array_id).push(NULL);
            }
        }
        state.new_reg(Data::array(array_id as u32))
    } else {
        // Check if all elements are constant (no instructions emitted)
        let mut constant_array = true;
        let mut elem_ids: Vec<u16> = Vec::with_capacity(array_items.len());
        for elem in array_items {
            let id = elem.compile(ctx, state, output, None, false, true).unwrap_id();
            if elem.is_constant_literal() {
                state.pools.obj_pool.get_mut(array_id).push(state.registers[id as usize]);
            } else {
                constant_array = false;
                state.pools.obj_pool.get_mut(array_id).push(NULL);
            }
            elem_ids.push(id);
        }

        if constant_array {
            // The template array is held by a register to prevent it from being freed by the GC
            let template_reg = state.new_reg(Data::array(array_id as u32));
            let dest_reg = state.new_reg(Data::array(0)); // 0 is a placeholder that's overwritten by EmptyArray

            output.push(Instr::CloneArray(
                template_reg,
                dest_reg,
                state.pools.obj_pool[array_id].len() as u16,
            ));
            dest_reg
        } else {
            let dest_reg = state.new_reg(Data::array(0)); // 0 is a placeholder that's overwritten by EmptyArray
            output.push(Instr::EmptyArray(dest_reg));
            for elem_reg in elem_ids {
                output.push(Instr::Push(dest_reg, elem_reg));
            }
            dest_reg
        }
    }
}

#[must_use]
fn compile_struct_literal<'arena>(
    name: &'arena QualifiedName,
    fields: &'arena [StructFieldExpr],
    span: Span,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let struct_name = name.get_name();
    let namespace = name.get_namespace();
    let Some(expected_struct_idx) = state.scope(ctx.file_idx).find_struct(
        namespace,
        struct_name,
        span,
        ctx.file_idx,
        state.sources,
    ) else {
        compiler_errors::error_unknown_struct(struct_name, span, state.sources, ctx.file_idx);
    };
    let type_id = state.structs[expected_struct_idx].id;
    let expected_fields_len = state.structs[expected_struct_idx].fields.len();
    if expected_fields_len < fields.len() {
        let unexpected_field = &fields[expected_fields_len];
        compiler_errors::error_struct_no_such_field(
            ctx.file_idx,
            struct_name,
            state.structs[expected_struct_idx].name_span,
            unexpected_field.name_span,
            unexpected_field.name,
            state.sources,
        )
    }
    let struct_id = {
        state.pools.obj_pool.push(Vec::with_capacity(fields.len()));
        state.pools.obj_pool.len() - 1
    };
    if ctx.single_run {
        for field_idx in 0..expected_fields_len {
            if let Some(StructFieldExpr { name: _, value, name_span: _, value_span }) =
                fields.iter().find(|field| {
                    field.name == state.structs[expected_struct_idx].fields[field_idx].name
                })
            {
                let field_type = value.infer_type(ctx, state);
                let field = &state.structs[expected_struct_idx].fields[field_idx];
                if !struct_field_type_matches(&field.field_type, &field_type) {
                    compiler_errors::error_struct_field_invalid_type(
                        ctx.file_idx,
                        struct_name,
                        field.span,
                        field.name,
                        &field.field_type,
                        *value_span,
                        &field_type,
                        state.sources,
                    );
                }
                let id = value.compile(ctx, state, output, None, false, true).unwrap_id();
                if value.is_constant_literal() {
                    state.pools.obj_pool.get_mut(struct_id).push(state.registers[id as usize]);
                } else {
                    output.push(Instr::ObjElemMov(
                        id,
                        struct_id as u16,
                        state.pools.obj_pool[struct_id].len() as u16,
                    ));
                    state.pools.obj_pool.get_mut(struct_id).push(NULL);
                }
            } else {
                let missing_elems = (0..expected_fields_len)
                    .into_iter()
                    .filter(|i| {
                        !fields.iter().any(|field| {
                            field.name == state.structs[expected_struct_idx].fields[*i].name
                        })
                    })
                    .map(|i| state.structs[struct_id].fields[i].name)
                    .collect::<Vec<&str>>();
                compiler_errors::error_struct_missing_fields(
                    ctx.file_idx,
                    state.structs[expected_struct_idx].name_span,
                    span,
                    state.sources,
                    &missing_elems,
                )
            }
        }

        state.new_reg(Data::struct_instance(type_id, struct_id as u32))
    } else {
        let mut dynamic: Vec<(u16, u16)> = Vec::with_capacity(expected_fields_len);
        for field_idx in 0..expected_fields_len {
            if let Some(StructFieldExpr { name: _, value, name_span: _, value_span }) =
                fields.iter().find(|field| {
                    field.name == state.structs[expected_struct_idx].fields[field_idx].name
                })
            {
                let field_type = value.infer_type(ctx, state);
                let field = &state.structs[expected_struct_idx].fields[field_idx];
                if !struct_field_type_matches(&field.field_type, &field_type) {
                    compiler_errors::error_struct_field_invalid_type(
                        ctx.file_idx,
                        struct_name,
                        field.span,
                        field.name,
                        &field.field_type,
                        *value_span,
                        &field_type,
                        state.sources,
                    );
                }
                let id = value.compile(ctx, state, output, None, false, true).unwrap_id();
                if value.is_constant_literal() {
                    state.pools.obj_pool.get_mut(struct_id).push(state.registers[id as usize]);
                } else {
                    state.pools.obj_pool.get_mut(struct_id).push(NULL);
                    dynamic.push((id, field_idx as u16));
                }
            } else {
                let missing_elems = (0..expected_fields_len)
                    .into_iter()
                    .filter(|i| {
                        !fields.iter().any(|field| {
                            field.name == state.structs[expected_struct_idx].fields[*i].name
                        })
                    })
                    .map(|i| state.structs[struct_id].fields[i].name)
                    .collect::<Vec<&str>>();
                compiler_errors::error_struct_missing_fields(
                    ctx.file_idx,
                    state.structs[expected_struct_idx].name_span,
                    span,
                    state.sources,
                    &missing_elems,
                );
            }
        }

        let template_reg = state.new_reg(Data::struct_instance(type_id, struct_id as u32));

        let dest_reg = state.new_reg(Data::struct_instance(type_id, 0));

        output.push(Instr::CloneStruct(template_reg, dest_reg));
        for (val_reg, slot) in dynamic {
            output.push(Instr::SetFieldStruct(dest_reg, val_reg, slot));
        }
        dest_reg
    }
}

#[must_use]
fn compile_map_literal<'arena>(
    kv_pairs: &'arena [(Expr, Span, Expr, Span)],
    map_span: Span,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let mut global_key_type: DataType = DataType::Unknown;
    let mut global_val_type: DataType = DataType::Unknown;
    let map_id = state.pools.map_pool.len();
    state.pools.map_pool.push(HashMap::with_capacity_and_hasher(kv_pairs.len(), FxBuildHasher));
    if ctx.single_run {
        for (i, (key, key_span, val, val_span)) in kv_pairs.iter().enumerate() {
            if let Some((_, repeat_key_span, _, _)) =
                kv_pairs.iter().skip(i + 1).find(|(k, _, _, _)| k == key)
            {
                error_duplicate_map_key(
                    *key_span,
                    *repeat_key_span,
                    map_span,
                    ctx.file_idx,
                    state.sources,
                );
            }
            let key_t = key.infer_type(ctx, state);
            let val_t = val.infer_type(ctx, state);
            if i == 0 {
                global_key_type = key_t;
                global_val_type = val_t;
            } else {
                if key_t != global_key_type {
                    error_map_diff_types(
                        ctx.file_idx,
                        state.sources,
                        map_span,
                        &global_key_type,
                        *key_span,
                        &key_t,
                    )
                }
                if val_t != global_val_type {
                    error_map_diff_types(
                        ctx.file_idx,
                        state.sources,
                        map_span,
                        &global_val_type,
                        *val_span,
                        &val_t,
                    )
                }
            }
            let output_len = output.len();
            let key_val_id = key.compile(ctx, state, output, None, false, true).unwrap_id();
            if !(key.is_constant_literal()
                || matches!(key, Expr::Array(_, _)) && output_len == output.len())
            {
                error_not_literal_map_key(*key_span, map_span, ctx.file_idx, state.sources);
            }
            let key_val = state.registers[key_val_id as usize];
            let id = val.compile(ctx, state, output, None, false, true).unwrap_id();
            if val.is_constant_literal() {
                state.pools.map_pool[map_id].insert(key_val, state.registers[id as usize]);
            } else {
                state.pools.map_pool[map_id].insert(key_val, NULL);
                output.push(Instr::MapInsert(map_id as u16, state.new_reg(key_val), id));
            }
        }
        state.new_reg(Data::map(map_id as u32))
    } else {
        let mut dynamic: Vec<(Data, u16)> = Vec::with_capacity(kv_pairs.len());
        for (i, (key, key_span, val, val_span)) in kv_pairs.iter().enumerate() {
            if let Some((_, repeat_key_span, _, _)) =
                kv_pairs.iter().skip(i + 1).find(|(k, _, _, _)| k == key)
            {
                error_duplicate_map_key(
                    *key_span,
                    *repeat_key_span,
                    map_span,
                    ctx.file_idx,
                    state.sources,
                );
            }
            let key_t = key.infer_type(ctx, state);
            let val_t = val.infer_type(ctx, state);
            if i == 0 {
                global_key_type = key_t;
                global_val_type = val_t;
            } else {
                if key_t != global_key_type {
                    error_map_diff_types(
                        ctx.file_idx,
                        state.sources,
                        map_span,
                        &global_key_type,
                        *key_span,
                        &key_t,
                    )
                }
                if val_t != global_val_type {
                    error_map_diff_types(
                        ctx.file_idx,
                        state.sources,
                        map_span,
                        &global_val_type,
                        *val_span,
                        &val_t,
                    )
                }
            }
            let output_len = output.len();
            let key_val_id = key.compile(ctx, state, output, None, false, true).unwrap_id();
            if !(key.is_constant_literal()
                || matches!(key, Expr::Array(_, _)) && output_len == output.len())
            {
                error_not_literal_map_key(*key_span, map_span, ctx.file_idx, state.sources);
            }
            let key_val = state.registers[key_val_id as usize];
            let val_id = val.compile(ctx, state, output, None, false, true).unwrap_id();
            if val.is_constant_literal() {
                state.pools.map_pool[map_id].insert(key_val, state.registers[val_id as usize]);
            } else {
                state.pools.map_pool[map_id].insert(key_val, NULL);
                dynamic.push((key_val, val_id));
            }
        }

        let template_reg = state.new_reg(Data::map(map_id as u32));

        let dest_reg = state.new_reg(Data::map(0));

        output.push(Instr::CloneMap(template_reg, dest_reg));
        for (key_val, val_id) in dynamic {
            let key_reg = state.new_const_reg(key_val);
            output.push(Instr::MapInsertReg(dest_reg, key_reg, val_id));
        }
        dest_reg
    }
}

#[must_use]
fn compile_struct_field_access<'arena>(
    struct_expr: &'arena Expr,
    field: &str,
    struct_span: Span,
    field_span: Span,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let t = struct_expr.infer_type(ctx, state);
    if let DataType::Struct(s_id) = t {
        let s = &state.structs[s_id as usize];
        let idx = s.fields.iter().position(|f| f.name == field).unwrap_or_else(|| {
            compiler_errors::error_struct_unknown_field(
                ctx.file_idx,
                field_span,
                field,
                s.name,
                &s.fields,
                state.sources,
            );
        });
        let id = struct_expr.compile(ctx, state, output, None, false, true).unwrap_id();
        let dest_reg_id = state.alloc_reg();
        output.push(Instr::GetFieldStruct(id, idx as u16, dest_reg_id));
        dest_reg_id
    } else {
        error_invalid_type(
            &DataType::Struct(0),
            &t,
            struct_span,
            None,
            None,
            ctx.file_idx,
            state.sources,
        );
    }
}

#[must_use]
fn compile_array_indexing<'arena>(
    array: &'arena Expr,
    index: &'arena Expr,
    span: Span,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let inferred = array.infer_type(ctx, state);
    if !inferred.is_indexable() {
        error_type_not_indexable(&inferred, span, false, ctx.file_idx, state.sources);
    }

    let id = array.compile(ctx, state, output, None, false, true).unwrap_id();

    let index_inferred = index.infer_type(ctx, state);
    if index_inferred != DataType::Int {
        error_invalid_index_type(&index_inferred, span, ctx.file_idx, state.sources);
    }
    let index_id = index.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(index_id);
    let dest_reg_id = state.alloc_reg();

    let to_push = if inferred == DataType::String {
        Instr::GetIndexString(id, index_id, dest_reg_id)
    } else {
        Instr::GetIndexArray(id, index_id, dest_reg_id)
    };
    output.push(to_push);
    state.add_to_src(ctx, output, span);
    dest_reg_id
}

#[must_use]
fn compile_array_slice<'arena>(
    array: &'arena Expr,
    idx_start: &'arena Expr,
    idx_end: &'arena Expr,
    span: Span,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let inferred = array.infer_type(ctx, state);
    if !inferred.is_indexable() {
        error_type_not_indexable(&inferred, span, false, ctx.file_idx, state.sources);
    }
    let id = array.compile(ctx, state, output, None, false, true).unwrap_id();
    let idx_start_inferred = idx_start.infer_type(ctx, state);
    if idx_start_inferred != DataType::Int {
        error_invalid_index_type(&idx_start_inferred, span, ctx.file_idx, state.sources);
    }
    let idx_start_id = idx_start.compile(ctx, state, output, None, false, true).unwrap_id();
    let idx_end_inferred = idx_end.infer_type(ctx, state);
    if idx_end_inferred != DataType::Int {
        error_invalid_index_type(&idx_end_inferred, span, ctx.file_idx, state.sources);
    }
    let idx_end_id = idx_end.compile(ctx, state, output, None, false, true).unwrap_id();
    output.push(Instr::StoreFuncArg(idx_end_id));
    state.add_arg_hint(1);
    state.free_reg(idx_start_id);
    state.free_reg(idx_end_id);
    let dest_reg_id = state.alloc_reg();
    let to_push = if inferred == DataType::String {
        Instr::GetSliceString(id, idx_start_id, dest_reg_id)
    } else {
        Instr::GetSliceArray(id, idx_start_id, dest_reg_id)
    };
    output.push(to_push);
    state.add_to_src(ctx, output, span);
    state.sub_arg_hint(1);
    dest_reg_id
}

#[must_use]
fn uniform_op<'arena>(
    instr: fn(u16, u16, u16) -> Instr,
    symbol: &'static str,
    l: &'arena Expr,
    r: &'arena Expr,
    span_l: Span,
    span_r: Span,
    t: &DataType,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let (t_l, t_r) = (l.infer_type(ctx, state), r.infer_type(ctx, state));
    if &t_l != t || &t_r != t {
        compiler_errors::error_op(&t_l, &t_r, symbol, span_l, span_r, ctx.file_idx, state.sources);
    }

    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    let id_r = r.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    state.free_reg(id_r);
    let id = state.alloc_reg_tgt(tgt_id);
    output.push(instr(id_l, id_r, id));
    id
}

#[inline]
#[must_use]
fn uniform_op2<'arena>(
    instr: fn(u16, u16, u16) -> Instr,
    t_1: &'static DataType,
    instr2: fn(u16, u16, u16) -> Instr,
    t_2: &'static DataType,
    symbol: &'static str,
    l: &'arena Expr,
    r: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let (t_l, t_r) = (l.infer_type(ctx, state), r.infer_type(ctx, state));
    if !((&t_l == t_1 && &t_r == t_1) || (&t_l == t_2 && &t_r == t_2)) {
        compiler_errors::error_op(&t_l, &t_r, symbol, span_l, span_r, ctx.file_idx, state.sources);
    }
    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    let id_r = r.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    state.free_reg(id_r);
    let id = state.alloc_reg_tgt(tgt_id);
    output.push(if &t_l == t_1 { instr(id_l, id_r, id) } else { instr2(id_l, id_r, id) });
    id
}

#[must_use]
fn compile_div_op<'arena>(
    l: &'arena Expr,
    r: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    if let Expr::Int(n) = r
        && *n == 0
    {
        error_division_by_zero(false, span_l.extend(span_r), ctx.file_idx, state.sources);
    }
    let id = uniform_op2(
        Instr::DivFloat,
        &DataType::Float,
        Instr::DivInt,
        &DataType::Int,
        "/",
        l,
        r,
        span_l,
        span_r,
        tgt_id,
        ctx,
        state,
        output,
    );
    if matches!(output.last(), Some(Instr::DivInt(..))) {
        state.add_to_src(ctx, output, span_l.extend(span_r));
    }
    id
}

#[must_use]
fn compile_add_op<'arena>(
    l: &'arena Expr,
    r: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let t_l = l.infer_type(ctx, state);
    let t_r = r.infer_type(ctx, state);
    if t_l != t_r
        || !matches!(t_l, DataType::String | DataType::Array(_) | DataType::Float | DataType::Int)
    {
        compiler_errors::error_op(&t_l, &t_r, "+", span_l, span_r, ctx.file_idx, state.sources);
    }
    // var+1 or 1+var use the dedicated IncInt/IncIntTo instructions
    if t_l == DataType::Int
        && let Some(Expr::Var(src_name, _)) = {
            if matches!(r, Expr::Int(1)) {
                Some(l)
            } else if matches!(l, Expr::Int(1)) {
                Some(r)
            } else {
                None
            }
        }
        && let Some(src_var) = state.find_var(src_name.get_name())
    {
        let src_id = src_var.register_id;
        let id = tgt_id.unwrap_or_else(|| state.alloc_reg());
        output.push(if src_id == id { Instr::IncInt(id) } else { Instr::IncIntTo(src_id, id) });
        return id;
    }
    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    let id_r = r.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    state.free_reg(id_r);
    let id = state.alloc_reg_tgt(tgt_id);
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

#[must_use]
fn compile_sub_op<'arena>(
    l: &'arena Expr,
    r: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let t_l = l.infer_type(ctx, state);
    let t_r = r.infer_type(ctx, state);
    if !((t_l == DataType::Float && t_r == DataType::Float)
        || (t_l == DataType::Int && t_r == DataType::Int))
    {
        compiler_errors::error_op(&t_l, &t_r, "-", span_l, span_r, ctx.file_idx, state.sources);
    }
    // var-1 uses the dedicated DecInt/DecIntTo instructions
    if t_l == DataType::Int
        && matches!(r, Expr::Int(1))
        && let Expr::Var(src_name, _) = l
        && let Some(src_var) = state.find_var(src_name.get_name())
    {
        let src_id = src_var.register_id;
        let id = tgt_id.unwrap_or_else(|| state.alloc_reg());
        output.push(if src_id == id { Instr::DecInt(id) } else { Instr::DecIntTo(src_id, id) });
        return id;
    }
    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    let id_r = r.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    state.free_reg(id_r);
    let id = state.alloc_reg_tgt(tgt_id);
    output.push(if t_l == DataType::Float {
        Instr::SubFloat(id_l, id_r, id)
    } else {
        Instr::SubInt(id_l, id_r, id)
    });
    id
}

#[must_use]
fn compile_mod_op<'arena>(
    l: &'arena Expr,
    r: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    if let Expr::Int(n) = r
        && *n == 0
    {
        error_division_by_zero(true, span_l.extend(span_r), ctx.file_idx, state.sources);
    }
    let id = uniform_op2(
        Instr::ModFloat,
        &DataType::Float,
        Instr::ModInt,
        &DataType::Int,
        "%",
        l,
        r,
        span_l,
        span_r,
        tgt_id,
        ctx,
        state,
        output,
    );
    if matches!(output.last(), Some(Instr::ModInt(..))) {
        state.add_to_src(ctx, output, span_l.extend(span_r));
    }
    id
}

#[must_use]
fn compile_eq_op<'arena>(
    l: &'arena Expr,
    r: &'arena Expr,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let l_type = l.infer_type(ctx, state);
    let r_type = r.infer_type(ctx, state);
    let is_array = matches!(l_type, DataType::Array(_) | DataType::Struct(_))
        && matches!(r_type, DataType::Array(_) | DataType::Struct(_));
    let is_string = l_type == DataType::String && r_type == DataType::String;
    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    let id_r = r.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    state.free_reg(id_r);
    let id = state.alloc_reg_tgt(tgt_id);
    output.push(if is_array {
        Instr::ObjEq(id_l, id_r, id)
    } else if is_string {
        Instr::StrEq(id_l, id_r, id)
    } else {
        Instr::Eq(id_l, id_r, id)
    });
    id
}

#[must_use]
fn compile_neq_op<'arena>(
    l: &'arena Expr,
    r: &'arena Expr,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let l_type = l.infer_type(ctx, state);
    let r_type = r.infer_type(ctx, state);
    let is_array = matches!(l_type, DataType::Array(_) | DataType::Struct(_))
        && matches!(r_type, DataType::Array(_) | DataType::Struct(_));
    let is_string = l_type == DataType::String && r_type == DataType::String;
    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    let id_r = r.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    state.free_reg(id_r);
    let id = state.alloc_reg_tgt(tgt_id);
    if is_array {
        output.push(Instr::ObjNotEq(id_l, id_r, id));
    } else if is_string {
        output.push(Instr::StrNotEq(id_l, id_r, id));
    } else {
        output.push(Instr::NotEq(id_l, id_r, id));
    }
    id
}

#[must_use]
fn compile_neg_op<'arena>(
    l: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let operand_type = l.infer_type(ctx, state);
    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    let id = state.alloc_reg_tgt(tgt_id);
    if operand_type == DataType::Float {
        output.push(Instr::NegFloat(id_l, id));
    } else if operand_type == DataType::Int {
        output.push(Instr::NegInt(id_l, id));
    } else {
        compiler_errors::error_op(
            &DataType::Null,
            &operand_type,
            "-",
            span_l,
            span_r,
            ctx.file_idx,
            state.sources,
        );
    }
    id
}

#[must_use]
fn compile_bool_neg_op<'arena>(
    l: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let operand_type = l.infer_type(ctx, state);
    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    let id = state.alloc_reg_tgt(tgt_id);
    if operand_type != DataType::Bool {
        compiler_errors::error_op(
            &DataType::Null,
            &operand_type,
            "-",
            span_l,
            span_r,
            ctx.file_idx,
            state.sources,
        );
    }
    output.push(Instr::NegBool(id_l, id));
    id
}

fn compile_type_eq_op<'arena>(
    value: &'arena Expr,
    type_candidate: &TypeExpr,
    span: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let type_candidate =
        type_candidate.to_datatype(ctx.file_idx, state.scope(ctx.file_idx), state.sources);

    let value_type = value.infer_type(ctx, state);

    if matches!(value_type, DataType::Union(_) | DataType::Unknown) {
        let val_reg_id = value.compile(ctx, state, output, None, false, true).unwrap_id();
        state.free_reg(val_reg_id);
        let type_idx = state.compile_type(type_candidate);
        let dest_reg_id = state.alloc_reg_tgt(tgt_id);
        output.push(Instr::IsType(val_reg_id, type_idx, dest_reg_id));
        state.add_to_src(ctx, output, span);
        dest_reg_id
    } else {
        state.new_const_reg(Data::bool(value_type == type_candidate))
    }
}

fn compile_array_index_assignment<'arena>(
    array: &'arena Expr,
    index: &'arena Expr,
    value: &'arena Expr,
    index_span: Span,
    elem_span: Span,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let array_type = array.infer_type(ctx, state);
    if !array_type.is_indexable() {
        error_type_not_indexable(&array_type, index_span, false, ctx.file_idx, state.sources);
    }
    // Get the id of the source array/string (may be a nested GetIndex)
    let id = array.compile(ctx, state, output, None, false, true).unwrap_id();

    let final_id = index.compile(ctx, state, output, None, false, true).unwrap_id();

    let elem_type = value.infer_type(ctx, state);
    let elem_id = value.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(elem_id);
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
        error_cannot_push_type_to_array(
            &array_type,
            &elem_type,
            index_span,
            elem_span,
            ctx.file_idx,
            state.sources,
        );
    }

    let to_push = if array_type == DataType::String {
        Instr::SetElementString(id, elem_id, final_id)
    } else {
        Instr::SetElementObj(id, elem_id, final_id)
    };
    output.push(to_push);
    state.add_to_src(ctx, output, index_span);
    state.free_reg(id);
}

fn compile_struct_field_assignment<'arena>(
    struct_field_assignment: &'arena StructFieldAssignmentExpr,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let struct_expr = &struct_field_assignment.struct_expr;
    let new_val = &struct_field_assignment.field_value;
    let field = &struct_field_assignment.field;
    let t = struct_expr.infer_type(ctx, state);
    let new_val_type = new_val.infer_type(ctx, state);
    let DataType::Struct(struct_id) = t else {
        let struct_span = unsafe { *struct_field_assignment.spans.get_unchecked(0) };
        error_invalid_type(
            &DataType::Struct(0),
            &t,
            struct_span,
            None,
            None,
            ctx.file_idx,
            state.sources,
        );
    };
    let mut field_index: Option<u16> = None;
    let field_struct = &state.structs[struct_id as usize];
    let struct_name = &field_struct.name;
    for (
        i,
        StructField {
            name: expected_field_name,
            field_type: expected_field_type,
            span: expected_field_span,
        },
    ) in field_struct.fields.iter().enumerate()
    {
        if expected_field_name == field {
            if !struct_field_type_matches(expected_field_type, &new_val_type) {
                let value_span = unsafe { *struct_field_assignment.spans.get_unchecked(2) };
                compiler_errors::error_struct_field_invalid_type(
                    ctx.file_idx,
                    struct_name,
                    *expected_field_span,
                    expected_field_name,
                    expected_field_type,
                    value_span,
                    &new_val_type,
                    state.sources,
                );
            }
            field_index = Some(i as u16);
            break;
        }
    }
    let Some(field_index) = field_index else {
        let field_span = unsafe { *struct_field_assignment.spans.get_unchecked(1) };
        compiler_errors::error_struct_unknown_field(
            ctx.file_idx,
            field_span,
            field,
            struct_name,
            &field_struct.fields,
            state.sources,
        );
    };
    let id = struct_expr.compile(ctx, state, output, None, false, true).unwrap_id();
    let new_elem_reg_id = new_val.compile(ctx, state, output, None, false, true).unwrap_id();
    output.push(Instr::SetFieldStruct(id, new_elem_reg_id, field_index));
}

fn compile_if_block_branch<'arena>(
    branch: &'arena [Expr],
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    if let Some(tgt_id) = tgt_id {
        let regs_len = state.registers.len() as u16;
        let output_len = output.len();
        output.extend(compile_expr(
            &branch[..branch.len() - 1],
            ctx.advance_offset(output.len() as u16),
            state,
        ));
        let val_id = branch[branch.len() - 1]
            .compile(
                ctx.advance_offset(output.len() as u16),
                state,
                output,
                Some(tgt_id),
                false,
                true,
            )
            .unwrap_id();
        state.free_scope_registers(regs_len, &output[output_len..]);
        if val_id != tgt_id {
            output.push(Instr::Mov(val_id, tgt_id));
        }
    } else {
        output.extend(compile_expr(branch, ctx.advance_offset(output.len() as u16), state));
    }
}

fn compile_if_block<'arena>(
    IfBlockExpr { condition, then, otherwise, condition_span, span: _ }: &'arena IfBlockExpr,
    previous_jumps: Vec<usize>,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let condition_start = output.len();
    for j in previous_jumps {
        set_jmp_size(&mut output[j], (condition_start - j) as u16);
    }

    let condition_type = condition.infer_type(ctx, state);
    if condition_type != DataType::Bool {
        error_invalid_type(
            &DataType::Bool,
            &condition_type,
            *condition_span,
            None,
            None,
            ctx.file_idx,
            state.sources,
        )
    }
    let (b, true_jump_idxs, false_jump_idxs) = compile_condition(condition, ctx, state, output);
    if let Some(const_bool) = b {
        if const_bool {
            compile_if_block_branch(then, tgt_id, ctx, state, output);
            // This is just to make it so that the branch that's thrown away is still type-checked
            compile_if_block_branch(otherwise, tgt_id, ctx, state, &mut Vec::new());
        } else if let [Expr::IfBlock(if_block)] = otherwise {
            compile_if_block(if_block, Vec::new(), tgt_id, ctx, state, output);
        } else if !otherwise.is_empty() {
            compile_if_block_branch(otherwise, tgt_id, ctx, state, output);
        }
        return;
    }

    // Modify true jump instructions to point to body_start
    let body_start = output.len();
    for j in true_jump_idxs {
        set_jmp_size(&mut output[j], (body_start - j) as u16);
    }

    compile_if_block_branch(then, tgt_id, ctx, state, output);

    let jump_over_instr_idx = if otherwise.is_empty() {
        0
    } else {
        output.push(Instr::Jmp(0));
        output.len() - 1
    };

    let branch_start = output.len();
    if otherwise.is_empty() {
        // single if block
        for j in false_jump_idxs {
            set_jmp_size(&mut output[j], (branch_start - j) as u16);
        }
    } else if let [Expr::IfBlock(if_block)] = otherwise {
        compile_if_block(if_block, false_jump_idxs, tgt_id, ctx, state, output);
    } else {
        for j in false_jump_idxs {
            set_jmp_size(&mut output[j], (branch_start - j) as u16);
        }
        compile_if_block_branch(otherwise, tgt_id, ctx, state, output);
    }

    if !otherwise.is_empty() {
        let output_len = output.len();
        set_jmp_size(&mut output[jump_over_instr_idx], (output_len - jump_over_instr_idx) as u16);
    }
}

fn compile_while_loop<'arena>(
    condition: &'arena Expr,
    code: &'arena [Expr],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let output_len_before = output.len();

    let (true_jump_idxs, false_jump_idxs) =
        compile_short_circuit_condition(condition, ctx, state, output, false);

    let body_start = output.len();
    for j in true_jump_idxs {
        set_jmp_size(&mut output[j], (body_start - j) as u16);
    }

    // parse the code block, clone the vars to avoid overriding anything
    let loop_id = ctx.block_id + 1;

    let mut cond_code =
        compile_expr(code, ctx.no_single_run().advance_offset(output.len() as u16), state);

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

fn compile_for_loop<'arena>(
    var_name: &'arena str,
    array: &'arena Expr,
    code: &'arena [Expr],
    span: Span,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let real_var = var_name != "_";

    // parse the array, get its id (the target array is the first Expr in array_code)
    let array_type = array.infer_type(ctx, state);
    let array = array.compile(ctx, state, output, None, false, true).unwrap_id();

    let array_len_id = state.alloc_reg();

    output.push(Instr::CallLibFunc(LibFunc::Len, array, array_len_id));

    // set up the id of the index variable (0..len)
    let index_id = if ctx.single_run {
        state.new_reg(Data::int(0))
    } else {
        let id = state.alloc_reg();
        output.push(Instr::SetInt(id, 0));
        id
    };

    // do the 'i < len' condition, set up the condition's id (true/false)
    let condition_id = state.alloc_reg();

    output.push(Instr::InfInt(index_id, array_len_id, condition_id));

    // set up the variable for the current element (for current_element_id in ... {}) => current_element_id = array[index]
    let current_element_id = if real_var { state.alloc_reg() } else { 0 };

    let v_len = state.v.len();

    let is_str = array_type == DataType::String;

    if real_var {
        state.new_var(
            var_name,
            current_element_id,
            match array_type {
                DataType::String => DataType::String,
                DataType::Array(a_type) => a_type.map_or(DataType::Null, |t| *t),
                t => {
                    error_type_not_indexable(&t, span, true, ctx.file_idx, state.sources);
                }
            },
        );
    }
    let loop_id = ctx.block_id + 1;

    // accounts for the GetIndexArray/GetIndexString instruction
    let pending = real_var as u16;

    let regs_before = state.registers.len() as u16;
    let mut cond_code = compile_expr(
        code,
        ctx.no_single_run().advance_offset(output.len() as u16 + pending),
        state,
    );
    // Clean up variables
    state.v.truncate(v_len);
    state.free_loop_scope_registers(regs_before, &cond_code);

    // add the condition ('i < len') jumping logic
    let mut len = (cond_code.len() + 3) as u16 + pending;
    add_cmp_false(condition_id, &mut len, output, true);

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

    if ctx.single_run {
        state.free_reg(array_len_id);
        state.free_reg(index_id);
        state.free_reg(condition_id);
        if real_var {
            state.free_reg(current_element_id);
        }
    }
}

fn compile_int_for_loop<'arena>(
    int_for_loop: &'arena IntForLoopExpr,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let lower_bound = int_for_loop.get_lower_bound();
    let upper_bound = int_for_loop.get_upper_bound();
    let code = int_for_loop.get_loop_code();

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
    let t1 = lower_bound.infer_type(ctx, state);
    let t2 = upper_bound.infer_type(ctx, state);
    if t1 != DataType::Int {
        error_range_invalid_type(int_for_loop.lower_bound_span, &t1, ctx.file_idx, state.sources);
    }
    if t2 != DataType::Int {
        error_range_invalid_type(int_for_loop.upper_bound_span, &t2, ctx.file_idx, state.sources);
    }
    let elem_id = if ctx.single_run {
        lower_bound.compile(ctx, state, output, None, false, true).unwrap_id()
    } else {
        let start_elem_id = lower_bound.compile(ctx, state, output, None, false, true).unwrap_id();
        let start_val = state.registers[start_elem_id as usize];
        let elem_id = state.alloc_reg();
        if state.is_register_const(start_elem_id) && start_val.is_int() {
            output.push(Instr::SetInt(elem_id, start_val.as_int()));
        } else if start_elem_id != elem_id {
            output.push(Instr::Mov(start_elem_id, elem_id));
        }
        elem_id
    };
    let end_elem_id = upper_bound.compile(ctx, state, output, None, false, true).unwrap_id();

    // elem_id is a fresh mutable register -> remove from const_registers just in case
    state.remove_reg_from_consts(elem_id);

    let v_len = state.v.len();
    state.new_var(int_for_loop.var_name, elem_id, DataType::Int);
    let loop_id = ctx.block_id + 1;

    // (1) if i >= end_elem jump out -> push placeholder first so that compile_expr sees the correct offset
    let jmp_idx = output.len();
    output.push(Instr::SupEqIntJmp(elem_id, end_elem_id, 0));

    let regs_before = state.registers.len() as u16;
    let compiled_loop_code =
        compile_expr(code, ctx.no_single_run().advance_offset(output.len() as u16), state);
    state.free_loop_scope_registers(regs_before, &compiled_loop_code);
    let compiled_loop_code_len = compiled_loop_code.len() as u16;

    // (2) loop_body
    output.extend(compiled_loop_code);

    // (3) i+= 1
    output.push(Instr::IncInt(elem_id));

    // (4) if i < end_elem jump back to body
    output.push(Instr::InfIntJmpBack(elem_id, end_elem_id, compiled_loop_code_len + 1));

    let exit_size = (output.len() - jmp_idx) as u16;
    output[jmp_idx] = Instr::SupEqIntJmp(elem_id, end_elem_id, exit_size);

    parse_loop_flow_control(&mut output[jmp_idx + 1..], loop_id, exit_size, true, false);
    state.v.truncate(v_len);

    if ctx.single_run {
        state.free_reg(end_elem_id);
        state.free_reg(elem_id);
    }
}

fn compile_loop_block<'arena>(
    code: &'arena [Expr],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let loop_id = ctx.block_id + 1;
    let regs_before = state.registers.len() as u16;
    let mut compiled =
        compile_expr(code, ctx.no_single_run().advance_offset(output.len() as u16), state);
    state.free_loop_scope_registers(regs_before, &compiled);
    let code_length = compiled.len() as u16;
    parse_loop_flow_control(&mut compiled, loop_id, code_length + 1, false, true);
    output.extend(compiled);
    output.push(Instr::JmpBack(code_length));
}

fn compile_try_catch_block<'arena>(
    e: &'arena [Expr],
    err_var: &'arena str,
    catch_code: &'arena [Expr],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    output.push(Instr::StartErrorCatch(0, 0)); // patched later on
    let err_catch_instr = output.len() - 1;
    let main_code = compile_expr(e, ctx, state);
    output.extend(main_code);
    output.push(Instr::StopErrorCatch);
    output.push(Instr::Jmp(0)); // jumps over the catch handler if no error arises
    let jmp_catch_instr = output.len() - 1;

    let v_len = state.v.len();
    let err_reg_id = state.alloc_reg();
    state.new_var(err_var, err_reg_id, DataType::String);
    output[err_catch_instr] =
        Instr::StartErrorCatch((output.len() - err_catch_instr) as u16, err_reg_id);
    let catch_code = compile_expr(catch_code, ctx, state);
    state.v.truncate(v_len);
    output.extend(catch_code);
    output[jmp_catch_instr] = Instr::Jmp((output.len() - jmp_catch_instr) as u16);
    state.free_reg(err_reg_id);
}

fn compile_var_declaration<'arena>(
    var_declaration: &'arena VariableDeclarationExpr,
    remaining_code: &[Expr],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let name = &var_declaration.name;
    let value = &var_declaration.value;
    let value_type = value.infer_type(ctx, state);

    let declared_type = if let Some(v_t) = &var_declaration.var_type {
        let declared_var_type =
            v_t.0.to_datatype(ctx.file_idx, state.scope(ctx.file_idx), state.sources);
        if !var_type_is_compatible(&declared_var_type, &value_type) {
            error_invalid_type(
                &declared_var_type,
                &value_type,
                v_t.1,
                None,
                None,
                ctx.file_idx,
                state.sources,
            );
        }
        declared_var_type
    } else {
        value_type.clone()
    };

    let is_var_read = matches!(value, Expr::Var(..)) && !matches!(value_type, DataType::Fn(_));

    let src_id = value.compile(ctx, state, output, None, ctx.single_run, true).unwrap_id();
    let var_id = if is_var_read
        && match value {
            Expr::Var(v, _) if v.is_namespace_empty() && state.find_var(v.get_name()).is_some() => {
                code_modifies_variable(v.get_name(), remaining_code)
                    || code_modifies_variable(name, remaining_code)
            }
            _ => true,
        } {
        let reg_id = state.alloc_reg();
        output.push(Instr::Mov(src_id, reg_id));
        reg_id
    } else if !ctx.single_run && code_modifies_variable(name, remaining_code) {
        let var_id = state.alloc_reg();
        move_reg_to_reg(output, src_id, var_id, state.registers[src_id as usize]);
        var_id
    } else {
        src_id
    };

    if let DataType::Fn(fn_id) = value_type
        && matches!(value, Expr::AnonymousFunction(..))
    {
        state.functions[fn_id as usize].name = var_declaration.name;
    }
    state.unfree_register(var_id);
    state.new_var_with_type(name, var_id, value_type, declared_type);
}

fn compile_var_assignment<'arena>(
    path: &[&str],
    name: &str,
    value: &'arena Expr,
    span: Span,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let var_type = value.infer_type(ctx, state);

    let local_var_idx = if path.is_empty() { state.find_var_idx(name) } else { None };
    let global_var_idx = if local_var_idx.is_some() {
        None
    } else if let Some(global_id) =
        state.scope(ctx.file_idx).find_global(path, name, span, ctx.file_idx, state.sources)
    {
        Some(global_id)
    } else {
        compiler_errors::error_unknown_variable(name, span, state.v, ctx.file_idx, state.sources);
    };

    let declared_var_type = if let Some(idx) = local_var_idx {
        &state.v[idx].declared_type
    } else {
        &state.globals[unsafe { global_var_idx.unwrap_unchecked() }].declared_type
    };

    if !var_type_is_compatible(declared_var_type, &var_type) {
        error_invalid_type(
            declared_var_type,
            &var_type,
            span,
            None,
            None,
            ctx.file_idx,
            state.sources,
        )
    }

    let reg_id = if let Some(pos) = local_var_idx {
        state.v[pos].register_id
    } else {
        state.globals[unsafe { global_var_idx.unwrap_unchecked() }].register_id
    };

    if var_type == DataType::Int {
        // (is_inc, src_register_id)
        let inc_dec: Option<(bool, u16)> = match value {
            // var+1/1+var use the dedicated IncInt/IncIntTo instructions
            Expr::Add(l, r, _, _) => {
                let src = if matches!(r, Expr::Int(1)) {
                    Some(l)
                } else if matches!(l, Expr::Int(1)) {
                    Some(r)
                } else {
                    None
                };
                src.and_then(|e| int_var_register(e, ctx, state)).map(|src_id| (true, src_id))
            }
            // var-1 uses the dedicated DecInt/DecIntTo instructions
            Expr::Sub(l, r, _, _) => {
                if matches!(r, Expr::Int(1)) {
                    int_var_register(l, ctx, state).map(|src_id| (false, src_id))
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some((is_inc, src_id)) = inc_dec {
            output.push(if src_id == reg_id {
                if is_inc { Instr::IncInt(reg_id) } else { Instr::DecInt(reg_id) }
            } else {
                if is_inc {
                    Instr::IncIntTo(src_id, reg_id)
                } else {
                    Instr::DecIntTo(src_id, reg_id)
                }
            });
            return;
        }
    }

    let output_len = output.len();
    let obj_id = value.compile(ctx, state, output, Some(reg_id), false, true).unwrap_id();
    if state.is_register_const(obj_id) {
        move_reg_to_reg(output, obj_id, reg_id, state.registers[obj_id as usize]);
    } else {
        move_value_to(output, output_len, obj_id, reg_id);
    }
    if !state.v.iter().any(|var| var.name != name && var.register_id == obj_id) {
        state.free_reg(obj_id);
    }
    if let Some(pos) = local_var_idx {
        state.v[pos].var_type = var_type;
    } else if let Some(pos) = global_var_idx {
        state.globals[pos].var_type = var_type;
    }
}

#[must_use]
fn int_var_register(e: &Expr, ctx: Ctx, state: &State<'_, '_>) -> Option<u16> {
    let (namespace, name, span): (&[&str], &str, Span) = match e {
        Expr::Var(n, s) => (n.get_namespace(), n.get_name(), *s),
        _ => return None,
    };
    if namespace.is_empty()
        && let Some(var) = state.find_var(name)
    {
        return (var.var_type == DataType::Int).then_some(var.register_id);
    }
    let global_var = &state.globals[state.scope(ctx.file_idx).find_global(
        namespace,
        name,
        span,
        ctx.file_idx,
        state.sources,
    )?];
    (global_var.var_type == DataType::Int).then_some(global_var.register_id)
}

#[must_use]
fn compile_var_access(
    path: &[&str],
    name: &str,
    span: Span,
    ctx: Ctx,
    state: &mut State<'_, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    // Local variable
    if path.is_empty()
        && let Some(Variable { register_id, .. }) = state.find_var(name)
    {
        return *register_id;
    }
    // Global variable
    if let Some(idx) =
        state.scope(ctx.file_idx).find_global(path, name, span, ctx.file_idx, state.sources)
    {
        return state.globals[idx].register_id;
    }

    // Function referenced by name
    let Some(fn_id) =
        state.scope(ctx.file_idx).find_function(path, name, span, ctx.file_idx, state.sources)
    else {
        compiler_errors::error_unknown_variable(name, span, state.v, ctx.file_idx, state.sources);
    };

    let arg_types: Vec<DataType> =
        state.functions[fn_id].args.iter().map(|(_, t)| t.clone().unwrap()).collect();

    let fn_impl_idx = compile_function_impl(output, ctx, state, fn_id, &arg_types);
    let loc = state.functions[fn_id].impls[fn_impl_idx].loc;
    state.new_reg(Data::function(loc))
}

fn compile_struct_definition<'arena>(
    name: &'arena str,
    fields: &[(&'arena str, TypeExpr, Span)],
    span: Span,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
) {
    let struct_id = state.structs.len() as u16;
    state.structs.push(Struct {
        // pushing it first allows structs to be recursive
        name,
        fields: Box::from([]),
        id: struct_id,
        name_span: span,
        src_file_idx: ctx.file_idx,
    });
    let struct_id = (state.structs.len() - 1) as u16;
    if let Some(struct_id) =
        state.scope_mut(ctx.file_idx).symbols.insert((name, Symbol::Struct), struct_id)
    {
        error_struct_already_defined(
            &state.structs[struct_id as usize],
            span,
            ctx.file_idx,
            state.sources,
        );
    }
    let parsed_fields = fields
        .iter()
        .map(|(field_name, field_type, field_span)| StructField {
            name: field_name,
            field_type: field_type.to_datatype(
                ctx.file_idx,
                state.scope(ctx.file_idx),
                state.sources,
            ),
            span: *field_span,
        })
        .collect();
    state.structs[struct_id as usize].fields = parsed_fields;
}

fn compile_function_definition<'arena>(
    function_declaration: &'arena FunctionDeclarationExpr,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
) {
    let fn_name = &function_declaration.name;
    let span = function_declaration.span;
    let fn_code = function_declaration.code;
    let fn_args = &function_declaration.args;
    let mut callees = Vec::new();
    collect_direct_fn_calls(fn_code, &mut callees);
    let fn_id = state.functions.len() as u16;
    if let Some(func) = state.scope_mut(ctx.file_idx).symbols.insert((fn_name, Symbol::Fn), fn_id) {
        compiler_errors::error_function_already_defined(
            &state.functions[func as usize],
            span,
            ctx.file_idx,
            state.sources,
        );
    }
    state.functions.push(Function {
        name: fn_name,
        args: fn_args
            .iter()
            .map(|arg| {
                (
                    arg.name,
                    arg.enforced_type.as_ref().map(|t_e| {
                        t_e.to_datatype(ctx.file_idx, state.scope(ctx.file_idx), state.sources)
                    }),
                )
            })
            .collect::<Vec<(&str, Option<DataType>)>>()
            .into_boxed_slice(),
        code: fn_code,
        impls: Vec::new(),
        is_recursive: None,
        returns_null: check_if_returns_void(fn_code),
        src_file_idx: ctx.file_idx,
        return_type_cache: Vec::new(),
        direct_calls: state.bump.alloc_slice_copy(&callees),
        name_span: span,
    });
    state.fn_registers.push(Vec::new());
}

fn compile_return<'arena>(
    return_value: Option<&'arena Expr<'arena>>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    if let Some(x) = return_value {
        let id = x.compile(ctx, state, output, None, false, true).unwrap_id();
        if ctx.is_compiling_recursive {
            output.push(Instr::RecursiveReturn(id));
        } else {
            output.push(Instr::Return(id));
        }
    }
}

#[inline]
fn compile_loop_break(ctx: Ctx, output: &mut Vec<Instr>) {
    output.push(Instr::NotEqJmp(ctx.block_id + 1, 0, 0));
}

#[inline]
fn compile_loop_continue(ctx: Ctx, output: &mut Vec<Instr>) {
    output.push(Instr::EqJmp(ctx.block_id + 1, 0, 0));
}

#[inline]
fn compile_eval_block<'arena>(
    code: &'arena [Expr<'arena>],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    output.extend(compile_expr(code, ctx.with_offset(output.len() as u16), state));
}

fn compile_match_block<'arena>(
    MatchExpr { obj: match_obj, arms, span }: &'arena MatchExpr,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    const MATCH_OBJ_VAR: &str = "{";
    let (mut if_code, if_arms): (&'arena [Expr<'arena>], &[MatchArm<'arena>]) =
        match arms.split_last() {
            Some((last, rest)) if matches!(last.pattern, Pattern::Wildcard(_)) => (last.code, rest),
            _ => (&[], *arms),
        };
    for arm in if_arms.iter().rev() {
        match arm.pattern {
            Pattern::Constant(cst, span) => {
                if_code = state.bump.alloc_slice_copy(&[Expr::IfBlock(IfBlockExpr {
                    condition: state.bump.alloc(Expr::Eq(
                        state.bump.alloc(Expr::Var(
                            QualifiedName::new(&[MATCH_OBJ_VAR], state.bump),
                            span,
                        )),
                        cst,
                    )),
                    then: arm.code,
                    otherwise: if_code,
                    condition_span: span,
                    span,
                })]);
            }
            Pattern::Identifier(id, id_span) => todo!(),
            Pattern::Constructor(PatternConstructor {
                qualified_name,
                fields,
                fill_the_rest,
                span,
            }) => todo!(),
            Pattern::Wildcard(_) => unsafe { unreachable_unchecked() },
        }
    }

    let interm_code = state.bump.alloc_slice_copy(&[
        Expr::VarDeclare(VariableDeclarationExpr {
            name: MATCH_OBJ_VAR,
            value: match_obj,
            var_type: None,
            span: Span::empty(),
        }),
        if_code[0],
    ]);
    compile_eval_block(interm_code, ctx, state, output);
}

pub fn compile_expr<'arena>(
    input: &'arena [Expr<'arena>],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
) -> Vec<Instr> {
    let v_len = state.v.len();
    let fn_len = state.functions.len();
    let symbols_len = state.scope(ctx.file_idx).symbols.len();
    let mut output: Vec<Instr> = Vec::with_capacity(input.len());
    for (idx, x) in input.iter().enumerate() {
        if let Some(id) = x.compile_with_code_context(
            ctx,
            state,
            &mut output,
            None,
            false,
            &input[idx + 1..],
            false,
        ) {
            state.free_reg(id);
        }
    }
    for var_idx in v_len..state.v.len() {
        state.free_reg(state.v[var_idx].register_id);
    }
    state.v.truncate(v_len);
    state.functions.truncate(fn_len);
    state.scope_mut(ctx.file_idx).symbols.truncate(symbols_len);
    output
}

impl<'arena> Expr<'arena> {
    pub const fn is_constant_literal(&self) -> bool {
        matches!(self, Self::Int(_) | Self::Float(_) | Self::String(_) | Self::Bool(_) | Self::Null)
    }
    #[inline(always)]
    pub fn compile(
        &'arena self,
        ctx: Ctx,
        state: &mut State<'arena, '_>,
        output: &mut Vec<Instr>,
        tgt_id: Option<u16>,
        var_assignment: bool,
        uses_id: bool,
    ) -> Option<u16> {
        self.compile_with_code_context(ctx, state, output, tgt_id, var_assignment, &[], uses_id)
    }
    pub fn compile_with_code_context(
        &'arena self,
        ctx: Ctx,
        state: &mut State<'arena, '_>,
        output: &mut Vec<Instr>,
        tgt_id: Option<u16>,
        var_assignment: bool,
        remaining_code: &[Self],
        uses_id: bool,
    ) -> Option<u16> {
        match self {
            Self::Int(num) => {
                debug_assert!(uses_id);
                let int = Data::int(*num);
                Some(if var_assignment { state.new_reg(int) } else { state.new_const_reg(int) })
            }
            Self::Float(num) => {
                debug_assert!(uses_id);
                let float = Data::float(*num);
                Some(if var_assignment { state.new_reg(float) } else { state.new_const_reg(float) })
            }
            Self::String(str) => {
                debug_assert!(uses_id);
                let s = Data::comp_str(str, &mut state.pools.str_pool);
                Some(if var_assignment { state.new_reg(s) } else { state.new_const_reg(s) })
            }
            Self::Null => {
                debug_assert!(uses_id);
                Some(if var_assignment { state.new_reg(NULL) } else { state.new_const_reg(NULL) })
            }
            Self::Bool(bool) => {
                debug_assert!(uses_id);
                let b = Data::bool(*bool);
                Some(if var_assignment { state.new_reg(b) } else { state.new_const_reg(b) })
            }
            Self::Var(name, span) => {
                debug_assert!(uses_id);
                Some(compile_var_access(
                    name.get_namespace(),
                    name.get_name(),
                    *span,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::TypeEq(value, type_candidate, span) => {
                debug_assert!(uses_id);
                Some(compile_type_eq_op(value, type_candidate, *span, tgt_id, ctx, state, output))
            }
            Self::Array(array_items, spans) => {
                debug_assert!(uses_id);
                Some(compile_array_literal(array_items, spans, ctx, state, output))
            }
            Self::Struct(name, fields, span) => {
                debug_assert!(uses_id);
                Some(compile_struct_literal(name, fields, *span, ctx, state, output))
            }
            Self::Map(kv_pairs, span) => {
                debug_assert!(uses_id);
                Some(compile_map_literal(kv_pairs, *span, ctx, state, output))
            }
            Self::GetStructField(struct_expr, field, struct_span, field_span) => {
                debug_assert!(uses_id);
                Some(compile_struct_field_access(
                    struct_expr,
                    field,
                    *struct_span,
                    *field_span,
                    ctx,
                    state,
                    output,
                ))
            }
            // array[index]
            Self::ArrayGetIndex(array, index, span) => {
                debug_assert!(uses_id);
                Some(compile_array_indexing(array, index, *span, ctx, state, output))
            }
            // array[start..end]
            Self::ArrayGetSlice(array, idx_start, idx_end, span) => {
                debug_assert!(uses_id);
                Some(compile_array_slice(array, idx_start, idx_end, *span, ctx, state, output))
            }
            Self::Mul(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(uniform_op2(
                    Instr::MulFloat,
                    &DataType::Float,
                    Instr::MulInt,
                    &DataType::Int,
                    "*",
                    l,
                    r,
                    *span1,
                    *span2,
                    tgt_id,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::Div(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(compile_div_op(l, r, *span1, *span2, tgt_id, ctx, state, output))
            }
            Self::Add(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(compile_add_op(l, r, *span1, *span2, tgt_id, ctx, state, output))
            }
            Self::Sub(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(compile_sub_op(l, r, *span1, *span2, tgt_id, ctx, state, output))
            }
            Self::Mod(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(compile_mod_op(l, r, *span1, *span2, tgt_id, ctx, state, output))
            }
            Self::Pow(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(uniform_op2(
                    Instr::PowFloat,
                    &DataType::Float,
                    Instr::PowInt,
                    &DataType::Int,
                    "^",
                    l,
                    r,
                    *span1,
                    *span2,
                    tgt_id,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::Eq(l, r) => {
                debug_assert!(uses_id);
                Some(compile_eq_op(l, r, tgt_id, ctx, state, output))
            }
            Self::NotEq(l, r) => {
                debug_assert!(uses_id);
                Some(compile_neq_op(l, r, tgt_id, ctx, state, output))
            }
            Self::Sup(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(uniform_op2(
                    Instr::SupFloat,
                    &DataType::Float,
                    Instr::SupInt,
                    &DataType::Int,
                    ">",
                    l,
                    r,
                    *span1,
                    *span2,
                    tgt_id,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::SupEq(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(uniform_op2(
                    Instr::SupEqFloat,
                    &DataType::Float,
                    Instr::SupEqInt,
                    &DataType::Int,
                    ">=",
                    l,
                    r,
                    *span1,
                    *span2,
                    tgt_id,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::Inf(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(uniform_op2(
                    Instr::InfFloat,
                    &DataType::Float,
                    Instr::InfInt,
                    &DataType::Int,
                    "<",
                    l,
                    r,
                    *span1,
                    *span2,
                    tgt_id,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::InfEq(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(uniform_op2(
                    Instr::InfEqFloat,
                    &DataType::Float,
                    Instr::InfEqInt,
                    &DataType::Int,
                    "<=",
                    l,
                    r,
                    *span1,
                    *span2,
                    tgt_id,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::BoolAnd(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(uniform_op(
                    Instr::BoolAnd,
                    "&&",
                    l,
                    r,
                    *span1,
                    *span2,
                    &DataType::Bool,
                    tgt_id,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::BoolOr(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(uniform_op(
                    Instr::BoolOr,
                    "||",
                    l,
                    r,
                    *span1,
                    *span2,
                    &DataType::Bool,
                    tgt_id,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::Neg(l, span1, span2) => {
                debug_assert!(uses_id);
                Some(compile_neg_op(l, *span1, *span2, tgt_id, ctx, state, output))
            }
            Self::BoolNeg(l, span1, span2) => {
                debug_assert!(uses_id);
                Some(compile_bool_neg_op(l, *span1, *span2, tgt_id, ctx, state, output))
            }
            Self::FunctionCall(function_call) => {
                let output_id = compile_function_call(function_call, output, ctx, state, tgt_id);
                if uses_id {
                    Some(output_id.unwrap_or_else(|| state.new_const_reg(NULL)))
                } else {
                    if let Some(id) = output_id {
                        state.free_reg(id);
                    }
                    None
                }
            }
            Self::ObjFunctionCall(function_call) => {
                let output_id = compile_method_call(output, ctx, state, tgt_id, function_call);
                if uses_id {
                    Some(output_id.unwrap_or_else(|| state.new_const_reg(NULL)))
                } else {
                    if let Some(id) = output_id {
                        state.free_reg(id);
                    }
                    None
                }
            }
            Self::AnonymousFunction(_, _, _) => {
                debug_assert!(uses_id);
                // This is replaced later on by `builtin_functions` when it's called
                Some(state.new_reg(Data::function(0)))
            }

            // ------------------
            // --- STATEMENTS ---
            // ------------------

            // x[y] = z;
            Self::ArrayModify(array, index, value, index_markers, elem_markers) => {
                debug_assert!(!uses_id);
                compile_array_index_assignment(
                    array,
                    index,
                    value,
                    *index_markers,
                    *elem_markers,
                    ctx,
                    state,
                    output,
                );
                None
            }
            Self::SetStructField(struct_field_assignment) => {
                debug_assert!(!uses_id);
                compile_struct_field_assignment(struct_field_assignment, ctx, state, output);
                None
            }
            Self::IfBlock(if_block) => {
                let condition_return_id =
                    if uses_id { Some(state.alloc_reg_tgt(tgt_id)) } else { None };
                compile_if_block(if_block, Vec::new(), condition_return_id, ctx, state, output);
                condition_return_id
            }
            Self::WhileBlock(condition, code) => {
                debug_assert!(!uses_id);
                compile_while_loop(condition, code, ctx, state, output);
                None
            }
            Self::ForLoop(var_name, array, code, span) => {
                debug_assert!(!uses_id);
                compile_for_loop(var_name, array, code, *span, ctx, state, output);
                None
            }
            Self::IntForLoop(int_for_loop) => {
                debug_assert!(!uses_id);
                compile_int_for_loop(int_for_loop, ctx, state, output);
                None
            }
            Self::LoopBlock(code) => {
                debug_assert!(!uses_id);
                compile_loop_block(code, ctx, state, output);
                None
            }
            Self::TryCatchBlock(e, err_var, catch_code) => {
                debug_assert!(!uses_id);
                compile_try_catch_block(e, err_var, catch_code, ctx, state, output);
                None
            }
            Self::VarDeclare(var_declaration) => {
                debug_assert!(!uses_id);
                compile_var_declaration(var_declaration, remaining_code, ctx, state, output);
                None
            }
            Self::VarAssign(name, value, span) => {
                debug_assert!(!uses_id);
                compile_var_assignment(
                    name.get_namespace(),
                    name.get_name(),
                    value,
                    *span,
                    ctx,
                    state,
                    output,
                );
                None
            }
            Self::StructDeclare(name, fields, span) => {
                debug_assert!(!uses_id);
                compile_struct_definition(name, fields, *span, ctx, state);
                None
            }
            Self::FunctionDecl(function_declaration) => {
                debug_assert!(!uses_id);
                compile_function_definition(function_declaration, ctx, state);
                None
            }
            Self::ReturnVal(return_value) => {
                debug_assert!(!uses_id);
                compile_return(*return_value, ctx, state, output);
                None
            }
            Self::Break => {
                debug_assert!(!uses_id);
                compile_loop_break(ctx, output);
                None
            }
            Self::Continue => {
                debug_assert!(!uses_id);
                compile_loop_continue(ctx, output);
                None
            }
            Self::EvalBlock(code) => {
                debug_assert!(!uses_id);
                compile_eval_block(code, ctx, state, output);
                None
            }
            Self::Match(match_expr) => {
                debug_assert!(!uses_id);
                compile_match_block(match_expr, ctx, state, output);
                None
            }

            Self::ImportDylib(..) | Self::ImportFile(..) => unsafe { unreachable_unchecked() },
        }
    }
}

#[cfg(target_os = "macos")]
const DYLIB_EXT: &str = "dylib";
#[cfg(target_os = "linux")]
const DYLIB_EXT: &str = "so";
#[cfg(target_os = "windows")]
const DYLIB_EXT: &str = "dll";

#[cfg(target_arch = "aarch64")]
const ARCH_SUFFIX: &str = "-aarch64";
#[cfg(target_arch = "x86_64")]
const ARCH_SUFFIX: &str = "-x86_64";
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
const ARCH_SUFFIX: &str = "";

#[derive(Clone, Hash, Eq, PartialEq, Copy)]
pub enum Symbol {
    Fn,
    Struct,
    Global,
}

#[derive(Clone, Default)]
pub struct Scope<'arena> {
    pub symbols: IndexMap<(&'arena str, Symbol), u16, FxBuildHasher>,
    pub children: Vec<(&'arena str, Self)>,
    /// Number of top-level symbols
    pub toplevel_count: usize,
}

impl Scope<'_> {
    #[cold]
    pub fn fns(&self) -> impl Iterator<Item = (&str, u16)> {
        self.symbols.iter().filter(|((_, kind), _)| *kind == Symbol::Fn).map(|(&(k, _), &v)| (k, v))
    }
    #[cold]
    pub fn structs(&self) -> impl Iterator<Item = (&str, u16)> {
        self.symbols
            .iter()
            .filter(|((_, kind), _)| *kind == Symbol::Struct)
            .map(|(&(k, _), &v)| (k, v))
    }
    #[must_use]
    pub fn find_function(
        &self,
        path: &[&str],
        function_name: &str,
        span: Span,
        file_idx: u16,
        sources: &[Source],
    ) -> Option<usize> {
        self.walk_to_namespace(path)
            .unwrap_or_else(|| error_unknown_namespace(path, span, file_idx, sources))
            .symbols
            .get(&(function_name, Symbol::Fn))
            .map(|symbol| *symbol as usize)
    }
    #[must_use]
    pub fn find_function_fallible(&self, path: &[&str], function_name: &str) -> Option<usize> {
        self.walk_to_namespace(path)?
            .symbols
            .get(&(function_name, Symbol::Fn))
            .map(|symbol| *symbol as usize)
    }
    #[must_use]
    pub fn find_struct(
        &self,
        path: &[&str],
        struct_name: &str,
        span: Span,
        file_idx: u16,
        sources: &[Source],
    ) -> Option<usize> {
        self.walk_to_namespace(path)
            .unwrap_or_else(|| error_unknown_namespace(path, span, file_idx, sources))
            .symbols
            .get(&(struct_name, Symbol::Struct))
            .map(|symbol| *symbol as usize)
    }
    #[must_use]
    pub fn find_global(
        &self,
        path: &[&str],
        global_var_name: &str,
        span: Span,
        file_idx: u16,
        sources: &[Source],
    ) -> Option<usize> {
        self.walk_to_namespace(path)
            .unwrap_or_else(|| error_unknown_namespace(path, span, file_idx, sources))
            .symbols
            .get(&(global_var_name, Symbol::Global))
            .map(|symbol| *symbol as usize)
    }
    #[must_use]
    pub fn walk_to_namespace(&self, path: &[&str]) -> Option<&Self> {
        let mut current = self;
        for sub in path {
            current = if let Some((_, child_namespace)) =
                current.children.iter().find(|(name, _)| name == sub)
            {
                child_namespace
            } else {
                return None;
            };
        }
        Some(current)
    }
}

/// Recursively collects functions, dyn libs, and imported files
fn parse_toplevel<'a>(
    bump: &'a Bump,
    code: Vec<Expr<'a>>,
    file_path: &Path,
    src_file_idx: u16,
    fns: &mut Vec<Function<'a>>,
    structs: &mut Vec<Struct<'a>>,
    fn_registers: &mut Vec<Vec<u16>>,
    dynamic_libs: &mut Vec<Dylib<'a>>,
    sources: &mut Vec<Source<'a>>,
    scope: &mut Scope<'a>,
    files: &mut FxHashMap<PathBuf, Scope<'a>>,
    file_scopes: &mut Vec<Scope<'a>>,
    pending_structs: &mut Vec<(u16, u16, &'a [(&'a str, TypeExpr<'a>, Span)])>,
    pending_fns: &mut Vec<(u16, u16, &[FunctionDeclarationArgumentExpr<'a>])>,
    #[cfg(not(target_arch = "wasm32"))] pending_dylibs: &mut Vec<(
        u16,
        u16,
        &[DylibFnExpr<'a>],
        Rc<Library>,
        Span,
    )>,
    pending_globals: &mut Vec<(VariableDeclarationExpr<'a>, u16)>,
    keel_home_libs_path: &LazyCell<Option<PathBuf>, impl FnOnce() -> Option<PathBuf>>,
) {
    let mut imports = Vec::new();
    let mut file_globals: Vec<VariableDeclarationExpr> = Vec::new();
    for expr in code {
        match expr {
            Expr::VarDeclare(var_declaration) => file_globals.push(var_declaration),
            Expr::FunctionDecl(function_declaration) => {
                let fn_name = function_declaration.name;
                let span = function_declaration.span;
                let fn_code = function_declaration.code;
                let fn_args = function_declaration.args;
                if let Some(func_idx) =
                    scope.find_function(&[], fn_name, span, src_file_idx, sources)
                {
                    compiler_errors::error_function_already_defined(
                        &fns[func_idx],
                        span,
                        src_file_idx,
                        sources,
                    );
                }
                fn_registers.push(Vec::new());
                let returns_void = check_if_returns_void(fn_code);
                let mut callees = Vec::new();
                collect_direct_fn_calls(fn_code, &mut callees);

                let fn_id = fns.len() as u16;
                fns.push(Function {
                    name: fn_name,
                    args: Box::new([]),
                    code: fn_code,
                    impls: Vec::new(),
                    is_recursive: None,
                    returns_null: returns_void,
                    src_file_idx,
                    return_type_cache: Vec::new(),
                    direct_calls: bump.alloc_slice_copy(&callees),
                    name_span: span,
                });
                pending_fns.push((fn_id, src_file_idx, fn_args));
                if let Some(func_idx) = scope.symbols.insert((fn_name, Symbol::Fn), fn_id) {
                    compiler_errors::error_function_already_defined(
                        &fns[func_idx as usize],
                        span,
                        src_file_idx,
                        sources,
                    );
                }
            }
            Expr::StructDeclare(struct_name, fields, span) => {
                let struct_id = structs.len() as u16;
                structs.push(Struct {
                    name: struct_name,
                    fields: Box::from([]),
                    id: struct_id,
                    name_span: span,
                    src_file_idx,
                });
                if let Some(struct_id) =
                    scope.symbols.insert((struct_name, Symbol::Struct), struct_id)
                {
                    error_struct_already_defined(
                        &structs[struct_id as usize],
                        span,
                        src_file_idx,
                        sources,
                    );
                }
                pending_structs.push((struct_id, src_file_idx, fields));
            }
            #[cfg(target_arch = "wasm32")]
            Expr::ImportDylib(..) => wasm_error("WASM does not support loading dynamic libraries"),
            #[cfg(target_arch = "wasm32")]
            Expr::ImportFile(..) => wasm_error("WASM does not support importing files"),
            import @ (Expr::ImportFile(..) | Expr::ImportDylib(..)) => imports.push(import),
            _ => {}
        }
    }

    files.insert(file_path.to_path_buf(), scope.clone());

    for import in imports {
        match import {
            #[cfg(not(target_arch = "wasm32"))]
            Expr::ImportDylib(DylibImportExpr { path, functions, span }) => {
                let base_path = if Path::new(path).is_relative() {
                    &file_path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(path)
                        .to_string_lossy()
                        .to_string()
                } else {
                    path
                };
                let dylib_name = bump.alloc_str(
                    std::path::PathBuf::from(base_path)
                        .file_prefix()
                        .and_then(|s| s.to_str())
                        .unwrap_or(base_path),
                );
                // If the extension is omitted, the extension is chosen based on the target OS.
                // An architecture-specific suffix is also tried before the extension
                let lib = Rc::new(unsafe {
                    if Path::new(base_path).extension().is_none() {
                        let path = {
                            let arch_path = hformat!({ base_path }, ARCH_SUFFIX, ".", DYLIB_EXT);
                            if Path::new(&arch_path).exists() {
                                arch_path
                            } else {
                                hformat!({ base_path }, ".", DYLIB_EXT)
                            }
                        };
                        libloading::Library::new(path).unwrap_or_else(|_| {
                            error_cannot_load_dynlib(span, src_file_idx, sources);
                        })
                    } else {
                        libloading::Library::new(base_path).unwrap_or_else(|_| {
                            error_cannot_load_dynlib(span, src_file_idx, sources);
                        })
                    }
                });
                pending_dylibs.push((
                    src_file_idx,
                    dynamic_libs.len() as u16,
                    functions,
                    lib,
                    span,
                ));
                dynamic_libs.push(Dylib { name: dylib_name, fns: Box::new([]) });
            }
            Expr::ImportFile(path, alias, span) => {
                let file_path = file_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(path)
                    .canonicalize()
                    .unwrap_or_else(|_| {
                        keel_home_libs_path
                            .as_ref()
                            .unwrap_or_else(
                                #[cold]
                                || error_cannot_read_file(span, src_file_idx, sources),
                            )
                            .join(path)
                    });

                let child_name = alias.unwrap_or_else(|| {
                    bump.alloc_str(file_path.file_prefix().and_then(|s| s.to_str()).unwrap_or(path))
                });

                if let Some(cached) = files.get(&file_path) {
                    scope.children.push((child_name, cached.clone()));
                    continue;
                }

                let file_contents =
                    bump.alloc_str(&std::fs::read_to_string(&file_path).unwrap_or_else(|_| {
                        error_cannot_read_file(span, src_file_idx, sources);
                    }));
                let file_name = bump.alloc_str(file_path.to_str().unwrap_or(path));

                let child_src_idx = sources.len() as u16;

                // bump.alloc_str(&file_contents);

                let src = Source { filename: file_name, contents: file_contents };

                let file_code = parser::parse(src.contents, src, bump);

                sources.push(src);

                // Parse the imported file's contents

                let mut child_scope = Scope::default();

                let file_path_cloned = file_path.clone();
                parse_toplevel(
                    bump,
                    file_code,
                    &file_path,
                    child_src_idx,
                    fns,
                    structs,
                    fn_registers,
                    dynamic_libs,
                    sources,
                    &mut child_scope,
                    files,
                    file_scopes,
                    pending_structs,
                    pending_fns,
                    #[cfg(not(target_arch = "wasm32"))]
                    pending_dylibs,
                    pending_globals,
                    keel_home_libs_path,
                );
                files.insert(file_path_cloned, child_scope.clone());
                scope.children.push((child_name, child_scope));
            }
            _ => unsafe { unreachable_unchecked() },
        }
    }

    for var_declaration in file_globals {
        if let Some(previous_global) = scope
            .symbols
            .insert((var_declaration.name, Symbol::Global), pending_globals.len() as u16)
        {
            error_global_already_defined(
                var_declaration.name,
                pending_globals[previous_global as usize].0.span,
                var_declaration.span,
                src_file_idx,
                sources,
            );
        }
        pending_globals.push((var_declaration, src_file_idx));
    }

    if file_scopes.len() <= src_file_idx as usize {
        file_scopes.resize_with(src_file_idx as usize + 1, Scope::default);
    }
    scope.toplevel_count = scope.symbols.len();
    file_scopes[src_file_idx as usize] = scope.clone();
}

fn resolve_types<'arena>(
    structs: &mut [Struct<'arena>],
    fns: &mut [Function<'arena>],
    pending_structs: Vec<(u16, u16, &'arena [(&str, TypeExpr, Span)])>,
    pending_fns: Vec<(u16, u16, &[FunctionDeclarationArgumentExpr<'arena>])>,
    #[cfg(not(target_arch = "wasm32"))] pending_dylibs: Vec<(
        u16,
        u16,
        &[DylibFnExpr<'arena>],
        Rc<Library>,
        Span,
    )>,
    file_namespaces: &[Scope],
    dynamic_libs_fns: &mut Vec<DylibFn>,
    dynamic_libs: &mut [Dylib<'arena>],
    sources: &[Source],
) {
    for (struct_id, src_file_idx, fields) in pending_structs {
        let resolved_fields = fields
            .iter()
            .map(|(field_name, field_type, field_span)| StructField {
                name: field_name,
                field_type: field_type.to_datatype(
                    src_file_idx,
                    &file_namespaces[src_file_idx as usize],
                    sources,
                ),
                span: *field_span,
            })
            .collect();
        structs[struct_id as usize].fields = resolved_fields;
    }
    for (fn_id, src_file_idx, args) in pending_fns {
        let resolved_args = args
            .iter()
            .map(|arg| {
                (
                    arg.name,
                    arg.enforced_type.map(|t_e| {
                        t_e.to_datatype(
                            src_file_idx,
                            &file_namespaces[src_file_idx as usize],
                            sources,
                        )
                    }),
                )
            })
            .collect::<Vec<(&str, Option<DataType>)>>()
            .into_boxed_slice();
        fns[fn_id as usize].args = resolved_args;
    }
    #[cfg(not(target_arch = "wasm32"))]
    for (src_file_idx, dynlib_id, fn_signatures, lib, span) in pending_dylibs {
        let namespace = &file_namespaces[src_file_idx as usize];
        let fns = fn_signatures
            .iter()
            .map(|DylibFnExpr { name, args, name_span }| {
                use crate::compiler::type_system::datatype_to_vmtype;

                let (fn_return_type, fn_return_type_span) = unsafe { args.get_unchecked(0) };
                let fn_args = args
                    .iter()
                    .skip(1)
                    .map(|(t, span)| (t.to_datatype(src_file_idx, namespace, sources), *span))
                    .collect::<Vec<(DataType, Span)>>()
                    .into_boxed_slice();
                let fn_return_type = fn_return_type.to_datatype(src_file_idx, namespace, sources);
                let return_val = FnSignature {
                    name,
                    args: fn_args.iter().map(|(t, _)| t.clone()).collect(),
                    return_type: fn_return_type.clone(),
                    id: dynamic_libs_fns.len() as u16,
                };
                let arg_types: Vec<_> = fn_args
                    .iter()
                    .map(|(t, span)| t.to_c_type(false, *span, structs, src_file_idx, sources))
                    .collect();
                let return_type = fn_return_type.to_c_type(
                    true,
                    *fn_return_type_span,
                    structs,
                    src_file_idx,
                    sources,
                );
                let cif = libffi::middle::Cif::new(arg_types, return_type);
                let ptr = unsafe {
                    libffi::middle::CodePtr(
                        lib.get::<*const ()>(name.as_bytes())
                            .unwrap_or_else(|_| {
                                error_cannot_find_dynlib_symbol(
                                    name,
                                    *name_span,
                                    span,
                                    src_file_idx,
                                    sources,
                                );
                            })
                            .try_as_raw_ptr()
                            .unwrap_unchecked(),
                    )
                };

                dynamic_libs_fns.push(DylibFn {
                    types: std::iter::once(&fn_return_type)
                        .chain(fn_args.iter().map(|(t, _)| t))
                        .map(datatype_to_vmtype)
                        .collect(),
                    _lib: Rc::clone(&lib),
                    ptr,
                    cif,
                    args_len: fn_args.len(),
                });
                return_val
            })
            .collect();
        dynamic_libs[dynlib_id as usize].fns = fns;
    }
}

pub fn compile<'arena>(
    contents: &str,
    filename: &'arena str,
    debug: bool,
    bump: &'arena Bump,
) -> (
    Vec<Instr>,
    RegisterFile,
    Pools,
    ErrorCtx<'arena>,
    Vec<Vec<u16>>,
    Vec<DylibFn>,
    usize,
    usize,
    Vec<Struct<'arena>>,
    Vec<DataType>,
) {
    #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
    let now = std::time::Instant::now();

    let main_src_contents = bump.alloc_str(contents);

    let main_src = Source { filename, contents: main_src_contents };

    let code = parser::parse(main_src.contents, main_src, bump);

    #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
    if debug {
        println!("PARSING TIME: {:.2?}", now.elapsed());
    }

    let mut variables: Vec<Variable> = Vec::new();
    let mut globals: Vec<Variable> = Vec::new();
    let mut registers: Vec<Data> = Vec::new();
    let mut pools: Pools = Pools {
        obj_pool: Pool::with_capacity(5),
        map_pool: Pool::with_capacity(0),
        str_pool: Pool::with_capacity(5),
    };
    let mut instr_src: Vec<InstrSrc> = Vec::new();
    let mut fn_registers: Vec<Vec<u16>> = Vec::new();
    let mut functions: Vec<Function> = Vec::new();
    let mut structs: Vec<Struct> = Vec::new();
    let mut dyn_libs: Vec<Dylib> = Vec::new();
    let mut dylib_fns: Vec<DylibFn> = Vec::new();
    let mut allocated_arg_count_peak = 0;
    let mut allocated_arg_count = 0;
    let mut allocated_call_depth = 0;
    let mut const_registers: FxHashMap<Data, u16> = FxHashMap::default();
    let mut free_registers = Vec::new();
    let mut types: Vec<DataType> = Vec::new();

    let mut sources: Vec<Source> = vec![main_src];
    let mut scope = Scope::default();

    let mut files: FxHashMap<PathBuf, Scope> = FxHashMap::default();
    let mut file_scopes: Vec<Scope> = Vec::new();
    let mut pending_structs = Vec::new();
    let mut pending_fns = Vec::with_capacity(2);
    #[cfg(not(target_arch = "wasm32"))]
    let mut pending_dylibs = Vec::new();
    let mut pending_globals: Vec<(VariableDeclarationExpr, u16)> = Vec::new();

    let keel_home_libs =
        LazyCell::new(|| std::env::home_dir().map(|p| p.join(".keel").join("libs/")));

    parse_toplevel(
        bump,
        code,
        &PathBuf::from(filename),
        0,
        &mut functions,
        &mut structs,
        &mut fn_registers,
        &mut dyn_libs,
        &mut sources,
        &mut scope,
        &mut files,
        &mut file_scopes,
        &mut pending_structs,
        &mut pending_fns,
        #[cfg(not(target_arch = "wasm32"))]
        &mut pending_dylibs,
        &mut pending_globals,
        &keel_home_libs,
    );
    resolve_types(
        &mut structs,
        &mut functions,
        pending_structs,
        pending_fns,
        #[cfg(not(target_arch = "wasm32"))]
        pending_dylibs,
        &file_scopes,
        &mut dylib_fns,
        &mut dyn_libs,
        &sources,
    );

    let ctx = Ctx {
        block_id: 0,
        is_compiling_recursive: false,
        file_idx: 0,
        single_run: true,
        offset: 0,
    };
    let mut const_registers_bitset = FixedBitSet::new();
    let mut free_registers_bitset = FixedBitSet::new();
    let mut state = State {
        v: &mut variables,
        globals: &mut globals,
        registers: &mut registers,
        functions: &mut functions,
        structs: &mut structs,
        pools: &mut pools,
        instr_src: &mut instr_src,
        fn_registers: &mut fn_registers,
        dylibs: &mut dyn_libs,
        allocated_arg_count: &mut allocated_arg_count,
        allocated_arg_count_peak: &mut allocated_arg_count_peak,
        allocated_call_depth: &mut allocated_call_depth,
        const_registers: &mut const_registers,
        const_registers_bitset: &mut const_registers_bitset,
        free_registers: &mut free_registers,
        free_registers_bitset: &mut free_registers_bitset,
        sources: &mut sources,
        reserved_registers: FxHashSet::default(),
        file_scopes: &mut file_scopes,
        types: &mut types,
        bump,
    };
    let mut instructions: Vec<Instr> = Vec::with_capacity(4);
    for (var_declaration, file_idx) in pending_globals {
        let var_ctx = ctx.with_file_idx(file_idx);
        let value_type = var_declaration.value.infer_type(var_ctx, &mut state);
        let declared_type = if let Some(v_t) = var_declaration.var_type {
            let declared_var_type =
                v_t.0.to_datatype(file_idx, state.scope(file_idx), state.sources);
            if !var_type_is_compatible(&declared_var_type, &value_type) {
                error_invalid_type(
                    &declared_var_type,
                    &value_type,
                    v_t.1,
                    None,
                    None,
                    file_idx,
                    &sources,
                );
            }
            declared_var_type
        } else {
            value_type.clone()
        };
        let register_id = var_declaration
            .value
            .compile(var_ctx, &mut state, &mut instructions, None, true, true)
            .unwrap_id();
        state.reserved_registers.insert(register_id);
        state.globals.push(Variable {
            name: var_declaration.name,
            register_id,
            declared_type,
            var_type: value_type,
        });
    }
    let program_instructions = compile_expr(
        state.functions
            .iter()
            .find(|func| func.name == "main" && func.src_file_idx == 0)
            .unwrap_or_else(|| {
                #[cfg(target_arch = "wasm32")]
                wasm_error("Cannot find main function");

                eprintln!(
                    "--------------\n{RED}KEEL RUNTIME ERROR:{RESET}\nCannot find {BLUE}{BOLD}main{RESET} function\n--------------",
                );
                std::process::exit(1);
            })
            .code,
        // &mut variables,
        ctx.with_offset(instructions.len() as u16),
        &mut state,
    );
    instructions.reserve_exact(program_instructions.len() + 1);
    instructions.extend(program_instructions);
    instructions.push(Instr::Halt(0));
    fn_registers.push(Vec::new());

    #[cfg(debug_assertions)]
    if debug {
        println!("---- DEBUG ----");
        if !pools.obj_pool.is_empty() {
            println!("---  OBJECTS  ---");
            for (i, data) in pools.obj_pool.iter().enumerate() {
                println!(" {i} {data:?}");
            }
        }
        println!("-- REGISTERS --");
        for (i, data) in registers.iter().enumerate() {
            println!(
                " [{i}] {}",
                data.format(&pools.obj_pool, &pools.str_pool, &pools.map_pool, &structs, true)
            );
        }
        if !instructions.is_empty() {
            println!("-- INSTRUCTIONS --");
            for (i, instr) in instructions.iter().enumerate() {
                println!(" {i}: {instr:?}");
            }
        }
        println!("------------------");
    }

    (
        instructions,
        RegisterFile(registers),
        pools,
        ErrorCtx { instr_src, sources },
        fn_registers,
        dylib_fns,
        allocated_arg_count_peak,
        allocated_call_depth,
        structs,
        types,
    )

    // VmData {
    //     instructions,
    //     registers: RegisterFile(registers),
    //     pools,
    //     err_ctx: ErrorCtx { instr_src, sources },
    //     fn_registers,
    //     dylib_fns,
    //     allocated_arg_count,
    //     allocated_call_depth,
    //     structs,
    // }
}
