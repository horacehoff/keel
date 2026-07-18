use super::super::expr::Expr;
use super::super::expr::Span;
use super::super::type_system::DataType;
use super::check_arg_type;
use crate::compiler::UnwrapId;
use crate::compiler::compiler_data::Ctx;
use crate::compiler::compiler_data::State;
use crate::compiler::compiler_data::Variable;
use crate::compiler::error_unknown_function;
use crate::instr::Instr;
use crate::instr::LibFunc;
use crate::instr::LibFuncVoid;
use crate::util::check_args;

pub fn fs_lib_functions(
    name: &str,
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    tgt_id: Option<u16>,
    args: &[Expr],
    span: Span,
    args_indexes: &[Span],
) -> Option<u16> {
    let src = ctx.src;
    match name {
        "read" => {
            check_args(args, 1, name, src, span, state.sources);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(id, v);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::FsRead, id, output_id));
            state.add_to_src(ctx, output, span);
            return Some(output_id);
        }
        "exists" => {
            check_args(args, 1, name, src, span, state.sources);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(id, v);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::FsExists, id, output_id));
            state.add_to_src(ctx, output, span);
            return Some(output_id);
        }
        "write" => {
            check_args(args, 2, name, src, span, state.sources);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            check_arg_type(v, ctx, state, args, args_indexes, 1, &[DataType::String]);
            let filepath = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            let contents = args[1]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(filepath, v);
            state.free_reg(contents, v);
            output.push(Instr::CallLibFuncVoid(
                LibFuncVoid::FsWrite,
                filepath,
                contents,
            ));
            state.add_to_src(ctx, output, span);
        }
        "append" => {
            check_args(args, 2, name, src, span, state.sources);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            check_arg_type(v, ctx, state, args, args_indexes, 1, &[DataType::String]);
            let filepath = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            let contents = args[1]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(filepath, v);
            state.free_reg(contents, v);
            output.push(Instr::CallLibFuncVoid(
                LibFuncVoid::FsAppend,
                filepath,
                contents,
            ));
            state.add_to_src(ctx, output, span);
        }
        "delete" => {
            check_args(args, 1, name, src, span, state.sources);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let path = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(path, v);
            output.push(Instr::CallLibFuncVoid(LibFuncVoid::FsDelete, path, 0));
            state.add_to_src(ctx, output, span);
        }
        "delete_dir" => {
            check_args(args, 1, name, src, span, state.sources);
            check_arg_type(v, ctx, state, args, args_indexes, 0, &[DataType::String]);
            let path = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(path, v);
            output.push(Instr::CallLibFuncVoid(LibFuncVoid::FsDeleteDir, path, 0));
            state.add_to_src(ctx, output, span);
        }
        fn_name => {
            error_unknown_function(fn_name, span, std::iter::empty(), src, state.sources);
        }
    }
    None
}
