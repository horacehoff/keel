use crate::compiler::Ctx;
use crate::compiler::DataType;
use crate::compiler::Function;
use crate::compiler::FunctionDeclarationExpr;
use crate::compiler::Span;
use crate::compiler::State;
use crate::compiler::Struct;
use crate::compiler::StructField;
use crate::compiler::Symbol;
use crate::compiler::TypeExpr;
use crate::compiler::check_if_returns_void;
use crate::compiler::collect_direct_fn_calls;
use crate::compiler::compiler_errors;
use crate::compiler::error_struct_already_defined;

pub(super) fn compile_struct_definition<'arena>(
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

pub(super) fn compile_function_definition<'arena>(
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
        returns_null: check_if_returns_void(fn_code),
        src_file_idx: ctx.file_idx,
        return_type_cache: Vec::new(),
        direct_calls: state.bump.alloc_slice_copy(&callees),
        name_span: span,
    });
}
