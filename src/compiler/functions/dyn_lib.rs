use crate::{
    ast::FunctionCallExpr,
    compiler::{
        DataType,
        codegen::compile_reg_window,
        compiler_errors::{check_args_length, error_function_arg_invalid_type},
        program::FnSignature,
        state::{Ctx, State},
        types::c_arg_matches,
    },
    instr::Instr,
};

#[inline(always)]
pub(super) fn find_dyn_lib_function(
    lib_name: &str,
    name: &str,
    state: &State,
) -> Option<(usize, usize)> {
    let lib_idx = state.dylibs.iter().position(|l| l.name == lib_name)?;
    let lib_fn_idx = state.dylibs[lib_idx].fns.iter().position(|s| s.name == name)?;
    Some((lib_idx, lib_fn_idx))
}

pub(super) fn compile_dyn_lib_function<'arena>(
    function_call: &'arena FunctionCallExpr,
    lib_idx: usize,
    lib_fn_idx: usize,
    output: &mut Vec<Instr>,
    state: &mut State<'arena, '_>,
    ctx: Ctx,
    tgt_id: Option<u16>,
) -> Option<u16> {
    let span = function_call.get_call_span();
    let FnSignature { name, id: dyn_fn_id, .. } = state.dylibs[lib_idx].fns[lib_fn_idx];
    let arg_len = state.dylibs[lib_idx].fns[lib_fn_idx].args.len();
    let returns_null = state.dylibs[lib_idx].fns[lib_fn_idx].return_type == DataType::Null;

    check_args_length(function_call.args, arg_len, name, span, state.sources, ctx.file_idx);
    for i in 0..arg_len {
        let inferred_arg_type = function_call.args[i].infer_type(ctx, state);
        let declared_type = &state.dylibs[lib_idx].fns[lib_fn_idx].args[i];
        if !c_arg_matches(&inferred_arg_type, declared_type) {
            error_function_arg_invalid_type(
                &inferred_arg_type,
                declared_type,
                function_call.get_nth_arg_span(i),
                name,
                None,
                ctx.file_idx,
                state.sources,
            );
        }
    }

    let first_arg = compile_reg_window(function_call.args.iter(), ctx, state, output);
    let dest_reg_id = if returns_null { state.alloc_reg() } else { state.alloc_reg_tgt(tgt_id) };
    output.push(Instr::CallDynamicLibFunc { fn_id: dyn_fn_id, dst: dest_reg_id, first_arg });
    state.add_to_src(ctx, output, span);
    state.free_reg_window(first_arg, function_call.args.len() as u16);
    if returns_null {
        state.free_reg(dest_reg_id);
        None
    } else {
        Some(dest_reg_id)
    }
}
