use super::super::expr::Expr;
use super::super::expr::Span;
use super::super::type_system::DataType;
use super::check_arg_type;
use super::user_functions::handle_user_function;
use crate::compiler::UnwrapId;
use crate::compiler::compiler_data::Ctx;
use crate::compiler::compiler_data::State;
use crate::compiler::compiler_data::Variable;
use crate::data::Data;
use crate::errors::ErrType;
use crate::errors::throw_compiler_error;
use crate::instr::Instr;
use crate::instr::LibFunc;
use crate::util::check_args;
use crate::util::check_args_range;

pub fn builtin_functions(
    name: &str,
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    tgt_id: Option<u16>,
    args: &[Expr],
    markers: Span,
    args_indexes: &[Span],
) -> Option<u16> {
    let src = ctx.src;
    let current_src_file = ctx.current_src_file;
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
            check_args(args, 1, name, src, markers, state.sources);
            let infered = args[0].infer_type(v, ctx, state);
            state.registers.push(Data::p_str(
                infered.format_detailed(state).as_str(),
                &mut state.pools.strings,
            ));
            Some((state.registers.len() - 1) as u16)
        }
        "float" => {
            check_args(args, 1, name, src, markers, state.sources);
            check_arg_type(
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
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
            Some(output_id)
        }
        "int" => {
            check_args(args, 1, name, src, markers, state.sources);
            check_arg_type(
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
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
            Some(output_id)
        }
        "str" => {
            check_args(args, 1, name, src, markers, state.sources);
            let id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(id, v);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Str, id, output_id));
            Some(output_id)
        }
        "bool" => {
            check_args(args, 1, name, src, markers, state.sources);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(id, v);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Bool, id, output_id));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
            Some(output_id)
        }
        "input" => {
            check_args_range(args, 0, 1, name, src, markers);
            let id = if args.is_empty() {
                state
                    .registers
                    .push(Data::p_str("", &mut state.pools.strings));
                (state.registers.len() - 1) as u16
            } else {
                check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
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
            check_args_range(args, 1, 2, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::Int]);
            if args.len() != 1 {
                check_arg_type(v, ctx, state, args, args_indexes, 1, &[DataType::Int]);
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
            check_args(args, 0, name, src, markers, state.sources);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TheAnswer, 0, output_id));
            Some(output_id)
        }
        "argv" => {
            check_args(args, 0, name, src, markers, state.sources);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Argv, 0, output_id));
            Some(output_id)
        }
        "exit" => {
            check_args_range(args, 0, 1, name, src, markers);
            let halt_code = if args.is_empty() {
                0
            } else {
                check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::Int]);
                args[0]
                    .compile(v, ctx, state, output, None, false, true)
                    .unwrap_id()
            };
            output.push(Instr::Halt(halt_code));
            None
        }
        "throw" => {
            check_args(args, 1, name, src, markers, state.sources);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let err_reg_id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            output.push(Instr::ThrowError(err_reg_id));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
            None
        }
        fn_name => {
            if let Some((_, fn_id)) = state
                .namespace
                .fns
                .iter_mut()
                .find(|(func_name, _)| func_name == fn_name && func_name != "main")
            {
                handle_user_function(
                    fn_name,
                    *fn_id as usize,
                    output,
                    v,
                    ctx,
                    state,
                    tgt_id,
                    args,
                    markers,
                    args_indexes,
                )
            } else {
                throw_compiler_error(src, markers, ErrType::UnknownFunction(fn_name));
            }
        }
    }
}
