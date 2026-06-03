use crate::check_args;
use crate::errors::ErrType;
use crate::errors::throw_parser_error;
use crate::expr::Expr;
use crate::expr::Span;
use crate::functions::check_arg_type;
use crate::instr::Instr;
use crate::instr::LibFunc;
use crate::instr::LibFuncVoid;
use crate::parser::get_id;
use crate::parser_data::Ctx;
use crate::parser_data::State;
use crate::parser_data::Variable;
use crate::registers::alloc_register;
use crate::registers::free_register;
use crate::type_system::DataType;

pub fn fs_lib_functions(
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
) {
    let src = ctx.src;
    let current_src_file = ctx.current_src_file;
    match name {
        "read" => {
            check_args!(args, 1, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            free_register(id, state.free_registers, v, state.const_registers);
            output.push(Instr::CallLibFunc(
                LibFunc::FsRead,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
        }
        "exists" => {
            check_args!(args, 1, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            free_register(id, state.free_registers, v, state.const_registers);
            output.push(Instr::CallLibFunc(
                LibFunc::FsExists,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
        }
        "write" => {
            check_args!(args, 2, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            check_arg_type(v, ctx, state, args, args_indexes, 1, &[DataType::String]);
            let filepath = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            let contents = get_id(
                &args[1], v, ctx, state, output, None, false, offset, single_run,
            );
            free_register(filepath, state.free_registers, v, state.const_registers);
            free_register(contents, state.free_registers, v, state.const_registers);
            output.push(Instr::CallLibFuncVoid(
                LibFuncVoid::FsWrite,
                filepath,
                contents,
            ));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
        }
        "append" => {
            check_args!(args, 2, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            check_arg_type(v, ctx, state, args, args_indexes, 1, &[DataType::String]);
            let filepath = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            let contents = get_id(
                &args[1], v, ctx, state, output, None, false, offset, single_run,
            );
            free_register(filepath, state.free_registers, v, state.const_registers);
            free_register(contents, state.free_registers, v, state.const_registers);
            output.push(Instr::CallLibFuncVoid(
                LibFuncVoid::FsAppend,
                filepath,
                contents,
            ));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
        }
        "delete" => {
            check_args!(args, 1, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let path = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            free_register(path, state.free_registers, v, state.const_registers);
            output.push(Instr::CallLibFuncVoid(LibFuncVoid::FsDelete, path, 0));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
        }
        "delete_dir" => {
            check_args!(args, 1, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let path = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            free_register(path, state.free_registers, v, state.const_registers);
            output.push(Instr::CallLibFuncVoid(LibFuncVoid::FsDeleteDir, path, 0));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
        }
        name => {
            throw_parser_error(src, markers, ErrType::UnknownFunction(name));
        }
    }
}
