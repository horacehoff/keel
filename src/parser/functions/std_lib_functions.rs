use crate::Data;
use crate::Instr;
use crate::LibFunc;
use crate::check_args;
use crate::check_args_range;
use crate::errors::ErrType;
use crate::errors::throw_parser_error;
use crate::expr::Expr;
use crate::expr::Span;
use crate::functions::check_arg_type;
use crate::get_id;
use crate::parser_data::Ctx;
use crate::parser_data::State;
use crate::parser_data::Variable;
use crate::registers::alloc_register;
use crate::registers::free_register;
use crate::type_system::DataType;
use crate::type_system::infer_type;
use crate::user_functions::handle_user_function;

pub fn std_lib_functions(
    name: &str,
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    args: &[Expr],
    markers: &Span,
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
                free_register(id, state.free_registers, v, state.const_registers);
            }
        }
        "type" => {
            check_args!(args, 1, name, src, markers);
            let infered = infer_type(&args[0], v, state.fns, src, state.dyn_libs);
            state.registers.push(Data::p_str(
                &infered.to_string(),
                &mut state.pools.string_pool,
            ));
            return Some((state.registers.len() - 1) as u16);
        }
        "float" => {
            check_args!(args, 1, name, src, markers);
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
            free_register(id, state.free_registers, v, state.const_registers);
            output.push(Instr::CallLibFunc(
                LibFunc::Float,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
            state
                .instr_src
                .push((*output.last().unwrap(), *markers, current_src_file));
        }
        "int" => {
            check_args!(args, 1, name, src, markers);
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
            free_register(id, state.free_registers, v, state.const_registers);
            output.push(Instr::CallLibFunc(
                LibFunc::Int,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
            state
                .instr_src
                .push((*output.last().unwrap(), *markers, current_src_file));
        }
        "str" => {
            check_args!(args, 1, name, src, markers);
            let id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            free_register(id, state.free_registers, v, state.const_registers);
            output.push(Instr::CallLibFunc(
                LibFunc::Str,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "bool" => {
            check_args!(args, 1, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            free_register(id, state.free_registers, v, state.const_registers);
            output.push(Instr::CallLibFunc(
                LibFunc::Bool,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
            state
                .instr_src
                .push((*output.last().unwrap(), *markers, current_src_file));
        }
        "input" => {
            check_args_range!(args, 0, 1, name, src, markers);
            let id = if args.is_empty() {
                state
                    .registers
                    .push(Data::p_str("", &mut state.pools.string_pool));
                (state.registers.len() - 1) as u16
            } else {
                check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
                get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                )
            };
            free_register(id, state.free_registers, v, state.const_registers);
            output.push(Instr::CallLibFunc(
                LibFunc::Input,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "range" => {
            check_args_range!(args, 1, 2, name, src, markers);
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
            free_register(id_first_arg, state.free_registers, v, state.const_registers);
            free_register(
                source_reg_id,
                state.free_registers,
                v,
                state.const_registers,
            );
            output.push(Instr::CallLibFunc(
                LibFunc::Range,
                source_reg_id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "the_answer" => {
            check_args!(args, 0, name, src, markers);
            output.push(Instr::CallLibFunc(
                LibFunc::TheAnswer,
                0,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "argv" => {
            check_args!(args, 0, name, src, markers);
            output.push(Instr::CallLibFunc(
                LibFunc::Argv,
                0,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "exit" => {
            check_args_range!(args, 0, 1, name, src, markers);
            let halt_code = if !args.is_empty() {
                check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::Int]);
                get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                )
            } else {
                0
            };
            output.push(Instr::Halt(halt_code));
        }
        fn_name => {
            return Some(handle_user_function(
                fn_name, output, v, ctx, state, args, markers, offset, single_run,
            ));
        }
    }
    None
}
