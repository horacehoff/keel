use super::expr::Expr;
use super::expr::Span;
use super::type_system::DataType;
use super::type_system::fn_matches_signature;
use crate::compiler::UnwrapId;
use crate::compiler::compiler_data::Ctx;
use crate::compiler::compiler_data::State;
use crate::compiler::compiler_errors::check_args;
use crate::compiler::compiler_errors::error_function_arg_invalid_type;
use crate::compiler::compiler_errors::error_function_arg_invalid_type_multiple;
use crate::compiler::compiler_errors::error_unknown_function_in_namespace;
use crate::compiler::expr::FunctionCallExpr;
use crate::instr::Instr;
use builtin_functions::builtin_functions;
use fs_lib_functions::fs_lib_functions;
use std::slice;
use user_functions::handle_user_function;

pub mod user_functions;

#[path = "builtin/builtin_functions.rs"]
mod builtin_functions;

#[path = "fs/fs_lib_functions.rs"]
mod fs_lib_functions;

#[cfg(target_arch = "wasm32")]
use crate::errors::wasm_error;

pub fn check_arg_type(
    fn_name: &str,
    ctx: Ctx,
    state: &mut State<'_>,
    args: &[Expr],
    args_indexes: &[Span],
    arg_idx: usize,
    expected: &[DataType],
) {
    let inferred = args[arg_idx].infer_type(ctx, state);
    let matches = if let DataType::Union(polytype) = &inferred {
        polytype.iter().all(|x| expected.contains(x))
    } else {
        expected.contains(&inferred)
    };
    if !matches {
        error_function_arg_invalid_type_multiple(
            &inferred,
            expected,
            args_indexes[arg_idx],
            fn_name,
            None,
            ctx.file_idx,
            state.sources,
        )
    }
}

pub fn check_user_fn_arg_types(
    fn_id: usize,
    fn_name: &str,
    inferred_arg_types: &[DataType],
    args_indexes: &[Span],
    ctx: Ctx,
    state: &mut State<'_>,
) {
    let args_len = state.fns[fn_id].args.len();
    for i in 0..args_len {
        let t = state.fns[fn_id].args[i].1.clone();
        if let Some(t) = &t
            && if let (DataType::FnSignature(expected_sig), DataType::Fn(concrete_id)) =
                (t, &inferred_arg_types[i])
            {
                !fn_matches_signature(*concrete_id as usize, expected_sig, ctx, state)
            } else {
                inferred_arg_types[i] != *t
            }
        {
            error_function_arg_invalid_type(
                &inferred_arg_types[i],
                t,
                args_indexes[i],
                fn_name,
                Some((state.fns[fn_id].name_span, state.fns[fn_id].src_file)),
                ctx.file_idx,
                state.sources,
            );
        }
    }
}

pub fn compile_function_call(
    function_call: &FunctionCallExpr,
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'_>,
    tgt_id: Option<u16>,
) -> Option<u16> {
    let namespace = function_call.qualified_name.get_namespace();
    if namespace.is_empty() {
        builtin_functions(output, ctx, state, tgt_id, function_call)
    } else if namespace == ["fs"] {
        #[cfg(target_arch = "wasm32")]
        wasm_error("WASM does not support the file system library");

        fs_lib_functions(output, ctx, state, tgt_id, function_call)
    } else if let Some((fn_args, returns_null, dyn_id)) = state
        .dyn_libs
        .iter()
        .find(|l| l.name == namespace[0])
        .and_then(|lib| lib.fns.iter().find(|x| &x.name == function_call.qualified_name.get_name()))
        .map(|sig| (sig.args.clone(), sig.return_type == DataType::Null, sig.id))
    {
        check_args(
            &function_call.args,
            fn_args.len(),
            function_call.qualified_name.get_name(),
            function_call.span,
            state.sources,
            ctx.file_idx,
        );
        for (i, a) in fn_args.iter().enumerate() {
            check_arg_type(
                function_call.qualified_name.get_name(),
                ctx,
                state,
                &function_call.args,
                &function_call.arg_spans,
                i,
                slice::from_ref(a),
            );
        }

        *state.allocated_arg_count = (*state.allocated_arg_count).max(function_call.args.len());
        for arg in &function_call.args {
            let arg_id = arg.compile(ctx, state, output, None, false, true).unwrap_id();
            output.push(Instr::StoreFuncArg(arg_id));
            state.free_reg(arg_id);
        }

        let register_id = if returns_null { 0 } else { state.alloc_reg_tgt(tgt_id) };
        output.push(Instr::CallDynamicLibFunc(dyn_id, register_id));
        state.add_to_src(ctx, output, function_call.span);
        if returns_null { None } else { Some(register_id) }
    } else if let Some(fn_id) = state.scope.find_function(
        namespace,
        function_call.qualified_name.get_name(),
        function_call.span,
        ctx.file_idx,
        state.sources,
    ) {
        handle_user_function(function_call, fn_id, output, ctx, state, tgt_id)
    } else {
        error_unknown_function_in_namespace(
            function_call.qualified_name.get_name(),
            state.scope,
            namespace,
            function_call.span,
            ctx.file_idx,
            state.sources,
        );
    }
}
