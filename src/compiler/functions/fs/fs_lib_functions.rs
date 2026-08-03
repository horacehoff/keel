use super::super::type_system::DataType;
use super::check_arg_type;
use crate::compiler::Scope;
use crate::compiler::UnwrapId;
use crate::compiler::compiler_data::Ctx;
use crate::compiler::compiler_data::State;
use crate::compiler::compiler_data::Variable;
use crate::compiler::compiler_errors::check_args;
use crate::compiler::compiler_errors::error_unknown_function;
use crate::compiler::expr::FunctionCallExpr;
use crate::instr::Instr;
use crate::instr::LibFunc;
use crate::instr::LibFuncVoid;

pub fn fs_lib_functions(
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx,
    state: &mut State<'_>,
    tgt_id: Option<u16>,
    function_call: &FunctionCallExpr,
) -> Option<u16> {
    let args = &function_call.args;
    let span = function_call.span;
    let arg_spans = &function_call.arg_spans;
    let name = function_call.qualified_name.get_name().as_str();
    match name {
        "read" => {
            check_args(args, 1, name, span, state.sources, ctx.file_idx);
            check_arg_type(name, v, ctx, state, args, arg_spans, 0, &[DataType::String]);
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
            check_args(args, 1, name, span, state.sources, ctx.file_idx);
            check_arg_type(name, v, ctx, state, args, arg_spans, 0, &[DataType::String]);
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
            check_args(args, 2, name, span, state.sources, ctx.file_idx);
            check_arg_type(name, v, ctx, state, args, arg_spans, 0, &[DataType::String]);
            check_arg_type(name, v, ctx, state, args, arg_spans, 1, &[DataType::String]);
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
            check_args(args, 2, name, span, state.sources, ctx.file_idx);
            check_arg_type(name, v, ctx, state, args, arg_spans, 0, &[DataType::String]);
            check_arg_type(name, v, ctx, state, args, arg_spans, 1, &[DataType::String]);
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
            check_args(args, 1, name, span, state.sources, ctx.file_idx);
            check_arg_type(name, v, ctx, state, args, arg_spans, 0, &[DataType::String]);
            let path = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(path, v);
            output.push(Instr::CallLibFuncVoid(LibFuncVoid::FsDelete, path, 0));
            state.add_to_src(ctx, output, span);
        }
        "delete_dir" => {
            check_args(args, 1, name, span, state.sources, ctx.file_idx);
            check_arg_type(name, v, ctx, state, args, arg_spans, 0, &[DataType::String]);
            let path = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(path, v);
            output.push(Instr::CallLibFuncVoid(LibFuncVoid::FsDeleteDir, path, 0));
            state.add_to_src(ctx, output, span);
        }
        fn_name => {
            error_unknown_function(
                fn_name,
                span,
                &Scope::default(),
                ctx.file_idx,
                state.sources,
            );
        }
    }
    None
}
