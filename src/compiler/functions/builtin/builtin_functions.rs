use super::super::type_system::DataType;
use super::check_arg_type;
use super::user_functions::handle_user_function;
use crate::ast::FunctionCallExpr;
use crate::compiler::Ctx;
use crate::compiler::State;
use crate::compiler::UnwrapId;
use crate::compiler::compiler_errors::check_args_length;
use crate::compiler::compiler_errors::check_args_range;
use crate::compiler::compiler_errors::error_expected_function;
use crate::compiler::compiler_errors::error_unknown_function;
use crate::data::Data;
use crate::instr::Instr;

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
            Some(state.value_reg(arg_type))
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
            output.push(Instr::Float(id, output_id));
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
            output.push(Instr::Int(id, output_id));
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
            output.push(Instr::Str(id, output_id));
            Some(output_id)
        }
        "bool" => {
            check_args_length(args, 1, name, span, state.sources, ctx.file_idx);
            check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::String]);
            let id = args[0].compile(ctx, state, output, None, false, true).unwrap_id();
            state.free_reg(id);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::Bool(id, output_id));
            state.add_to_src(ctx, output, span);
            Some(output_id)
        }
        "input" => {
            check_args_range(args, 0, 1, name, arg_spans, ctx.file_idx, state.sources, span);
            let id = if args.is_empty() {
                let data = Data::comp_str("", &mut state.pools.str_pool);
                state.value_reg(data)
            } else {
                check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::String]);
                let id = args[0].compile(ctx, state, output, None, false, true).unwrap_id();
                state.free_reg(id);
                id
            };
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::Input(id, output_id));
            Some(output_id)
        }
        "range" => {
            check_args_range(args, 1, 2, name, arg_spans, ctx.file_idx, state.sources, span);
            check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::Int]);
            if args.len() != 1 {
                check_arg_type(name, ctx, state, args, arg_spans, 1, &[DataType::Int]);
            }

            let (min_reg, max_expr) = if args.len() == 1 {
                (state.new_const_reg(Data::int(0)), &args[0])
            } else {
                (args[0].compile(ctx, state, output, None, false, true).unwrap(), &args[1])
            };
            let max_reg = max_expr.compile(ctx, state, output, None, false, true).unwrap_id();
            state.free_reg(min_reg);
            state.free_reg(max_reg);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::Range(min_reg, max_reg, output_id));
            Some(output_id)
        }
        "the_answer" => {
            check_args_length(args, 0, name, span, state.sources, ctx.file_idx);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::TheAnswer(output_id));
            Some(output_id)
        }
        "argv" => {
            check_args_length(args, 0, name, span, state.sources, ctx.file_idx);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::Argv(output_id));
            Some(output_id)
        }
        "exit" => {
            check_args_range(args, 0, 1, name, arg_spans, ctx.file_idx, state.sources, span);
            let halt_code = if args.is_empty() {
                u16::MAX
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
                let DataType::Fn(fn_id) = var.var_type else {
                    error_expected_function(&var.var_type, span, ctx.file_idx, state.sources)
                };
                handle_user_function(function_call, fn_id as usize, output, ctx, state, tgt_id)
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
