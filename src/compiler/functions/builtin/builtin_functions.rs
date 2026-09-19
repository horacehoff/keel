use super::super::type_system::DataType;
use super::check_arg_type;
use super::check_user_fn_arg_types;
use super::user_functions::handle_user_function;
use crate::compiler::UnwrapId;
use crate::compiler::compiler_data::Ctx;
use crate::compiler::compiler_data::State;
use crate::compiler::compiler_errors::check_args_length;
use crate::compiler::compiler_errors::check_args_range;
use crate::compiler::compiler_errors::error_expected_function;
use crate::compiler::compiler_errors::error_unknown_function;
use crate::compiler::expr::FunctionCallExpr;
use crate::compiler::functions::user_functions::compile_function_impl;
use crate::compiler::registers::move_value_to;
use crate::data::Data;
use crate::instr::Instr;
use crate::instr::LibFunc;

pub fn builtin_functions<'arena>(
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    tgt_id: Option<u16>,
    function_call: &'arena FunctionCallExpr,
) -> Option<u16> {
    let args = function_call.args;
    let span = function_call.get_call_span();
    let arg_spans = function_call.get_arg_spans();
    let name = function_call.qualified_name.get_name();
    match name {
        "print" => {
            for arg in args {
                let id = arg.compile(ctx, state, output, None, false, true).unwrap_id();
                output.push(Instr::Print(id));
                state.free_reg(id);
            }
            None
        }
        "type" => {
            check_args_length(args, 1, name, span, state.sources, ctx.file_idx);
            let infered = args[0].infer_type(ctx, state);
            let arg_type =
                Data::comp_str(infered.format_detailed(state).as_str(), &mut state.pools.str_pool);
            Some(state.new_reg(arg_type))
        }
        "float" => {
            check_args_length(args, 1, name, span, state.sources, ctx.file_idx);
            check_arg_type(
                name,
                ctx,
                state,
                args,
                arg_spans,
                0,
                &[DataType::String, DataType::Int],
            );
            let id = args[0].compile(ctx, state, output, None, false, true).unwrap_id();
            state.free_reg(id);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Float, id, output_id));
            state.add_to_src(ctx, output, span);
            Some(output_id)
        }
        "int" => {
            check_args_length(args, 1, name, span, state.sources, ctx.file_idx);
            check_arg_type(
                name,
                ctx,
                state,
                args,
                arg_spans,
                0,
                &[DataType::String, DataType::Float],
            );
            let id = args[0].compile(ctx, state, output, None, false, true).unwrap_id();
            state.free_reg(id);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Int, id, output_id));
            state.add_to_src(ctx, output, span);
            Some(output_id)
        }
        "string" => {
            check_args_length(args, 1, name, span, state.sources, ctx.file_idx);
            let id = args[0].compile(ctx, state, output, None, false, true).unwrap_id();
            if args[0].infer_type(ctx, state) == DataType::String {
                return Some(id);
            }
            state.free_reg(id);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Str, id, output_id));
            Some(output_id)
        }
        "bool" => {
            check_args_length(args, 1, name, span, state.sources, ctx.file_idx);
            check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::String]);
            let id = args[0].compile(ctx, state, output, None, false, true).unwrap_id();
            state.free_reg(id);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Bool, id, output_id));
            state.add_to_src(ctx, output, span);
            Some(output_id)
        }
        "input" => {
            check_args_range(args, 0, 1, name, arg_spans, ctx.file_idx, state.sources, span);
            let id = if args.is_empty() {
                let data = Data::comp_str("", &mut state.pools.str_pool);
                state.new_reg(data)
            } else {
                check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::String]);
                args[0].compile(ctx, state, output, None, false, true).unwrap_id()
            };
            state.free_reg(id);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Input, id, output_id));
            Some(output_id)
        }
        "range" => {
            check_args_range(args, 1, 2, name, arg_spans, ctx.file_idx, state.sources, span);
            check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::Int]);
            if args.len() != 1 {
                check_arg_type(name, ctx, state, args, arg_spans, 1, &[DataType::Int]);
            }

            let id_first_arg = args[0].compile(ctx, state, output, None, false, true).unwrap_id();
            let source_reg_id = if args.len() == 1 {
                id_first_arg
            } else {
                let id_second_arg =
                    args[1].compile(ctx, state, output, None, false, true).unwrap_id();
                output.push(Instr::StoreFuncArg(id_first_arg));
                state.add_arg_hint(1);
                state.sub_arg_hint(1);
                id_second_arg
            };
            state.free_reg(id_first_arg);
            state.free_reg(source_reg_id);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Range, source_reg_id, output_id));
            Some(output_id)
        }
        "the_answer" => {
            check_args_length(args, 0, name, span, state.sources, ctx.file_idx);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TheAnswer, 0, output_id));
            Some(output_id)
        }
        "argv" => {
            check_args_length(args, 0, name, span, state.sources, ctx.file_idx);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Argv, 0, output_id));
            Some(output_id)
        }
        "exit" => {
            check_args_range(args, 0, 1, name, arg_spans, ctx.file_idx, state.sources, span);
            let halt_code = if args.is_empty() {
                0
            } else {
                check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::Int]);
                args[0].compile(ctx, state, output, None, false, true).unwrap_id()
            };
            output.push(Instr::Halt(halt_code));
            None
        }
        "throw" => {
            check_args_length(args, 1, name, span, state.sources, ctx.file_idx);
            check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::String]);
            let err_reg_id = args[0].compile(ctx, state, output, None, false, true).unwrap_id();
            output.push(Instr::ThrowError(err_reg_id));
            state.add_to_src(ctx, output, span);
            None
        }
        fn_name => {
            if let Some(var) = state.find_var(fn_name) {
                let fn_reg = var.register_id;
                let fn_id = if let DataType::Fn(id) = var.var_type {
                    id as usize
                } else {
                    error_expected_function(&var.var_type, span, ctx.file_idx, state.sources)
                };

                let inferred_arg_types =
                    args.iter().map(|arg| arg.infer_type(ctx, state)).collect::<Vec<DataType>>();

                check_user_fn_arg_types(fn_id, fn_name, &inferred_arg_types, arg_spans, ctx, state);

                let fn_impl_idx =
                    compile_function_impl(output, ctx, state, fn_id, &inferred_arg_types);

                let loc = state.functions[fn_id].impls[fn_impl_idx].loc;
                state.registers[fn_reg as usize] = Data::function(loc);

                for (i, arg_expr) in args.iter().enumerate() {
                    let tgt_id = state.functions[fn_id].impls[fn_impl_idx].args_loc[i];
                    let start_len = output.len();
                    let arg_id =
                        arg_expr.compile(ctx, state, output, Some(tgt_id), false, true).unwrap_id();
                    move_value_to(output, start_len, arg_id, tgt_id);
                }

                let return_register_id = state.alloc_reg_tgt(tgt_id);
                output.push(Instr::CallFuncDynamic(fn_reg, return_register_id));
                Some(return_register_id)
            } else if let Some(fn_id) = state.scope(ctx.file_idx).find_function(
                &[],
                fn_name,
                span,
                ctx.file_idx,
                state.sources,
            ) {
                handle_user_function(function_call, fn_id, output, ctx, state, tgt_id)
            } else {
                error_unknown_function(
                    fn_name,
                    span,
                    state.scope(ctx.file_idx),
                    ctx.file_idx,
                    state.sources,
                );
            }
        }
    }
}
