use crate::ast::Expr;
use crate::ast::QualifiedName;
use crate::compiler::Span;
use crate::compiler::StructFieldExpr;
use crate::compiler::compiler_errors;
use crate::compiler::compiler_errors::error_duplicate_map_key;
use crate::compiler::compiler_errors::error_map_diff_types;
use crate::compiler::compiler_errors::error_not_literal_map_key;
use crate::compiler::registers::UnwrapId;
use crate::compiler::state::Ctx;
use crate::compiler::state::State;
use crate::compiler::types::DataType;
use crate::compiler::types::struct_field_type_matches;
use crate::data::Data;
use crate::data::NULL;
use crate::instr::Instr;
use core::hint::unreachable_unchecked;
use rustc_hash::FxBuildHasher;
use std::collections::HashMap;

#[must_use]
pub(super) fn compile_array_literal<'arena>(
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
        let array_reg = state.scratch_reg();
        output.push(Instr::EmptyArray(array_reg));
        return array_reg;
    }
    if ctx.single_run {
        let array_reg = state.init_reg(Data::array(array_id as u32), ctx);
        for elem in array_items {
            if let Some(literal) = elem.literal_value(&mut state.pools.str_pool) {
                state.pools.obj_pool[array_id].push(literal);
            } else {
                let id = elem.compile(ctx, state, output, None, false, true).unwrap_id();
                output.push(Instr::SetFieldObj(
                    array_reg,
                    id,
                    state.pools.obj_pool[array_id].len() as u16,
                ));
                state.pools.obj_pool[array_id].push(NULL);
            }
        }
        array_reg
    } else if array_items.iter().all(Expr::is_constant_literal) {
        for elem in array_items {
            let Some(literal) = elem.literal_value(&mut state.pools.str_pool) else {
                unsafe { unreachable_unchecked() }
            };
            state.pools.obj_pool[array_id].push(literal);
        }
        // The template array is held by a register to prevent it from being freed by the GC
        let template_reg = state.value_reg(Data::array(array_id as u32));
        let dest_reg = state.scratch_reg();
        output.push(Instr::CloneObj(template_reg, dest_reg, array_items.len() as u16));
        dest_reg
    } else {
        let mut elem_ids: Vec<u16> = Vec::with_capacity(array_items.len());
        for elem in array_items {
            elem_ids.push(elem.compile(ctx, state, output, None, false, true).unwrap_id());
        }
        let dest_reg = state.scratch_reg();
        output.push(Instr::EmptyArray(dest_reg));
        for elem_reg in elem_ids {
            output.push(Instr::Push(dest_reg, elem_reg));
        }
        dest_reg
    }
}

#[must_use]
pub(super) fn compile_struct_literal<'arena>(
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
    let struct_data = Data::struct_instance(type_id, struct_id as u32);
    let struct_reg = ctx.single_run.then(|| state.init_reg(struct_data, ctx));
    let mut dynamic: Vec<(u16, u16)> = Vec::with_capacity(expected_fields_len);
    for field_idx in 0..expected_fields_len {
        if let Some(StructFieldExpr { name: _, value, name_span: _, value_span }) = fields
            .iter()
            .find(|field| field.name == state.structs[expected_struct_idx].fields[field_idx].name)
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
            if let Some(literal) = value.literal_value(&mut state.pools.str_pool) {
                state.pools.obj_pool[struct_id].push(literal);
            } else {
                let id = value.compile(ctx, state, output, None, false, true).unwrap_id();
                state.pools.obj_pool[struct_id].push(NULL);
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

    let dest_reg = struct_reg.unwrap_or_else(|| {
        let template_reg = state.value_reg(Data::struct_instance(type_id, struct_id as u32));
        let dest_reg = state.scratch_reg();
        output.push(Instr::CloneObj(template_reg, dest_reg, expected_fields_len as u16));
        dest_reg
    });

    for (val_reg, slot) in dynamic {
        output.push(Instr::SetFieldObj(dest_reg, val_reg, slot));
    }
    dest_reg
}

#[must_use]
pub(super) fn compile_map_literal<'arena>(
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
    let mut keys: Vec<(Data, Span)> = Vec::with_capacity(kv_pairs.len());
    let mut dynamic: Vec<(Data, u16)> =
        Vec::with_capacity(if ctx.single_run { 0 } else { kv_pairs.len() });
    let map_reg = if ctx.single_run {
        state.init_reg(Data::map(map_id as u32), ctx)
    } else {
        0 // placeholder that's never used
    };
    for (i, (key, key_span, val, val_span)) in kv_pairs.iter().enumerate() {
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
        let key_val = if let Some(literal) = key.literal_value(&mut state.pools.str_pool) {
            literal
        } else {
            let output_len = output.len();
            let key_val_id = key.compile(ctx, state, output, None, false, true).unwrap_id();
            if !(matches!(key, Expr::Array(_, _)) && output_len == output.len()) {
                error_not_literal_map_key(*key_span, map_span, ctx.file_idx, state.sources);
            }
            state.register_value(key_val_id)
        };

        if let Some((_, repeat_key_span)) = keys.iter().find(|(k, _)| *k == key_val) {
            error_duplicate_map_key(
                *repeat_key_span,
                *key_span,
                map_span,
                ctx.file_idx,
                state.sources,
            );
        }
        keys.push((key_val, *key_span));

        if let Some(literal) = val.literal_value(&mut state.pools.str_pool) {
            state.pools.map_pool[map_id].insert(key_val, literal);
        } else {
            let val_id: u16 = val.compile(ctx, state, output, None, false, true).unwrap_id();
            state.pools.map_pool[map_id].insert(key_val, NULL);
            if ctx.single_run {
                output.push(Instr::MapInsertReg(map_reg, state.value_reg(key_val), val_id));
            } else {
                dynamic.push((key_val, val_id));
            }
        }
    }
    if ctx.single_run {
        map_reg
    } else {
        let template_reg = state.value_reg(Data::map(map_id as u32));
        let dest_reg = state.scratch_reg();
        output.push(Instr::CloneMap(template_reg, dest_reg));
        for (key_val, val_id) in dynamic {
            let key_reg = state.new_const_reg(key_val);
            output.push(Instr::MapInsertReg(dest_reg, key_reg, val_id));
        }
        dest_reg
    }
}
