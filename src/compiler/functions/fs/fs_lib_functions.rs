use crate::compiler::get_id;
use crate::compiler_data::Ctx;
use crate::compiler_data::State;
use crate::compiler_data::Variable;
use crate::errors::ErrType;
use crate::errors::throw_compiler_error;
use crate::expr::Expr;
use crate::expr::Span;
use crate::functions::check_arg_type;
use crate::instr::Instr;
use crate::instr::LibFunc;
use crate::instr::LibFuncVoid;
use crate::type_system::DataType;
use crate::util::check_args;

pub fn fs_lib_functions(
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
        "read" => {
            check_args(args, 1, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let id = get_id(&args[0], v, ctx, state, output, None, false);
            state.free_reg(id, v);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::FsRead, id, output_id));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
            return Some(output_id);
        }
        "exists" => {
            check_args(args, 1, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let id = get_id(&args[0], v, ctx, state, output, None, false);
            state.free_reg(id, v);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::FsExists, id, output_id));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
            return Some(output_id);
        }
        "write" => {
            check_args(args, 2, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            check_arg_type(v, ctx, state, args, args_indexes, 1, &[DataType::String]);
            let filepath = get_id(&args[0], v, ctx, state, output, None, false);
            let contents = get_id(&args[1], v, ctx, state, output, None, false);
            state.free_reg(filepath, v);
            state.free_reg(contents, v);
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
            check_args(args, 2, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            check_arg_type(v, ctx, state, args, args_indexes, 1, &[DataType::String]);
            let filepath = get_id(&args[0], v, ctx, state, output, None, false);
            let contents = get_id(&args[1], v, ctx, state, output, None, false);
            state.free_reg(filepath, v);
            state.free_reg(contents, v);
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
            check_args(args, 1, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let path = get_id(&args[0], v, ctx, state, output, None, false);
            state.free_reg(path, v);
            output.push(Instr::CallLibFuncVoid(LibFuncVoid::FsDelete, path, 0));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
        }
        "delete_dir" => {
            check_args(args, 1, name, src, markers);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let path = get_id(&args[0], v, ctx, state, output, None, false);
            state.free_reg(path, v);
            output.push(Instr::CallLibFuncVoid(LibFuncVoid::FsDeleteDir, path, 0));
            state
                .instr_src
                .push((*output.last().unwrap(), markers, current_src_file));
        }
        name => {
            throw_compiler_error(src, markers, ErrType::UnknownFunction(name));
        }
    }
    None
}
