use crate::compiler::UnwrapId;
use crate::compiler::compiler_data::Variable;
use crate::compiler::compiler_data::{Ctx, State};
use crate::compiler::expr::FunctionCallExpr;
use crate::instr::Instr;
use builtin_methods::builtin_methods;

#[path = "builtin/builtin_methods.rs"]
mod builtin_methods;

pub fn compile_method_call(
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx,
    state: &mut State<'_>,
    tgt_id: Option<u16>,
    function_call: &FunctionCallExpr,
) -> Option<u16> {
    let obj_type = function_call.args[0].infer_type(v, ctx, state);
    let id = function_call.args[0]
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();

    if function_call.qualified_name.get_name() != "map"
        && function_call.qualified_name.get_name() != "filter"
    {
        state.free_reg(id, v);
    }

    let output_id = builtin_methods(id, obj_type, output, v, ctx, state, tgt_id, function_call);

    state.free_reg(id, v);

    output_id
}
