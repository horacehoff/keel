use crate::ast::Expr;
use crate::compiler::DataType;
use crate::compiler::functions::compile_function_call;
use crate::compiler::methods::compile_method_call;
use crate::compiler::registers::UnwrapId;
use crate::compiler::registers::move_value_to;
use crate::compiler::state::Ctx;
use crate::compiler::state::State;
use crate::data::Data;
use crate::data::NULL;
use crate::instr::Instr;
use crate::vm::StringPool;
use access::compile_array_indexing;
use access::compile_array_slice;
use access::compile_struct_field_access;
use access::compile_var_access;
use assignment::compile_array_index_assignment;
use assignment::compile_struct_field_assignment;
use assignment::compile_var_assignment;
use assignment::compile_var_declaration;
use control_flow::compile_eval_block;
use control_flow::compile_for_loop;
use control_flow::compile_if_block;
use control_flow::compile_int_for_loop;
use control_flow::compile_loop_block;
use control_flow::compile_loop_break;
use control_flow::compile_loop_continue;
use control_flow::compile_match_block;
use control_flow::compile_return;
use control_flow::compile_try_catch_block;
use control_flow::compile_while_loop;
use core::hint::unreachable_unchecked;
use definitions::compile_function_definition;
use definitions::compile_struct_definition;
use literals::compile_array_literal;
use literals::compile_map_literal;
use literals::compile_struct_literal;
use operators::bool_op;
use operators::compile_add_op;
use operators::compile_bool_neg_op;
use operators::compile_div_op;
use operators::compile_eq_op;
use operators::compile_mod_op;
use operators::compile_neg_op;
use operators::compile_sub_op;
use operators::compile_type_eq_op;
use operators::numeric_op;
pub(super) use patterns::declare_pattern_bindings;
pub(super) use patterns::type_after_mismatch;
use strings::apply_loop_in_place_str_appends;
use strings::is_eligible_for_in_place_append;
use strings::rewrite_str_concats;

mod access;
mod assignment;
mod conditions;
mod control_flow;
mod definitions;
mod literals;
mod operators;
mod patterns;
mod strings;

/// Compiles `exprs` in contiguous registers
#[must_use]
pub fn compile_reg_window<'arena>(
    exprs: impl ExactSizeIterator<Item = &'arena Expr<'arena>>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let first = state.alloc_reg_window(exprs.len() as u16);
    for (i, expr) in exprs.enumerate() {
        let tgt = first + i as u16;
        let start_len = output.len();
        let reg = expr.compile(ctx, state, output, Some(tgt), false, true).unwrap_id();
        move_value_to(output, start_len, reg, tgt);
        if reg != tgt {
            state.free_reg(reg);
        }
    }
    first
}

pub fn compile_expr<'arena>(
    input: &'arena [Expr<'arena>],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
) -> Vec<Instr> {
    let v_len = state.v.len();
    let fn_len = state.functions.len();
    let symbols_len = state.scope(ctx.file_idx).symbols.len();
    let mut output: Vec<Instr> = Vec::with_capacity(input.len());
    let mut str_regs = Vec::new();
    for (idx, x) in input.iter().enumerate() {
        let v_len = state.v.len();
        if let Some(id) = x.compile_with_code_context(
            ctx,
            state,
            &mut output,
            None,
            false,
            &input[idx + 1..],
            false,
        ) {
            state.free_reg(id);
        }
        if let Some(v) = state.v[v_len..].last()
            && v.declared_type == DataType::String
        {
            let v_reg = v.register_id;
            if matches!(output.last(), Some(&Instr::Mov(_, dst)) if dst == v_reg) {
                str_regs.push((v_reg, output.len() - 1));
            } else if ctx.single_run && !state.is_register_const(v_reg) {
                str_regs.push((v_reg, output.len()));
            }
        }
    }
    for (reg, start) in str_regs {
        if is_eligible_for_in_place_append(&output[start..], reg) {
            rewrite_str_concats(&mut output[start..], &[reg], state);
        }
    }
    for var_idx in v_len..state.v.len() {
        state.free_reg(state.v[var_idx].register_id);
    }
    state.v.truncate(v_len);
    state.functions.truncate(fn_len);
    state.scope_mut(ctx.file_idx).symbols.truncate(symbols_len);
    output
}

impl<'arena> Expr<'arena> {
    const fn is_constant_literal(&self) -> bool {
        matches!(self, Self::Int(_) | Self::Float(_) | Self::String(_) | Self::Bool(_) | Self::Null)
    }
    fn literal_value(&self, str_pool: &mut StringPool) -> Option<Data> {
        match self {
            Self::Int(int) => Some(Data::int(*int)),
            Self::Float(float) => Some(Data::float(*float)),
            Self::String(s) => Some(Data::comp_str(s, str_pool)),
            Self::Bool(bool) => Some(Data::bool(*bool)),
            Self::Null => Some(NULL),
            _ => None,
        }
    }
    #[inline(always)]
    pub fn compile(
        &'arena self,
        ctx: Ctx,
        state: &mut State<'arena, '_>,
        output: &mut Vec<Instr>,
        tgt_id: Option<u16>,
        var_assignment: bool,
        uses_id: bool,
    ) -> Option<u16> {
        self.compile_with_code_context(ctx, state, output, tgt_id, var_assignment, &[], uses_id)
    }
    pub fn compile_with_code_context(
        &'arena self,
        ctx: Ctx,
        state: &mut State<'arena, '_>,
        output: &mut Vec<Instr>,
        tgt_id: Option<u16>,
        var_assignment: bool,
        remaining_code: &[Self],
        uses_id: bool,
    ) -> Option<u16> {
        match self {
            Self::Int(_) | Self::Float(_) | Self::String(_) | Self::Null | Self::Bool(_) => {
                debug_assert!(uses_id);
                let data =
                    unsafe { self.literal_value(&mut state.pools.str_pool).unwrap_unchecked() };
                Some(if var_assignment {
                    state.init_reg(data, ctx)
                } else {
                    state.new_const_reg(data)
                })
            }
            Self::Var(name, span) => {
                debug_assert!(uses_id);
                Some(compile_var_access(name, *span, ctx, state, output))
            }
            Self::TypeEq(value, type_candidate, span) => {
                debug_assert!(uses_id);
                Some(compile_type_eq_op(value, type_candidate, *span, tgt_id, ctx, state, output))
            }
            Self::Array(array_items, spans) => {
                debug_assert!(uses_id);
                Some(compile_array_literal(array_items, spans, ctx, state, output))
            }
            Self::Struct(name, fields, span) => {
                debug_assert!(uses_id);
                Some(compile_struct_literal(name, fields, *span, ctx, state, output))
            }
            Self::Map(kv_pairs, span) => {
                debug_assert!(uses_id);
                Some(compile_map_literal(kv_pairs, *span, ctx, state, output))
            }
            Self::GetStructField(struct_expr, field, struct_span, field_span) => {
                debug_assert!(uses_id);
                Some(compile_struct_field_access(
                    struct_expr,
                    field,
                    *struct_span,
                    *field_span,
                    ctx,
                    state,
                    output,
                ))
            }
            // array[index]
            Self::ArrayGetIndex(array, index, span) => {
                debug_assert!(uses_id);
                Some(compile_array_indexing(array, index, *span, ctx, state, output))
            }
            // array[start..end]
            Self::ArrayGetSlice(array, idx_start, idx_end, span) => {
                debug_assert!(uses_id);
                Some(compile_array_slice(array, idx_start, idx_end, *span, ctx, state, output))
            }
            Self::Mul(l, r, span1, span2)
            | Self::Pow(l, r, span1, span2)
            | Self::Sup(l, r, span1, span2)
            | Self::SupEq(l, r, span1, span2)
            | Self::Inf(l, r, span1, span2)
            | Self::InfEq(l, r, span1, span2) => {
                debug_assert!(uses_id);
                let (float_instr, int_instr, symbol): (
                    fn(u16, u16, u16) -> Instr,
                    fn(u16, u16, u16) -> Instr,
                    &'static str,
                ) = match self {
                    Self::Mul(..) => (Instr::MulFloat, Instr::MulInt, "*"),
                    Self::Pow(..) => (Instr::PowFloat, Instr::PowInt, "^"),
                    Self::Sup(..) => (
                        |a, b, jmp| Instr::InfFloat(b, a, jmp),
                        |a, b, jmp| Instr::InfInt(b, a, jmp),
                        ">",
                    ),
                    Self::SupEq(..) => (
                        |a, b, jmp| Instr::InfEqFloat(b, a, jmp),
                        |a, b, jmp| Instr::InfEqInt(b, a, jmp),
                        ">=",
                    ),
                    Self::Inf(..) => (Instr::InfFloat, Instr::InfInt, "<"),
                    _ => (Instr::InfEqFloat, Instr::InfEqInt, "<="),
                };
                Some(numeric_op(
                    float_instr,
                    int_instr,
                    symbol,
                    l,
                    r,
                    *span1,
                    *span2,
                    tgt_id,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::Div(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(compile_div_op(l, r, *span1, *span2, tgt_id, ctx, state, output))
            }
            Self::Add(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(compile_add_op(l, r, *span1, *span2, tgt_id, ctx, state, output))
            }
            Self::Sub(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(compile_sub_op(l, r, *span1, *span2, tgt_id, ctx, state, output))
            }
            Self::Mod(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(compile_mod_op(l, r, *span1, *span2, tgt_id, ctx, state, output))
            }
            Self::Eq(l, r) => {
                debug_assert!(uses_id);
                Some(compile_eq_op(l, r, false, tgt_id, ctx, state, output))
            }
            Self::NotEq(l, r) => {
                debug_assert!(uses_id);
                Some(compile_eq_op(l, r, true, tgt_id, ctx, state, output))
            }
            Self::BoolAnd(l, r, s1, s2) => {
                debug_assert!(uses_id);
                Some(bool_op(Instr::BoolAnd, "&&", l, r, *s1, *s2, tgt_id, ctx, state, output))
            }
            Self::BoolOr(l, r, span1, span2) => {
                debug_assert!(uses_id);
                Some(bool_op(Instr::BoolOr, "||", l, r, *span1, *span2, tgt_id, ctx, state, output))
            }
            Self::Neg(l, span1, span2) => {
                debug_assert!(uses_id);
                Some(compile_neg_op(l, *span1, *span2, tgt_id, ctx, state, output))
            }
            Self::BoolNeg(l, span1, span2) => {
                debug_assert!(uses_id);
                Some(compile_bool_neg_op(l, *span1, *span2, tgt_id, ctx, state, output))
            }
            Self::FunctionCall(function_call) => {
                let output_id = compile_function_call(function_call, output, ctx, state, tgt_id);
                if uses_id {
                    Some(output_id.unwrap_or_else(|| state.new_const_reg(NULL)))
                } else {
                    if let Some(id) = output_id {
                        state.free_reg(id);
                    }
                    None
                }
            }
            Self::ObjFunctionCall(function_call) => {
                let output_id = compile_method_call(output, ctx, state, tgt_id, function_call);
                if uses_id {
                    Some(output_id.unwrap_or_else(|| state.new_const_reg(NULL)))
                } else {
                    if let Some(id) = output_id {
                        state.free_reg(id);
                    }
                    None
                }
            }
            Self::AnonymousFunction(_, _, _) => {
                debug_assert!(uses_id);
                // This is replaced later on by `builtin_functions` when it's called
                Some(state.value_reg(Data::function(0)))
            }

            // ------------------
            // --- STATEMENTS ---
            // ------------------

            // x[y] = z;
            Self::ArrayModify(array, index, value, index_markers, elem_markers) => {
                debug_assert!(!uses_id);
                compile_array_index_assignment(
                    array,
                    index,
                    value,
                    *index_markers,
                    *elem_markers,
                    ctx,
                    state,
                    output,
                );
                None
            }
            Self::SetStructField(struct_field_assignment) => {
                debug_assert!(!uses_id);
                compile_struct_field_assignment(struct_field_assignment, ctx, state, output);
                None
            }
            Self::IfBlock(if_block) => {
                let condition_return_id =
                    if uses_id { Some(state.alloc_reg_tgt(tgt_id)) } else { None };
                compile_if_block(if_block, Vec::new(), condition_return_id, ctx, state, output);
                condition_return_id
            }
            Self::WhileBlock(condition, code) => {
                debug_assert!(!uses_id);
                let loop_start = output.len();
                compile_while_loop(condition, code, ctx, state, output);
                apply_loop_in_place_str_appends(loop_start, ctx, state, output);
                None
            }
            Self::ForLoop(var_name, array, code, span) => {
                debug_assert!(!uses_id);
                let loop_start = output.len();
                compile_for_loop(var_name, array, code, *span, ctx, state, output);
                apply_loop_in_place_str_appends(loop_start, ctx, state, output);
                None
            }
            Self::IntForLoop(int_for_loop) => {
                debug_assert!(!uses_id);
                let loop_start = output.len();
                compile_int_for_loop(int_for_loop, ctx, state, output);
                apply_loop_in_place_str_appends(loop_start, ctx, state, output);
                None
            }
            Self::LoopBlock(code) => {
                debug_assert!(!uses_id);
                let loop_start = output.len();
                compile_loop_block(code, ctx, state, output);
                apply_loop_in_place_str_appends(loop_start, ctx, state, output);
                None
            }
            Self::TryCatchBlock(e, err_var, catch_code) => {
                debug_assert!(!uses_id);
                compile_try_catch_block(e, err_var, catch_code, ctx, state, output);
                None
            }
            Self::VarDeclare(var_declaration) => {
                debug_assert!(!uses_id);
                compile_var_declaration(var_declaration, remaining_code, ctx, state, output);
                None
            }
            Self::VarAssign(name, value, span) => {
                debug_assert!(!uses_id);
                compile_var_assignment(
                    name.get_namespace(),
                    name.get_name(),
                    value,
                    *span,
                    ctx,
                    state,
                    output,
                );
                None
            }
            Self::StructDeclare(name, fields, span) => {
                debug_assert!(!uses_id);
                compile_struct_definition(name, fields, *span, ctx, state);
                None
            }
            Self::FunctionDecl(function_declaration) => {
                debug_assert!(!uses_id);
                compile_function_definition(function_declaration, ctx, state);
                None
            }
            Self::ReturnVal(return_value) => {
                debug_assert!(!uses_id);
                compile_return(*return_value, ctx, state, output);
                None
            }
            Self::Break => {
                debug_assert!(!uses_id);
                compile_loop_break(ctx, output);
                None
            }
            Self::Continue => {
                debug_assert!(!uses_id);
                compile_loop_continue(ctx, output);
                None
            }
            Self::EvalBlock(code) => {
                debug_assert!(!uses_id);
                compile_eval_block(code, ctx, state, output);
                None
            }
            Self::Match(match_expr) => {
                debug_assert!(!uses_id);
                compile_match_block(match_expr, ctx, state, output);
                None
            }

            Self::ImportDylib(..) | Self::ImportFile(..) => unsafe { unreachable_unchecked() },
        }
    }
}
