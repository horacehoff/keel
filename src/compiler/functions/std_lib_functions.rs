use crate::compiler::get_id;
use crate::compiler_data::Ctx;
use crate::compiler_data::State;
use crate::compiler_data::Variable;
use crate::data::Data;
use crate::errors::ErrType;
use crate::errors::throw_compiler_error;
use crate::expr::Expr;
use crate::expr::Span;
use crate::functions::check_arg_type;
use crate::instr::Instr;
use crate::instr::LibFunc;
use crate::type_system::DataType;
use crate::type_system::infer_type;
use crate::user_functions::handle_user_function;
use crate::util::check_args;
use crate::util::check_args_range;

pub fn std_lib_functions(
    name: &str,
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    args: &[Expr],
    markers: Span,
    args_indexes: &[Span],
    offset: u16,
    single_run: bool,
) -> Option<u16> {
    let src = ctx.src;
    let current_src_file = ctx.current_src_file;
    match name {
        "print" => {
            for arg in args {
                let id = get_id(arg, v, ctx, state, output, None, false, offset, single_run);
                output.push(Instr::Print(id));
                state.free_reg(id, v);
            }
        }
        "type" => {
            check_args(args, 1, name, src, markers);
            let infered = infer_type(&args[0], v, ctx, state);
            state.registers.push(Data::p_str(
                infered.format_detailed(state).as_str(),
                &mut state.pools.strings,
            ));
            return Some((state.registers.len() - 1) as u16);
        }
        "float" => {
            check_args(args, 1, name, src, markers);
            check_arg_type(
                v,
                ctx,
                state,
                args,
                args_indexes,
                0,
                &[DataType::String, DataType::Int],
            );
            let id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            state.free_reg(id, v);
            output.push(Instr::CallLibFunc(LibFunc::Float, id, state.alloc_reg()));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
        }
        "int" => {
            check_args(args, 1, name, src, markers);
            check_arg_type(
                v,
                ctx,
                state,
                args,
                args_indexes,
                0,
                &[DataType::String, DataType::Float],
            );
            let id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            state.free_reg(id, v);
            output.push(Instr::CallLibFunc(LibFunc::Int, id, state.alloc_reg()));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
        }
        "str" => {
            check_args(args, 1, name, src, markers);
            let id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            state.free_reg(id, v);
            output.push(Instr::CallLibFunc(LibFunc::Str, id, state.alloc_reg()));
        }
        "bool" => {
            check_args(args, 1, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            state.free_reg(id, v);
            output.push(Instr::CallLibFunc(LibFunc::Bool, id, state.alloc_reg()));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
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
                get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                )
            };
            state.free_reg(id, v);
            output.push(Instr::CallLibFunc(LibFunc::Input, id, state.alloc_reg()));
        }
        "range" => {
            check_args_range(args, 1, 2, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::Int]);
            if args.len() != 1 {
                check_arg_type(v, ctx, state, args, args_indexes, 1, &[DataType::Int]);
            }

            let id_first_arg = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            let source_reg_id = if args.len() == 1 {
                id_first_arg
            } else {
                let id_second_arg = get_id(
                    &args[1], v, ctx, state, output, None, false, offset, single_run,
                );
                output.push(Instr::StoreFuncArg(id_first_arg));
                *state.allocated_arg_count += 1;
                id_second_arg
            };
            state.free_reg(id_first_arg, v);
            state.free_reg(source_reg_id, v);
            output.push(Instr::CallLibFunc(
                LibFunc::Range,
                source_reg_id,
                state.alloc_reg(),
            ));
        }
        "the_answer" => {
            check_args(args, 0, name, src, markers);
            output.push(Instr::CallLibFunc(LibFunc::TheAnswer, 0, state.alloc_reg()));
        }
        "argv" => {
            check_args(args, 0, name, src, markers);
            output.push(Instr::CallLibFunc(LibFunc::Argv, 0, state.alloc_reg()));
        }
        "exit" => {
            check_args_range(args, 0, 1, name, src, markers);
            let halt_code = if args.is_empty() {
                0
            } else {
                check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::Int]);
                get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                )
            };
            output.push(Instr::Halt(halt_code));
        }
        "throw" => {
            check_args(args, 1, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let err_reg_id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            output.push(Instr::ThrowError(err_reg_id));
        }
        fn_name => {
            if let Some((_, fn_id)) = state
                .namespace
                .fns
                .iter_mut()
                .find(|(func_name, _)| func_name == fn_name)
            {
                return Some(handle_user_function(
                    fn_name,
                    *fn_id as usize,
                    output,
                    v,
                    ctx,
                    state,
                    args,
                    markers,
                    args_indexes,
                    offset,
                    single_run,
                ));
            }
            throw_compiler_error(src, markers, ErrType::UnknownFunction(fn_name));
        }
    }
    None
}
