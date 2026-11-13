use super::strings::compile_str_concat_chain;
use crate::compiler::Ctx;
use crate::compiler::DataType;
use crate::compiler::Expr;
use crate::compiler::Instr;
use crate::compiler::Span;
use crate::compiler::State;
use crate::compiler::TypeExpr;
use crate::compiler::UnwrapId;
use crate::compiler::compiler_errors;
use crate::compiler::error_division_by_zero;
use crate::data::Data;
use crate::data::FALSE;

#[must_use]
pub(super) fn bool_op<'arena>(
    instr: fn(u16, u16, u16) -> Instr,
    symbol: &'static str,
    l: &'arena Expr,
    r: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let (t_l, t_r) = (l.infer_type(ctx, state), r.infer_type(ctx, state));
    if t_l != DataType::Bool || t_r != DataType::Bool {
        compiler_errors::error_op(&t_l, &t_r, symbol, span_l, span_r, ctx.file_idx, state.sources);
    }

    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    let id_r = r.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    state.free_reg(id_r);
    let id = state.alloc_reg_tgt(tgt_id);
    output.push(instr(id_l, id_r, id));
    id
}

#[inline]
#[must_use]
pub(super) fn numeric_op<'arena>(
    float_instr: fn(u16, u16, u16) -> Instr,
    int_instr: fn(u16, u16, u16) -> Instr,
    symbol: &'static str,
    l: &'arena Expr,
    r: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let (t_l, t_r) = (l.infer_type(ctx, state), r.infer_type(ctx, state));
    if t_l != t_r || !matches!(t_l, DataType::Float | DataType::Int) {
        compiler_errors::error_op(&t_l, &t_r, symbol, span_l, span_r, ctx.file_idx, state.sources);
    }
    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    let id_r = r.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    state.free_reg(id_r);
    let id = state.alloc_reg_tgt(tgt_id);
    output.push(if t_l == DataType::Float {
        float_instr(id_l, id_r, id)
    } else {
        int_instr(id_l, id_r, id)
    });
    id
}

#[must_use]
pub(super) fn compile_div_op<'arena>(
    l: &'arena Expr,
    r: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    if let Expr::Int(n) = r
        && *n == 0
    {
        error_division_by_zero(false, span_l.extend(span_r), ctx.file_idx, state.sources);
    }
    let id = numeric_op(
        Instr::DivFloat,
        Instr::DivInt,
        "/",
        l,
        r,
        span_l,
        span_r,
        tgt_id,
        ctx,
        state,
        output,
    );
    if matches!(output.last(), Some(Instr::DivInt(..))) {
        state.add_to_src(ctx, output, span_l.extend(span_r));
    }
    id
}

#[must_use]
pub(super) fn compile_add_op<'arena>(
    l: &'arena Expr,
    r: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let t_l = l.infer_type(ctx, state);
    let t_r = r.infer_type(ctx, state);
    if t_l != t_r
        || !matches!(t_l, DataType::String | DataType::Array(_) | DataType::Float | DataType::Int)
    {
        compiler_errors::error_op(&t_l, &t_r, "+", span_l, span_r, ctx.file_idx, state.sources);
    }
    // var+1 or 1+var use the dedicated IncInt/IncIntTo instructions
    if t_l == DataType::Int
        && let Some((src_expr, imm)) = match (l, r) {
            (_, Expr::Int(i)) => i16::try_from(*i).ok().map(|i| (l, i)),
            (Expr::Int(i), _) => i16::try_from(*i).ok().map(|i| (r, i)),
            _ => None,
        }
    {
        let src_id = src_expr.compile(ctx, state, output, None, false, true).unwrap_id();
        state.free_reg(src_id);
        let id = state.alloc_reg_tgt(tgt_id);
        output.push(Instr::AddIntImm { src: src_id, dst: id, imm: imm as u16 });
        return id;
    }
    if t_l == DataType::String && matches!(l, Expr::Add(..)) {
        return compile_str_concat_chain(l, r, tgt_id, ctx, state, output);
    }
    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    let id_r = r.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    state.free_reg(id_r);
    let id = state.alloc_reg_tgt(tgt_id);
    if matches!(t_l, DataType::Array(_)) {
        output.push(Instr::AddArray(id_l, id_r, id));
    } else if t_l == DataType::String {
        output.push(Instr::AddStr(id_l, id_r, id));
    } else if t_l == DataType::Float {
        output.push(Instr::AddFloat(id_l, id_r, id));
    } else {
        output.push(Instr::AddInt(id_l, id_r, id));
    }
    id
}

#[must_use]
pub(super) fn compile_sub_op<'arena>(
    l: &'arena Expr,
    r: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    // var-1 uses the dedicated DecInt/DecIntTo instructions
    if let Expr::Int(i) = r
        && let Some(imm) = i.checked_neg().and_then(|i| i16::try_from(i).ok())
        && l.infer_type(ctx, state) == DataType::Int
    {
        let src_id = l.compile(ctx, state, output, None, false, true).unwrap_id();
        state.free_reg(src_id);
        let id = state.alloc_reg_tgt(tgt_id);
        output.push(Instr::AddIntImm { src: src_id, dst: id, imm: imm as u16 });
        return id;
    }
    numeric_op(
        Instr::SubFloat,
        Instr::SubInt,
        "-",
        l,
        r,
        span_l,
        span_r,
        tgt_id,
        ctx,
        state,
        output,
    )
}

#[must_use]
pub(super) fn compile_mod_op<'arena>(
    l: &'arena Expr,
    r: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    if let Expr::Int(n) = r
        && *n == 0
    {
        error_division_by_zero(true, span_l.extend(span_r), ctx.file_idx, state.sources);
    }
    let id = numeric_op(
        Instr::ModFloat,
        Instr::ModInt,
        "%",
        l,
        r,
        span_l,
        span_r,
        tgt_id,
        ctx,
        state,
        output,
    );
    if matches!(output.last(), Some(Instr::ModInt(..))) {
        state.add_to_src(ctx, output, span_l.extend(span_r));
    }
    id
}

#[must_use]
pub(super) fn compile_eq_op<'arena>(
    l: &'arena Expr,
    r: &'arena Expr,
    negate: bool,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let l_type = l.infer_type(ctx, state);
    let r_type = r.infer_type(ctx, state);
    let is_obj = (matches!(l_type, DataType::Array(_) | DataType::Struct(_))
        && matches!(r_type, DataType::Array(_) | DataType::Struct(_)))
        || (l_type == DataType::String && r_type == DataType::String);
    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    let id_r = r.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    state.free_reg(id_r);
    let id = state.alloc_reg_tgt(tgt_id);
    output.push(match (is_obj, negate) {
        (true, false) => Instr::ObjEq(id_l, id_r, id),
        (true, true) => Instr::ObjNotEq(id_l, id_r, id),
        (false, false) => Instr::Eq(id_l, id_r, id),
        (false, true) => Instr::NotEq(id_l, id_r, id),
    });
    id
}

#[must_use]
pub(super) fn compile_neg_op<'arena>(
    l: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let operand_type = l.infer_type(ctx, state);
    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    let id = state.alloc_reg_tgt(tgt_id);
    if operand_type == DataType::Float {
        // keeps the -0.0 sign
        let minus_one = state.new_const_reg(Data::float(-1.0));
        output.push(Instr::MulFloat(id_l, minus_one, id));
    } else if operand_type == DataType::Int {
        let zero = state.new_const_reg(Data::int(0));
        output.push(Instr::SubInt(zero, id_l, id));
    } else {
        compiler_errors::error_op(
            &DataType::Null,
            &operand_type,
            "-",
            span_l,
            span_r,
            ctx.file_idx,
            state.sources,
        );
    }
    id
}

#[must_use]
pub(super) fn compile_bool_neg_op<'arena>(
    l: &'arena Expr,
    span_l: Span,
    span_r: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let operand_type = l.infer_type(ctx, state);
    let id_l = l.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(id_l);
    let id = state.alloc_reg_tgt(tgt_id);
    if operand_type != DataType::Bool {
        compiler_errors::error_op(
            &DataType::Null,
            &operand_type,
            "-",
            span_l,
            span_r,
            ctx.file_idx,
            state.sources,
        );
    }
    let false_reg = state.new_const_reg(FALSE);
    output.push(Instr::Eq(id_l, false_reg, id));
    id
}

pub(super) fn compile_type_eq_op<'arena>(
    value: &'arena Expr,
    type_candidate: &TypeExpr,
    span: Span,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let type_candidate =
        type_candidate.to_datatype(ctx.file_idx, state.scope(ctx.file_idx), state.sources);

    let value_type = value.infer_type(ctx, state);

    if matches!(value_type, DataType::Union(_) | DataType::Unknown) {
        let val_reg_id = value.compile(ctx, state, output, None, false, true).unwrap_id();
        state.free_reg(val_reg_id);
        let type_idx = state.compile_type(type_candidate);
        let dest_reg_id = state.alloc_reg_tgt(tgt_id);
        output.push(Instr::IsType(val_reg_id, type_idx, dest_reg_id));
        state.add_to_src(ctx, output, span);
        dest_reg_id
    } else {
        state.new_const_reg(Data::bool(value_type == type_candidate))
    }
}
