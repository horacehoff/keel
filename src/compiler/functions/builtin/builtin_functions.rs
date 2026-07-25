use super::super::expr::Expr;
use super::super::expr::Span;
use super::super::registers::move_to_id;
use super::super::type_system::DataType;
use super::check_arg_type;
use super::user_functions::compile_function;
use super::user_functions::handle_user_function;
use crate::compiler::UnwrapId;
use crate::compiler::compiler_data::Ctx;
use crate::compiler::compiler_data::State;
use crate::compiler::compiler_data::Variable;
use crate::compiler::compiler_errors::check_args;
use crate::compiler::compiler_errors::check_args_range;
use crate::compiler::compiler_errors::error_expected_function;
use crate::compiler::compiler_errors::error_unknown_function;
use crate::data::Data;
use crate::instr::Instr;
use crate::instr::LibFunc;
use smol_strc::SmolStr;
use std::rc::Rc;

pub fn builtin_functions(
    name: &str,
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx,
    state: &mut State<'_>,
    tgt_id: Option<u16>,
    args: &[Expr],
    span: Span,
    args_indexes: &[Span],
) -> Option<u16> {
    match name {
        "print" => {
            for arg in args {
                let id = arg
                    .compile(v, ctx, state, output, None, false, true)
                    .unwrap_id();
                output.push(Instr::Print(id));
                state.free_reg(id, v);
            }
            None
        }
        "type" => {
            check_args(args, 1, name, span, state.sources, ctx.file_idx);
            let infered = args[0].infer_type(v, ctx, state);
            state.registers.push(Data::p_str(
                infered.format_detailed(state).as_str(),
                &mut state.pools.strings,
            ));
            Some((state.registers.len() - 1) as u16)
        }
        "float" => {
            check_args(args, 1, name, span, state.sources, ctx.file_idx);
            check_arg_type(
                name,
                v,
                ctx,
                state,
                args,
                args_indexes,
                0,
                &[DataType::String, DataType::Int],
            );
            let id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(id, v);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Float, id, output_id));
            state.add_to_src(ctx, output, span);
            Some(output_id)
        }
        "int" => {
            check_args(args, 1, name, span, state.sources, ctx.file_idx);
            check_arg_type(
                name,
                v,
                ctx,
                state,
                args,
                args_indexes,
                0,
                &[DataType::String, DataType::Float],
            );
            let id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(id, v);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Int, id, output_id));
            state.add_to_src(ctx, output, span);
            Some(output_id)
        }
        "str" => {
            check_args(args, 1, name, span, state.sources, ctx.file_idx);
            let id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(id, v);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Str, id, output_id));
            Some(output_id)
        }
        "bool" => {
            check_args(args, 1, name, span, state.sources, ctx.file_idx);
            check_arg_type(
                name,
                v,
                ctx,
                state,
                args,
                args_indexes,
                0,
                &[DataType::String],
            );
            let id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(id, v);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Bool, id, output_id));
            state.add_to_src(ctx, output, span);
            Some(output_id)
        }
        "input" => {
            check_args_range(
                args,
                0,
                1,
                name,
                args_indexes,
                ctx.file_idx,
                state.sources,
                span,
            );
            let id = if args.is_empty() {
                state
                    .registers
                    .push(Data::p_str("", &mut state.pools.strings));
                (state.registers.len() - 1) as u16
            } else {
                check_arg_type(
                    name,
                    v,
                    ctx,
                    state,
                    args,
                    args_indexes,
                    0,
                    &[DataType::String],
                );
                args[0]
                    .compile(v, ctx, state, output, None, false, true)
                    .unwrap_id()
            };
            state.free_reg(id, v);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Input, id, output_id));
            Some(output_id)
        }
        "range" => {
            check_args_range(
                args,
                1,
                2,
                name,
                args_indexes,
                ctx.file_idx,
                state.sources,
                span,
            );
            check_arg_type(name, v, ctx, state, args, args_indexes, 0, &[DataType::Int]);
            if args.len() != 1 {
                check_arg_type(name, v, ctx, state, args, args_indexes, 1, &[DataType::Int]);
            }

            let id_first_arg = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            let source_reg_id = if args.len() == 1 {
                id_first_arg
            } else {
                let id_second_arg = args[1]
                    .compile(v, ctx, state, output, None, false, true)
                    .unwrap_id();
                output.push(Instr::StoreFuncArg(id_first_arg));
                *state.allocated_arg_count += 1;
                id_second_arg
            };
            state.free_reg(id_first_arg, v);
            state.free_reg(source_reg_id, v);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Range, source_reg_id, output_id));
            Some(output_id)
        }
        "the_answer" => {
            check_args(args, 0, name, span, state.sources, ctx.file_idx);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TheAnswer, 0, output_id));
            Some(output_id)
        }
        "argv" => {
            check_args(args, 0, name, span, state.sources, ctx.file_idx);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Argv, 0, output_id));
            Some(output_id)
        }
        "exit" => {
            check_args_range(
                args,
                0,
                1,
                name,
                args_indexes,
                ctx.file_idx,
                state.sources,
                span,
            );
            let halt_code = if args.is_empty() {
                0
            } else {
                check_arg_type(name, v, ctx, state, args, args_indexes, 0, &[DataType::Int]);
                args[0]
                    .compile(v, ctx, state, output, None, false, true)
                    .unwrap_id()
            };
            output.push(Instr::Halt(halt_code));
            None
        }
        "throw" => {
            check_args(args, 1, name, span, state.sources, ctx.file_idx);
            check_arg_type(
                name,
                v,
                ctx,
                state,
                args,
                args_indexes,
                0,
                &[DataType::String],
            );
            let err_reg_id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            output.push(Instr::ThrowError(err_reg_id));
            state.add_to_src(ctx, output, span);
            None
        }
        fn_name => {
            if let Some(var) = v.iter().rfind(|var| var.name.as_str() == fn_name) {
                let fn_reg = var.register_id;
                let fn_id = if let DataType::Fn(id) = var.var_type {
                    id as usize
                } else {
                    error_expected_function(&var.var_type, span, ctx.file_idx, state.sources)
                };

                let inferred_arg_types = args
                    .iter()
                    .map(|arg| arg.infer_type(v, ctx, state))
                    .collect::<Vec<DataType>>();

                let fn_impl_idx = state.fns[fn_id]
                    .impls
                    .iter()
                    .position(|fn_impl| *fn_impl.arg_types == inferred_arg_types);

                if fn_impl_idx.is_none() {
                    // If it hasn't already been compiled for these argument types,
                    // compile it (which adds it to the function's implementation list)
                    let fn_args = state.fns[fn_id]
                        .args
                        .iter()
                        .map(|(a, _)| a.clone())
                        .collect::<Vec<SmolStr>>();
                    let fn_code: Rc<[Expr]> = Rc::clone(&state.fns[fn_id].code);
                    let closure_name = state.fns[fn_id].name.clone();
                    compile_function(
                        output,
                        v,
                        ctx,
                        state,
                        fn_id,
                        &fn_args,
                        &closure_name,
                        &inferred_arg_types,
                        args,
                        &fn_code,
                        fn_id as u16,
                        false,
                        state.fns[fn_id].src_file,
                    );
                }
                let fn_impl_idx = fn_impl_idx.unwrap_or_else(|| state.fns[fn_id].impls.len() - 1);

                let args_loc_len = state.fns[fn_id].impls[fn_impl_idx].args_loc.len();
                for i in 0..args_loc_len {
                    let tgt_id = state.fns[fn_id].impls[fn_impl_idx].args_loc[i];
                    let start_len = output.len();
                    let arg_id = args[i]
                        .compile(v, ctx, state, output, Some(tgt_id), false, true)
                        .unwrap_id();
                    if output.len() == start_len {
                        output.push(Instr::Mov(arg_id, tgt_id));
                    } else {
                        move_to_id(output, tgt_id);
                    }
                }

                let return_register_id = state.alloc_reg_tgt(tgt_id);
                output.push(Instr::CallFuncDynamic(fn_reg, return_register_id));
                Some(return_register_id)
            } else if let Some(fn_id) =
                state
                    .namespace
                    .find_function(&[], fn_name, span, ctx.file_idx, state.sources)
            {
                handle_user_function(
                    fn_name,
                    fn_id,
                    output,
                    v,
                    ctx,
                    state,
                    tgt_id,
                    args,
                    span,
                    args_indexes,
                )
            } else {
                error_unknown_function(fn_name, span, state.namespace, ctx.file_idx, state.sources);
            }
        }
    }
}
