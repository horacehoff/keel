use crate::compiler::Ctx;
use crate::compiler::DataType;
use crate::compiler::Expr;
use crate::compiler::Span;
use crate::compiler::State;
use crate::compiler::StructField;
use crate::compiler::StructFieldAssignmentExpr;
use crate::compiler::VariableDeclarationExpr;
use crate::compiler::code_modifies_variable;
use crate::compiler::compiler_errors;
use crate::compiler::error_cannot_push_type_to_array;
use crate::compiler::error_invalid_type;
use crate::compiler::error_type_not_indexable;
use crate::compiler::move_reg_to_reg;
use crate::compiler::move_value_to;
use crate::compiler::registers::UnwrapId;
use crate::compiler::struct_field_type_matches;
use crate::compiler::var_type_is_compatible;
use crate::instr::Instr;

pub(super) fn compile_array_index_assignment<'arena>(
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
    if array_type == DataType::String
        && ctx.in_function
        && let Expr::Var(var_name, var_span) = array
        && !(var_name.is_namespace_empty() && state.find_var(var_name.get_name()).is_some())
        && let Some(global_idx) = state.scope(ctx.file_idx).find_global(
            var_name.get_namespace(),
            var_name.get_name(),
            *var_span,
            ctx.file_idx,
            state.sources,
        )
    {
        let reg = u32::from(state.globals[global_idx].register_id);
        output.push(Instr::StoreGlobal { src_reg: id, reg });
    }
    state.free_reg(id);
}

pub(super) fn compile_struct_field_assignment<'arena>(
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
    output.push(Instr::SetFieldObj(id, new_elem_reg_id, field_index));
}

pub(super) fn compile_var_declaration<'arena>(
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
        move_reg_to_reg(output, src_id, var_id, state.register_value(src_id));
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

pub(super) fn compile_var_assignment<'arena>(
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

    let global_reg = (ctx.in_function && local_var_idx.is_none()).then_some(u32::from(reg_id));
    let reg_id = if global_reg.is_some() { state.alloc_reg() } else { reg_id };

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
            output.push(Instr::AddIntImm {
                src: src_id,
                dst: reg_id,
                imm: if is_inc { 1 } else { (-1i16) as u16 },
            });
            if let Some(global_reg) = global_reg {
                output.push(Instr::StoreGlobal { src_reg: reg_id, reg: global_reg });
                state.free_reg(reg_id);
            }
            return;
        }
    }

    let output_len = output.len();
    let obj_id = value.compile(ctx, state, output, Some(reg_id), false, true).unwrap_id();
    if state.is_register_const(obj_id) {
        move_reg_to_reg(output, obj_id, reg_id, state.register_value(obj_id));
    } else {
        move_value_to(output, output_len, obj_id, reg_id);
    }
    if let Some(global_reg) = global_reg {
        output.push(Instr::StoreGlobal { src_reg: reg_id, reg: global_reg });
        state.free_reg(reg_id);
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
    if ctx.in_function {
        return None;
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
