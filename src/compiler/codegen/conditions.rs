use crate::compiler::Ctx;
use crate::compiler::DataType;
use crate::compiler::Expr;
use crate::compiler::State;
use crate::compiler::registers::UnwrapId;
use crate::data::FALSE;
use crate::data::TRUE;
use crate::instr::Instr;
use core::hint::unreachable_unchecked;

/// Returns the (true,false) fused instructions for `instr`
const fn fused_jumps(instr: Instr, condition_id: u16) -> Option<(Instr, Instr)> {
    if !matches!(instr.get_tgt_id(), Some(id) if id == condition_id) {
        return None;
    }
    Some(match instr {
        Instr::InfFloat(a, b, _) => (Instr::InfFloatJmp(a, b, 0), Instr::NotInfFloatJmp(a, b, 0)),
        Instr::InfInt(a, b, _) => (Instr::InfIntJmp(a, b, 0), Instr::InfEqIntJmp(b, a, 0)),
        Instr::InfEqFloat(a, b, _) => {
            (Instr::InfEqFloatJmp(a, b, 0), Instr::NotInfEqFloatJmp(a, b, 0))
        }
        Instr::InfEqInt(a, b, _) => (Instr::InfEqIntJmp(a, b, 0), Instr::InfIntJmp(b, a, 0)),
        Instr::Eq(a, b, _) => (Instr::EqJmp(a, b, 0), Instr::NotEqJmp(a, b, 0)),
        Instr::ObjEq(a, b, _) => (Instr::ObjEqJmp(a, b, 0), Instr::ObjNotEqJmp(a, b, 0)),
        Instr::NotEq(a, b, _) => (Instr::NotEqJmp(a, b, 0), Instr::EqJmp(a, b, 0)),
        Instr::ObjNotEq(a, b, _) => (Instr::ObjNotEqJmp(a, b, 0), Instr::ObjEqJmp(a, b, 0)),
        _ => return None,
    })
}

/// Fuses the last comparison instruction into a jump instruction (jumps when condition is false)
fn add_cmp_false(condition_id: u16, state: &mut State<'_, '_>, output: &mut Vec<Instr>) {
    let Some((_, jump_instr)) = output.last().and_then(|&instr| fused_jumps(instr, condition_id))
    else {
        return output.push(Instr::EqJmp(condition_id, state.new_const_reg(FALSE), 0));
    };
    *output.last_mut().unwrap() = jump_instr;
}

fn compile_int_imm_false_jump<'arena>(
    condition: &'arena Expr,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> bool {
    let (l, bound, jump): (_, _, fn(u16, u16, u16) -> Instr) = match condition {
        Expr::Eq(l, Expr::Int(i)) => (l, Some(*i), Instr::NotEqIntImmJmp),
        Expr::NotEq(l, Expr::Int(i)) => (l, Some(*i), Instr::EqIntImmJmp),
        Expr::Inf(l, Expr::Int(i), _, _) => (l, Some(*i), Instr::SupEqIntImmJmp),
        Expr::InfEq(l, Expr::Int(i), _, _) => (l, i.checked_add(1), Instr::SupEqIntImmJmp),
        Expr::Sup(l, Expr::Int(i), _, _) => (l, i.checked_add(1), Instr::InfIntImmJmp),
        Expr::SupEq(l, Expr::Int(i), _, _) => (l, Some(*i), Instr::InfIntImmJmp),
        _ => return false,
    };
    if let Some(imm) = bound.and_then(|v| i16::try_from(v).ok())
        && l.infer_type(ctx, state) == DataType::Int
    {
        let id = l.compile(ctx, state, output, None, false, true).unwrap_id();
        state.free_reg(id);
        output.push(jump(id, imm as u16, 0));
        true
    } else {
        false
    }
}

/// Fuses the last comparison instruction into a jump instruction (jumps when condition is true)
#[inline(always)]
fn add_cmp_true(condition_id: u16, state: &mut State<'_, '_>, output: &mut Vec<Instr>) {
    match output.last().and_then(|&instr| fused_jumps(instr, condition_id)) {
        Some((jump_instr, _)) => *output.last_mut().unwrap() = jump_instr,
        None => output.push(Instr::EqJmp(condition_id, state.new_const_reg(TRUE), 0)),
    }
}

/// Sets the jump size field of a jump instruction
#[inline(always)]
const fn set_jmp_size(instr: &mut Instr, size: u16) {
    match instr {
        Instr::Jmp(jump_size)
        | Instr::InfEqFloatJmp(_, _, jump_size)
        | Instr::InfEqIntJmp(_, _, jump_size)
        | Instr::InfFloatJmp(_, _, jump_size)
        | Instr::InfIntJmp(_, _, jump_size)
        | Instr::NotInfFloatJmp(_, _, jump_size)
        | Instr::NotInfEqFloatJmp(_, _, jump_size)
        | Instr::NotEqJmp(_, _, jump_size)
        | Instr::EqJmp(_, _, jump_size)
        | Instr::ObjNotEqJmp(_, _, jump_size)
        | Instr::ObjEqJmp(_, _, jump_size)
        | Instr::EqIntImmJmp(_, _, jump_size)
        | Instr::NotEqIntImmJmp(_, _, jump_size)
        | Instr::SupEqIntImmJmp(_, _, jump_size)
        | Instr::InfIntImmJmp(_, _, jump_size)
        | Instr::IncIntInfJmp { jump_size, .. }
        | Instr::IsNotTypeJmp { jump_size, .. } => *jump_size = size,
        _ => unsafe { unreachable_unchecked() },
    }
}

pub(super) fn patch_jumps(
    output: &mut [Instr],
    jump_idxs: impl IntoIterator<Item = usize>,
    target: usize,
) {
    for jump_idx in jump_idxs {
        set_jmp_size(&mut output[jump_idx], (target - jump_idx) as u16);
    }
}

/// Compiles short-circuit && and || conditions
/// bool_or_mode true indicates left side of ||, emits true jumps
/// bool_or_mode false emits false jumps
/// Returns (true_jump_idxs, false_jump_idxs)
#[must_use]
pub(super) fn compile_short_circuit_condition<'arena>(
    condition: &'arena Expr,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
    bool_or_mode: bool,
) -> (Vec<usize>, Vec<usize>) {
    match condition {
        Expr::BoolOr(left, right, _, _) => {
            // left side of || always uses true jump mode
            let (mut true_jumps, _) =
                compile_short_circuit_condition(left, ctx, state, output, true);
            let (right_true, right_false) =
                compile_short_circuit_condition(right, ctx, state, output, bool_or_mode);
            true_jumps.extend(right_true);
            (true_jumps, right_false)
        }
        Expr::BoolAnd(left, right, _, _) => {
            if bool_or_mode {
                // && inside left side of ||
                let id_l = left.compile(ctx, state, output, None, false, true).unwrap_id();
                let id_r = right.compile(ctx, state, output, None, false, true).unwrap_id();
                state.free_reg(id_l);
                state.free_reg(id_r);
                let id = state.alloc_reg();
                output.push(Instr::BoolAnd(id_l, id_r, id));
                add_cmp_true(id, state, output);
                state.free_reg(id);
                (vec![output.len() - 1], Vec::new())
            } else {
                // normal && -> if either side is false, jump past the body
                let (_, mut false_jumps) =
                    compile_short_circuit_condition(left, ctx, state, output, false);
                let (_, right_false) =
                    compile_short_circuit_condition(right, ctx, state, output, false);
                false_jumps.extend(right_false);
                (Vec::new(), false_jumps)
            }
        }
        expr => {
            if !bool_or_mode && compile_int_imm_false_jump(expr, ctx, state, output) {
                return (Vec::new(), vec![output.len() - 1]);
            }
            let cond_id = expr.compile(ctx, state, output, None, false, true).unwrap_id();
            if bool_or_mode {
                add_cmp_true(cond_id, state, output);
                state.free_reg(cond_id);
                (vec![output.len() - 1], Vec::new())
            } else {
                add_cmp_false(cond_id, state, output);
                state.free_reg(cond_id);
                (Vec::new(), vec![output.len() - 1])
            }
        }
    }
}

#[must_use]
fn compile_const_condition(condition: &Expr) -> Option<bool> {
    match condition {
        Expr::Bool(b) => Some(*b),
        Expr::BoolNeg(b, _, _) => compile_const_condition(b).map(|b| !b),
        _ => None,
    }
}

#[must_use]
pub(super) fn compile_condition<'arena>(
    condition: &'arena Expr,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> (Option<bool>, Vec<usize>, Vec<usize>) {
    if let Some(b) = compile_const_condition(condition) {
        return (Some(b), Vec::new(), Vec::new());
    }

    if matches!(condition, Expr::BoolAnd(_, _, _, _) | Expr::BoolOr(_, _, _, _)) {
        let (true_jump_idxs, false_jmp_idxes) =
            compile_short_circuit_condition(condition, ctx, state, output, false);
        return (None, true_jump_idxs, false_jmp_idxes);
    }

    if compile_int_imm_false_jump(condition, ctx, state, output) {
        return (None, Vec::new(), vec![output.len() - 1]);
    }
    let cond_id = condition.compile(ctx, state, output, None, false, true).unwrap_id();
    if state.regs.const_registers.get(&TRUE) == Some(&cond_id) {
        return (Some(true), Vec::new(), Vec::new());
    } else if state.regs.const_registers.get(&FALSE) == Some(&cond_id) {
        return (Some(false), Vec::new(), Vec::new());
    }
    add_cmp_false(cond_id, state, output);
    state.free_reg(cond_id);
    (None, Vec::new(), vec![output.len() - 1])
}
