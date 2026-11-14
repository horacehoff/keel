#![allow(clippy::unnecessary_wraps)]
use crate::ast::FunctionCallExpr;
use crate::compiler::Ctx;
use crate::compiler::DataType;
use crate::compiler::compiler_errors::error_expected_function;
use crate::compiler::compiler_errors::error_unknown_function;
use crate::compiler::compiler_errors::error_unknown_function_in_namespace;
use crate::compiler::functions::builtin::check_builtin_call;
use crate::compiler::functions::builtin::compile_builtin_call;
use crate::compiler::functions::builtin::find_builtin;
use crate::compiler::functions::builtin::find_builtin_global;
use crate::compiler::functions::builtin::find_library;
use crate::compiler::functions::builtin::find_method;
use crate::compiler::functions::dyn_lib::compile_dyn_lib_function;
use crate::compiler::functions::dyn_lib::find_dyn_lib_function;
use crate::compiler::scope::Scope;
use crate::compiler::state::State;
use crate::instr::Instr;
pub use builtin::get_builtin_function_return_type;
pub use builtin::get_method_return_type;
pub(super) use user_defined::compile_function_impl;
use user_defined::handle_user_function;

mod builtin;
mod dyn_lib;
mod user_defined;

#[cfg(target_arch = "wasm32")]
use crate::errors::wasm_error;

pub fn compile_function_call<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    tgt_id: Option<u16>,
) -> Option<u16> {
    let namespace = function_call.qualified_name.get_namespace();
    let name = function_call.qualified_name.get_name();
    let span = function_call.get_call_span();
    if namespace.is_empty() {
        if let Some(function) = find_builtin_global(name) {
            check_builtin_call(function, function_call, None, ctx, state);
            compile_builtin_call(function, function_call, output, ctx, state, tgt_id)
        } else if let Some(var) = state.find_var(name) {
            let DataType::Fn(fn_id) = var.var_type else {
                error_expected_function(&var.var_type, span, ctx.file_idx, state.sources);
            };
            handle_user_function(function_call, fn_id as usize, output, ctx, state, tgt_id)
        } else if let Some(fn_id) =
            state.scope(ctx.file_idx).find_function(&[], name, span, ctx.file_idx, state.sources)
        {
            handle_user_function(function_call, fn_id, output, ctx, state, tgt_id)
        } else {
            error_unknown_function(
                name,
                span,
                state.scope(ctx.file_idx),
                ctx.file_idx,
                state.sources,
            )
        }
    } else if let Some(library) = find_library(namespace) {
        #[cfg(target_arch = "wasm32")]
        if !library.supported_on_wasm {
            wasm_error(&hybrid_format::hformat!(
                "WASM does not support the {} library",
                library.name
            ));
        }

        let Some(function) = find_builtin(library.functions, name) else {
            error_unknown_function(name, span, &Scope::default(), ctx.file_idx, state.sources)
        };

        check_builtin_call(function, function_call, None, ctx, state);
        compile_builtin_call(function, function_call, output, ctx, state, tgt_id)
    } else if let Some((lib_idx, lib_fn_idx)) =
        find_dyn_lib_function(namespace.last().unwrap(), name, state)
    {
        compile_dyn_lib_function(function_call, lib_idx, lib_fn_idx, output, state, ctx, tgt_id)
    } else if let Some(fn_id) =
        state.scope(ctx.file_idx).find_function(namespace, name, span, ctx.file_idx, state.sources)
    {
        handle_user_function(function_call, fn_id, output, ctx, state, tgt_id)
    } else {
        error_unknown_function_in_namespace(
            name,
            state.scope(ctx.file_idx),
            namespace,
            span,
            ctx.file_idx,
            state.sources,
        );
    }
}

#[inline(always)]
pub fn compile_method_call<'arena>(
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    tgt_id: Option<u16>,
    function_call: &'arena FunctionCallExpr<'arena>,
) -> Option<u16> {
    let (method, receiver_type) = find_method(function_call, ctx, state);
    check_builtin_call(method, function_call, Some(&receiver_type), ctx, state);
    compile_builtin_call(method, function_call, output, ctx, state, tgt_id)
}
