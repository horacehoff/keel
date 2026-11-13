use super::compile_reg_window;
use crate::ast::Expr;
use crate::ast::QualifiedName;
use crate::ast::Span;
use crate::compiler::DataType;
use crate::compiler::UnwrapId;
use crate::compiler::compiler_errors;
use crate::compiler::compiler_errors::error_invalid_index_type;
use crate::compiler::compiler_errors::error_type_not_indexable;
use crate::compiler::error_invalid_type;
use crate::compiler::functions::compile_function_impl;
use crate::compiler::registers::to_nested_operand;
use crate::compiler::state::Ctx;
use crate::compiler::state::State;
use crate::compiler::state::Variable;
use crate::data::Data;
use crate::instr::Instr;

#[must_use]
pub(super) fn compile_struct_field_access<'arena>(
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
pub(super) fn compile_array_indexing<'arena>(
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
    let len_after_array = output.len();

    let index_inferred = index.infer_type(ctx, state);
    if index_inferred != DataType::Int {
        error_invalid_index_type(&index_inferred, span, ctx.file_idx, state.sources);
    }
    let index_id = index.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(index_id);
    let dest_reg_id = state.alloc_reg();

    // Fuses two ArrayGetIndex into a single ArrayNestedGet
    if inferred != DataType::String
        && matches!(array, Expr::ArrayGetIndex(..))
        && output.len() == len_after_array
        && let Some(last) = output.last_mut()
        && let Instr::GetIndexArray(outer_id, outer_index_id, inner_id) = *last
        && inner_id == id
        && let (Some(o1), Some(o2), Some(o3), Some(o4)) = (
            to_nested_operand(outer_id),
            to_nested_operand(outer_index_id),
            to_nested_operand(index_id),
            to_nested_operand(dest_reg_id),
        )
    {
        *last = Instr::nested_index_array_instr((o1, o2, o3, o4));
    } else if inferred == DataType::String {
        output.push(Instr::GetIndexString(id, index_id, dest_reg_id));
    } else {
        output.push(Instr::GetIndexArray(id, index_id, dest_reg_id));
    }
    state.add_to_src(ctx, output, span);
    dest_reg_id
}

#[must_use]
pub(super) fn compile_array_slice<'arena>(
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
    let idx_end_inferred = idx_end.infer_type(ctx, state);
    if idx_end_inferred != DataType::Int {
        error_invalid_index_type(&idx_end_inferred, span, ctx.file_idx, state.sources);
    }
    let reg_window = compile_reg_window([idx_start, idx_end].into_iter(), ctx, state, output);
    let dest_reg_id = state.alloc_reg();
    let to_push = if inferred == DataType::String {
        Instr::GetSliceString(id, reg_window, dest_reg_id)
    } else {
        Instr::GetSliceArray(id, reg_window, dest_reg_id)
    };
    output.push(to_push);
    state.add_to_src(ctx, output, span);
    state.free_reg_window(reg_window, 2);
    dest_reg_id
}

#[must_use]
pub(super) fn compile_var_access(
    qualified_name: &QualifiedName,
    span: Span,
    ctx: Ctx,
    state: &mut State<'_, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let name = qualified_name.get_name();
    let path = qualified_name.get_namespace();
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
        // Global variables live in the top-level frame
        let register_id = state.globals[idx].register_id;
        if !ctx.in_function {
            return register_id;
        }
        let reg = state.alloc_reg();
        output.push(Instr::LoadGlobal { dest_reg: reg, reg: u32::from(register_id) });
        return reg;
    }

    // Function referenced by name
    let Some(fn_id) =
        state.scope(ctx.file_idx).find_function(path, name, span, ctx.file_idx, state.sources)
    else {
        compiler_errors::error_unknown_variable(name, span, state.v, ctx.file_idx, state.sources);
    };

    let arg_types: Vec<DataType> =
        state.functions[fn_id].args.iter().map(|(_, t)| t.clone().unwrap()).collect();

    let fn_impl_idx = compile_function_impl(state, fn_id, &arg_types);
    let unit = state.functions[fn_id].impls[fn_impl_idx].unit;
    state.value_reg(Data::function(unit))
}
