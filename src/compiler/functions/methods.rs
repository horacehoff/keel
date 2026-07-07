use crate::builtin_methods::builtin_methods;
use crate::compiler::get_id;
use crate::compiler_data::Variable;
use crate::compiler_data::{Ctx, State};
use crate::expr::{Expr, Span};
use crate::instr::Instr;
use smol_strc::SmolStr;

pub fn handle_method_calls(
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    tgt_id: Option<u16>,
    obj: &Expr,
    args: &[Expr],
    namespace: &[SmolStr],
    obj_markers: Span,
    fn_markers: Span,
    args_indexes: &[Span],
) -> Option<u16> {
    let name = namespace[namespace.len() - 1].as_str();

    let obj_type = obj.infer_type(v, ctx, state);
    let id = get_id(obj, v, ctx, state, output, None, false);
    state.free_reg(id, v);

    builtin_methods(
        name,
        id,
        obj_type,
        output,
        v,
        ctx,
        state,
        tgt_id,
        obj,
        args,
        obj_markers,
        fn_markers,
        args_indexes,
    )
}
