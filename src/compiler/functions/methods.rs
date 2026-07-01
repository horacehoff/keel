use crate::compiler::get_id;
use crate::compiler_data::Variable;
use crate::compiler_data::{Ctx, State};
use crate::expr::{Expr, Span};
use crate::instr::Instr;
use crate::std_lib_methods::std_lib_methods;
use crate::type_system::infer_type;
use smol_strc::SmolStr;

pub fn handle_method_calls(
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    obj: &Expr,
    args: &[Expr],
    namespace: &[SmolStr],
    obj_markers: Span,
    fn_markers: Span,
    args_indexes: &[Span],
    offset: u16,
    single_run: bool,
) {
    let name = namespace[namespace.len() - 1].as_str();

    let obj_type = infer_type(obj, v, ctx, state);
    let id = get_id(obj, v, ctx, state, output, None, false, offset, single_run);
    state.free_reg(id, v);

    std_lib_methods(
        name,
        id,
        obj_type,
        output,
        v,
        ctx,
        state,
        obj,
        args,
        obj_markers,
        fn_markers,
        args_indexes,
        offset,
        single_run,
    );
}
