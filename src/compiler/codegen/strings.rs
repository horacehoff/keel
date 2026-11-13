use crate::ast::Expr;
use crate::compiler::DataType;
use crate::compiler::UnwrapId;
use crate::compiler::state::Ctx;
use crate::compiler::state::State;
use crate::compiler::state::Variable;
use crate::data::Data;
use crate::instr::Instr;

enum StrRegUse {
    // safe to append in place!
    AppendInPlace,
    // aliases or escapes the reg => can't rewrite in-place
    Unsafe,
    Neutral,
}

pub(super) fn compile_str_concat_chain<'arena>(
    l: &'arena Expr,
    r: &'arena Expr,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let mut operands = vec![r];
    let mut current_node = l;
    while let Expr::Add(add_l, add_r, _, _) = current_node
        && add_l.infer_type(ctx, state) == DataType::String
        && add_r.infer_type(ctx, state) == DataType::String
    {
        operands.push(add_r);
        current_node = add_l;
    }
    operands.push(current_node);
    let ids: Vec<u16> = operands
        .iter()
        .rev()
        .map(|e| e.compile(ctx, state, output, None, false, true).unwrap_id())
        .collect();
    // the first two elements are filtered out because they require an AddStr
    let dst = tgt_id.filter(|t| !ids[2..].contains(t)).unwrap_or_else(|| state.alloc_reg());
    output.push(Instr::AddStr(ids[0], ids[1], dst));
    output.extend(ids[2..].iter().map(|&id| Instr::AddStrInPlace(dst, id)));
    ids.iter().filter(|&&id| id != dst).for_each(|&id| state.free_reg(id));
    if let Some(t) = tgt_id
        && t != dst
    {
        output.push(Instr::Mov(dst, t));
        state.free_reg(dst);
        t
    } else {
        dst
    }
}

fn categorize_str_reg_use(instr: Instr, reg: u16) -> StrRegUse {
    let is_safe = match instr {
        Instr::AddStr(o1, o2, dst) if o1 == reg && o2 != reg && dst == reg => {
            return StrRegUse::AppendInPlace;
        }
        Instr::AddStr(..) | Instr::AddStrInPlace(..) | Instr::Return(_) => true,
        Instr::Print(_)
        | Instr::EqJmp(..)
        | Instr::NotEqJmp(..)
        | Instr::ObjEqJmp(..)
        | Instr::ObjNotEqJmp(..)
        | Instr::Len(..)
        | Instr::Eq(..)
        | Instr::NotEq(..)
        | Instr::ObjEq(..)
        | Instr::ObjNotEq(..)
        | Instr::GetIndexString(..)
        | Instr::Contains { .. }
        | Instr::Find { .. }
        | Instr::StartsWith { .. }
        | Instr::EndsWith { .. } => instr.get_tgt_id() != Some(reg),
        Instr::Mov(src, dst) => src == dst || dst == reg,
        _ => false,
    };
    if is_safe || !instr.uses_reg(reg) { StrRegUse::Neutral } else { StrRegUse::Unsafe }
}

#[inline]
pub(super) fn is_eligible_for_in_place_append(code: &[Instr], reg: u16) -> bool {
    let mut can_append = false;
    for &instr in code {
        match categorize_str_reg_use(instr, reg) {
            StrRegUse::Unsafe => return false,
            StrRegUse::AppendInPlace => can_append = true,
            StrRegUse::Neutral => {}
        }
    }
    can_append
}

pub(super) fn rewrite_str_concats(
    code: &mut [Instr],
    regs: &[u16],
    state: &mut State<'_, '_>,
) -> u16 {
    let empty_str = Data::comp_str("", &mut state.pools.str_pool);
    let empty_str_reg = state.new_const_reg(empty_str);
    for instr in code {
        *instr = match *instr {
            Instr::AddStr(o1, o2, dst)
                if o1 == dst && o2 == empty_str_reg && regs.contains(&dst) =>
            {
                Instr::Mov(dst, dst)
            }
            Instr::AddStr(o1, o2, dst) if o1 == dst && o2 != dst && regs.contains(&dst) => {
                Instr::AddStrInPlace(dst, o2)
            }
            Instr::Mov(src, dst)
                if src != dst
                    && regs.contains(&dst)
                    && (!state.is_register_const(src)
                        || state.register_value(src).is_large_str()) =>
            {
                Instr::AddStr(src, empty_str_reg, dst)
            }
            i => i,
        };
    }
    empty_str_reg
}

pub(super) fn apply_loop_in_place_str_appends(
    loop_start: usize,
    ctx: Ctx,
    state: &mut State<'_, '_>,
    output: &mut Vec<Instr>,
) {
    let body = &output[loop_start..];
    let globals: &[Variable] = if ctx.in_function
        || body
            .iter()
            .any(|i| matches!(i, Instr::CallFunc { .. } | Instr::CallDynamicLibFunc { .. }))
    {
        &[]
    } else {
        state.globals
    };
    let regs: Vec<u16> = state
        .v
        .iter()
        .chain(globals)
        .filter(|v| v.declared_type == DataType::String)
        .map(|v| v.register_id)
        .filter(|&reg| is_eligible_for_in_place_append(body, reg))
        .collect();
    if regs.is_empty() {
        return;
    }
    let empty_str_reg = rewrite_str_concats(&mut output[loop_start..], &regs, state);
    output.splice(
        loop_start..loop_start,
        regs.iter().map(|&reg| Instr::AddStr(reg, empty_str_reg, reg)),
    );
    state.shift_instr_src(ctx, ctx.offset + loop_start as u32, regs.len() as u32);
}
