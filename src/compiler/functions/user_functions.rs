use super::super::type_system::DataType;
use super::super::type_system::c_arg_matches;
use super::check_user_fn_arg_types;
use crate::ast::Expr;
use crate::ast::FunctionCallExpr;
use crate::compiler::CodeUnitRegisters;
use crate::compiler::Ctx;
use crate::compiler::State;
use crate::compiler::UnwrapId;
use crate::compiler::Variable;
use crate::compiler::codegen::compile_reg_window;
use crate::compiler::compile_expr;
use crate::compiler::compiler_errors::check_args_user_fn;
use crate::compiler::compiler_errors::error_function_arg_invalid_type;
use crate::compiler::program::CodeUnit;
use crate::compiler::program::FunctionImpl;
use crate::compiler::registers::move_value_to;
use crate::compiler::registers::relocate_registers;
use crate::data::Data;
use crate::data::NULL;
use crate::instr::Instr;

/// Returns the index into `state.fns[fn_id].impls` for this specialization (those arg types)
/// matching `inferred_arg_types`. If it doesn't exist yet, it's compiled via `compile_function`.
pub fn compile_function_impl(
    state: &mut State<'_, '_>,
    fn_id: usize,
    inferred_arg_types: &[DataType],
) -> usize {
    let get_fn_impl = |state: &State| {
        state.functions[fn_id]
            .impls
            .iter()
            .position(|fn_impl| fn_impl.arg_types.as_ref() == inferred_arg_types)
    };
    // Try to check if function has already been compiled for these specific arg types
    if let Some(idx) = get_fn_impl(state) {
        return idx;
    }
    // If it hasn't, compile a new specialization of this function
    let fn_args = state.functions[fn_id].args.iter().map(|(a, _)| *a).collect::<Vec<&str>>();
    compile_function(
        state,
        fn_id,
        &fn_args,
        inferred_arg_types,
        state.functions[fn_id].code,
        state.functions[fn_id].src_file_idx,
    );
    unsafe { get_fn_impl(state).unwrap_unchecked() }
}

pub fn handle_user_function<'arena>(
    function_call: &'arena FunctionCallExpr,
    function_idx: usize,
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    tgt_id: Option<u16>,
) -> Option<u16> {
    let args = function_call.args;
    let fn_name = function_call.qualified_name.get_name();
    let span = function_call.get_call_span();
    let arg_spans = &function_call.get_arg_spans();

    let fn_returns_null = state.functions[function_idx].returns_null;

    // Check if the arguments are correct
    let args_len = state.functions[function_idx].args.len();
    check_args_user_fn(
        args,
        args_len,
        fn_name,
        ctx.file_idx,
        span,
        (state.functions[function_idx].name_span, state.functions[function_idx].src_file_idx),
        state,
        arg_spans,
    );

    //This inlines dylib wrappers
    // Actual general function inlining is coming soon
    if state.functions[function_idx].code.len() == 1
        && let Expr::ReturnVal(ret) = &state.functions[function_idx].code[0]
        && let Some(Expr::FunctionCall(FunctionCallExpr {
            qualified_name, args: call_args, ..
        })) = ret.as_ref()
        && !qualified_name.is_namespace_empty()
        && call_args.len() == args_len
        && call_args
            .iter()
            .zip(state.functions[function_idx].args.iter())
            .all(|(e, (p, _))| matches!(e, Expr::Var(n, _) if n.get_name() == *p))
        && let Some(fn_sig) = state
            .dylibs
            .iter()
            .find(|lib| &lib.name == qualified_name.get_namespace().last().unwrap())
            .and_then(|lib| lib.fns.iter().find(|f| f.name == qualified_name.get_name()))
    {
        let dyn_id = fn_sig.id;
        let returns_null = fn_sig.return_type == DataType::Null;
        let expected_arg_types = fn_sig.args.clone();
        for (i, arg) in args.iter().enumerate() {
            let inferred = arg.infer_type(ctx, state);
            if !c_arg_matches(&inferred, &expected_arg_types[i]) {
                error_function_arg_invalid_type(
                    &inferred,
                    &expected_arg_types[i],
                    arg_spans[i],
                    fn_name,
                    Some((
                        state.functions[function_idx].name_span,
                        state.functions[function_idx].src_file_idx,
                    )),
                    ctx.file_idx,
                    state.sources,
                )
            }
        }

        let first_arg = compile_reg_window(args.iter(), ctx, state, output);
        let dest_reg_id =
            if returns_null { state.alloc_reg() } else { state.alloc_reg_tgt(tgt_id) };
        output.push(Instr::CallDynamicLibFunc { fn_id: dyn_id, dst: dest_reg_id, first_arg });
        state.add_to_src(ctx, output, span);
        state.free_reg_window(first_arg, args.len() as u16);
        return Some(dest_reg_id);
    }

    // Infer arg types
    let inferred_arg_types =
        args.iter().map(|arg| arg.infer_type(ctx, state)).collect::<Vec<DataType>>();

    check_user_fn_arg_types(function_idx, fn_name, &inferred_arg_types, arg_spans, ctx, state);

    let fn_impl_idx = compile_function_impl(state, function_idx, &inferred_arg_types);
    let unit = state.functions[function_idx].impls[fn_impl_idx].unit;
    let frame_offset = state.call_frame_offset();
    state.reserve_call_window(frame_offset, args.len() as u16 + 1);
    for (i, arg_expr) in args.iter().enumerate() {
        let tgt_id = frame_offset + 1 + i as u16;
        if let DataType::Fn(arg_fn_id) = inferred_arg_types[i] {
            let unit = state.functions[arg_fn_id as usize].impls.first().map_or(0, |imp| imp.unit);
            let fn_reg_id = state.value_reg(Data::function(unit));
            output.push(Instr::Mov(fn_reg_id, tgt_id));
            continue;
        }
        let start_len = output.len();
        let arg_id = arg_expr.compile(ctx, state, output, Some(tgt_id), false, true).unwrap_id();
        move_value_to(output, start_len, arg_id, tgt_id);
        if arg_id != tgt_id {
            state.free_reg(arg_id);
        }
    }
    output.push(Instr::CallFunc { target: unit, frame_offset });
    // Makes overflow report at the failing call
    state.add_to_src(ctx, output, span);
    for i in 0..args.len() as u16 {
        state.free_reg(frame_offset + 1 + i);
    }

    if fn_returns_null {
        state.free_reg(frame_offset);
        None
    } else {
        Some(frame_offset)
    }
}

pub fn compile_function<'arena>(
    state: &mut State<'arena, '_>,
    function_id: usize,
    fn_args: &[&'arena str],
    inferred_arg_types: &[DataType],
    fn_code: &'arena [Expr],
    fn_file_idx: u16,
) {
    // Local vector vars and recorded_types to allow the inner body to type-check correctly
    let caller_registers = core::mem::take(&mut state.regs);
    let return_reg = state.scratch_reg();
    state.reserve_register(return_reg);
    let mut v_temp: Vec<Variable> = fn_args
        .iter()
        .enumerate()
        .map(|(i, x)| {
            // Allocate a registers slot for each func arg
            Variable {
                name: x,
                register_id: state.scratch_reg(),
                declared_type: inferred_arg_types[i].clone(),
                var_type: inferred_arg_types[i].clone(),
            }
        })
        .collect();
    let fixed_count = 1 + fn_args.len() as u16;

    let unit = state.code_units.len() as u32;
    state.code_units.push(CodeUnit::default());

    // Add this func specialization to the func's metadata
    state.functions[function_id]
        .impls
        .push(FunctionImpl { unit, arg_types: Box::from(inferred_arg_types) });

    core::mem::swap(state.v, &mut v_temp);
    let hidden_symbols = state.enter_function_scope(fn_file_idx, function_id);

    // Compile the function into instructions using local vars
    let parsed = compile_expr(
        fn_code,
        Ctx {
            in_function: true,
            file_idx: fn_file_idx,
            single_run: false,
            offset: 0,
            try_depth: 0,
            loop_try_depth: 0,
            current_unit: Some(unit),
        },
        state,
    );
    core::mem::swap(state.v, &mut v_temp);
    state.exit_function_scope(fn_file_idx, hidden_symbols);

    let CodeUnitRegisters { temp_count, permanent_values, .. } =
        core::mem::replace(&mut state.regs, caller_registers);
    let permanent_count = permanent_values.len() as u16;
    let frame_size = temp_count + permanent_count;
    state.max_frame_regs = state
        .max_frame_regs
        .max(u32::from(frame_size))
        .max(u32::from(fixed_count + permanent_count + 7));

    let mut instructions =
        Vec::with_capacity(permanent_values.len().div_ceil(u8::MAX as usize) + parsed.len() + 1);

    for (chunk_idx, chunk) in permanent_values.chunks(u8::MAX as usize).enumerate() {
        let src = state.function_constants.len();
        instructions.push(Instr::CopyConstants {
            count_src: (chunk.len() as u32) << 24 | src as u32,
            dst: fixed_count + (chunk_idx * u8::MAX as usize) as u16,
        });
        state.function_constants.extend_from_slice(chunk);
        state.function_constants.resize(src + chunk.len().max(8), NULL);
    }
    let len = instructions.len() as u32;
    instructions.extend(parsed);
    instructions.push(Instr::Return(0));
    relocate_registers(&mut instructions[len as usize..], fixed_count, permanent_count);

    let code_unit = &mut state.code_units[unit as usize];
    for src in &mut code_unit.instr_src {
        src.instr_idx += len;
    }
    code_unit.instructions = instructions;
}
