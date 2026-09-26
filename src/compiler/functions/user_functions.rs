use super::super::expr::Expr;
use super::super::type_system::DataType;
use super::super::type_system::c_arg_matches;
use super::super::type_system::can_reach;
use super::check_user_fn_arg_types;
use crate::compiler::UnwrapId;
use crate::compiler::compile_expr;
use crate::compiler::compiler_data::Ctx;
use crate::compiler::compiler_data::FunctionImpl;
use crate::compiler::compiler_data::State;
use crate::compiler::compiler_data::Variable;
use crate::compiler::compiler_errors::check_args_user_fn;
use crate::compiler::compiler_errors::error_function_arg_invalid_type;
use crate::compiler::expr::FunctionCallExpr;
use crate::compiler::registers::get_tgt_ids;
use crate::compiler::registers::move_value_to;
use crate::compiler::type_system::collect_direct_fn_calls;
use crate::data::Data;
use crate::data::NULL;
use crate::instr::Instr;
use rustc_hash::FxHashSet;

/// Computes whether the function `state.fns[fn_id]` is recursive.
/// This is only computed once per function.
pub fn is_function_recursive(fn_id: usize, state: &mut State<'_, '_>) -> bool {
    if let Some(is_recursive) = state.functions[fn_id].is_recursive {
        is_recursive
    } else {
        let mut visited = FxHashSet::default();
        visited.insert(fn_id);
        let is_recursive =
            can_reach(fn_id, fn_id, state.functions, &mut visited, state.file_scopes);
        state.functions[fn_id].is_recursive = Some(is_recursive);
        is_recursive
    }
}

/// Returns the index into `state.fns[fn_id].impls` for this specialization (those arg types)
/// matching `inferred_arg_types`. If it doesn't exist yet, it's compiled via `compile_function`.
pub fn compile_function_impl(
    output: &mut Vec<Instr>,
    ctx: Ctx,
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
    let is_recursive = is_function_recursive(fn_id, state);
    let fn_args = state.functions[fn_id].args.iter().map(|(a, _)| *a).collect::<Vec<&str>>();
    compile_function(
        output,
        ctx,
        state,
        fn_id,
        &fn_args,
        inferred_arg_types,
        state.functions[fn_id].code,
        fn_id as u16,
        is_recursive,
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
    let is_recursive = is_function_recursive(function_idx, state);

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

        state.add_arg_hint(args.len());
        for arg in args.iter().take(args.len().saturating_sub(1)) {
            let arg_id = arg.compile(ctx, state, output, None, false, true).unwrap_id();
            output.push(Instr::StoreFuncArg(arg_id));
            state.free_reg(arg_id);
        }
        let last_arg_reg_id = if let Some(arg) = args.last() {
            let arg_id = arg.compile(ctx, state, output, None, false, true).unwrap_id();
            state.free_reg(arg_id);
            arg_id
        } else {
            u16::MAX
        };

        let dest_reg_id = if returns_null { 0 } else { state.alloc_reg_tgt(tgt_id) };
        output.push(Instr::CallDynamicLibFunc { fn_id: dyn_id, dest_reg_id, last_arg_reg_id });
        state.add_to_src(ctx, output, span);
        state.sub_arg_hint(args.len());
        return Some(dest_reg_id);
    }

    // Infer arg types
    let inferred_arg_types =
        args.iter().map(|arg| arg.infer_type(ctx, state)).collect::<Vec<DataType>>();

    check_user_fn_arg_types(function_idx, fn_name, &inferred_arg_types, arg_spans, ctx, state);

    let fn_impl_idx = compile_function_impl(output, ctx, state, function_idx, &inferred_arg_types);
    let loc = state.functions[function_idx].impls[fn_impl_idx].loc;

    let saveframe_loc = output.len();
    let callsite_id = if is_recursive {
        let id = state.fn_registers.len() as u16;
        state.fn_registers.push(Vec::new());
        output.push(Instr::SaveFrame(0, 0, 0));
        *state.allocated_call_depth += 2;
        Some(id)
    } else {
        None
    };
    // Move evaluated call args into the expected arg slots
    // This is a VERY bad temporary fix
    // This is tthe index of the last argument that can run user code, only earlier arguments are at risk of being overwritten
    let last_harmless_arg = args
        .iter()
        .rposition(|arg| {
            let mut calls = Vec::new();
            collect_direct_fn_calls(std::slice::from_ref(arg), &mut calls);
            calls.iter().any(|call| {
                let fn_name = call.get_name();
                fn_name == "map"
                    || fn_name == "filter"
                    || state
                        .scope(ctx.file_idx)
                        .find_function_fallible(call.get_namespace(), fn_name)
                        .is_some()
                    || (call.is_namespace_empty() && state.find_var(fn_name).is_some())
            })
        })
        .unwrap_or(0);
    let mut deferred_args: Vec<(u16, u16)> = Vec::new();
    for (i, arg_expr) in args.iter().enumerate() {
        let tgt_id = state.functions[function_idx].impls[fn_impl_idx].args_loc[i];

        if let DataType::Fn(arg_fn_id) = inferred_arg_types[i] {
            let loc = state.functions[arg_fn_id as usize].impls.first().map_or(0, |imp| imp.loc);
            let fn_reg_id = state.new_reg(Data::function(loc));
            if fn_reg_id != tgt_id {
                output.push(Instr::Mov(fn_reg_id, tgt_id));
            }
            continue;
        }
        if i < last_harmless_arg {
            let arg_id = arg_expr.compile(ctx, state, output, None, false, true).unwrap_id();
            deferred_args.push((arg_id, tgt_id));
            continue;
        }

        let start_len = output.len();
        let arg_id = arg_expr.compile(ctx, state, output, Some(tgt_id), false, true).unwrap_id();
        move_value_to(output, start_len, arg_id, tgt_id);
    }
    for (arg_id, tgt_id) in deferred_args {
        if arg_id != tgt_id {
            output.push(Instr::Mov(arg_id, tgt_id));
        }
    }
    if !is_recursive {
        state.fn_registers.get_mut(function_idx).unwrap().extend(
            get_tgt_ids(&output[saveframe_loc..], state.registers.len()).ones().map(|id| id as u16),
        );
    }

    let return_register_id = if fn_returns_null { 0 } else { state.alloc_reg_tgt(tgt_id) };
    if is_recursive {
        output.push(Instr::CallFuncRecursive(loc, return_register_id));
    } else {
        output.push(Instr::CallFunc(loc, return_register_id));
        *state.allocated_call_depth += 2;
    }

    if is_recursive {
        output[saveframe_loc] = Instr::SaveFrame(
            (output.len() - 1 - saveframe_loc) as u16,
            return_register_id,
            callsite_id.unwrap(),
        );
    }

    if fn_returns_null { None } else { Some(return_register_id) }
}

pub fn compile_function<'arena>(
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    function_id: usize,
    fn_args: &[&'arena str],
    inferred_arg_types: &[DataType],
    fn_code: &'arena [Expr],
    fn_id: u16,
    is_recursive: bool,
    fn_file_idx: u16,
) {
    // Local vector vars and recorded_types to allow the inner body to type-check correctly
    let mut v_temp: Vec<Variable> = fn_args
        .iter()
        .enumerate()
        .map(|(i, x)| {
            // Allocate a registers slot for each func arg
            Variable {
                name: x,
                register_id: state.new_reg(NULL),
                declared_type: inferred_arg_types[i].clone(),
                var_type: inferred_arg_types[i].clone(),
            }
        })
        .collect();

    // Get the arg destination ids
    let mut args_loc = bumpalo::collections::Vec::with_capacity_in(v_temp.len(), state.bump);
    for x in &v_temp {
        args_loc.push(x.register_id);
    }

    // Temporarily jump over function to prevent executing it right now
    // This is a placeholder that's modified later on
    output.push(Instr::Jmp(0));
    let jump_idx = output.len() - 1;

    // Record start location for the compiled func body
    let fn_start = output.len();
    let loc = fn_start as u16 + ctx.offset;
    let args_loc = args_loc.into_bump_slice();
    // Add this func specialization to the func's metadata
    state.functions[function_id].impls.push(FunctionImpl {
        loc,
        args_loc,
        arg_types: Box::from(inferred_arg_types),
    });

    std::mem::swap(state.v, &mut v_temp);
    let hidden_symbols = state.enter_function_scope(fn_file_idx, function_id);

    // Compile the function into instructions using local vars
    let parsed = compile_expr(
        fn_code,
        Ctx {
            is_compiling_recursive: is_recursive,
            file_idx: fn_file_idx,
            single_run: false,
            offset: ctx.offset + output.len() as u16,
            ..ctx
        },
        state,
    );
    std::mem::swap(state.v, &mut v_temp);
    state.exit_function_scope(fn_file_idx, hidden_symbols);

    let all_written_regs = get_tgt_ids(&parsed, state.registers.len());

    state.reserved_registers.extend(all_written_regs.ones().map(|id| id as u16));
    state.reserved_registers.extend(args_loc);
    for instr in &parsed {
        match instr {
            Instr::CloneArray(template_reg, _, _)
            | Instr::CloneStruct(template_reg, _)
            | Instr::CloneMap(template_reg, _) => {
                state.reserved_registers.insert(*template_reg);
            }
            _ => {}
        }
    }
    state.unfree_reserved_registers();

    if is_recursive {
        // For each recursive call, only save registers that are read between that call's return and the end of the function
        for (save_frame_instr_pos, instr) in parsed.iter().enumerate() {
            if let Instr::SaveFrame(call_loc_relative, _, callsite_id) = *instr {
                let pos = save_frame_instr_pos + call_loc_relative as usize;
                let mut live_regs: Vec<u16> = Vec::new();
                for after_instr in &parsed[pos + 1..] {
                    if let Instr::CallFuncRecursive(_, _) = after_instr {
                        for reg in args_loc {
                            if unsafe { all_written_regs.contains_unchecked(*reg as usize) } {
                                live_regs.push(*reg);
                            }
                        }
                    } else {
                        after_instr.for_each_read_reg(|reg| {
                            if unsafe { all_written_regs.contains_unchecked(reg as usize) } {
                                live_regs.push(reg);
                            }
                        });
                    }
                }
                live_regs.sort_unstable();
                live_regs.dedup();
                unsafe {
                    *state.fn_registers.get_unchecked_mut(callsite_id as usize) = live_regs;
                }
            }
        }
    } else {
        state
            .fn_registers
            .get_mut(fn_id as usize)
            .unwrap()
            .extend(all_written_regs.ones().map(|id| id as u16));
    }

    output.extend(parsed);

    output.push(Instr::VoidReturn);

    // Fix the placeholder Jmp(0) to skip over the function body
    *output.get_mut(jump_idx).unwrap() = Instr::Jmp((output.len() - fn_start + 1) as u16);
}
