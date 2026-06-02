use std::collections::HashMap;

use crate::data::Data;
use crate::data::NULL;
use crate::errors::dev_error;
use crate::instr::Instr;
use crate::instr::LibFuncVoid;
use crate::parser_data::Variable;
use smol_strc::SmolStr;

pub fn move_to_id(x: &mut [Instr], tgt_id: u16) {
    if x.is_empty()
        || matches!(
            x.last().unwrap(),
            Instr::ObjElemMov(_, _, _) // | Instr::IoDelete(_)
            | Instr::IncInt(_)
            | Instr::DecInt(_)
        )
    {
        return;
    }
    let matching_elem_index = x
        .iter()
        .rposition(|w| get_tgt_id(*w).is_some())
        .unwrap_or(x.len() - 1);
    let matching_elem = x.get_mut(matching_elem_index).unwrap();
    match matching_elem {
        Instr::Mov(_, y)
        | Instr::SetInt(y, _)
        | Instr::SetBool(y, _)
        | Instr::CallFunc(_, y)
        | Instr::AddFloat(_, _, y)
        | Instr::AddInt(_, _, y)
        | Instr::AddArray(_, _, y)
        | Instr::AddStr(_, _, y)
        | Instr::MulFloat(_, _, y)
        | Instr::MulInt(_, _, y)
        | Instr::SubFloat(_, _, y)
        | Instr::SubInt(_, _, y)
        | Instr::DivFloat(_, _, y)
        | Instr::DivInt(_, _, y)
        | Instr::ModFloat(_, _, y)
        | Instr::ModInt(_, _, y)
        | Instr::PowFloat(_, _, y)
        | Instr::PowInt(_, _, y)
        | Instr::Eq(_, _, y)
        | Instr::ObjEq(_, _, y)
        | Instr::StrEq(_, _, y)
        | Instr::NotEq(_, _, y)
        | Instr::ObjNotEq(_, _, y)
        | Instr::StrNotEq(_, _, y)
        | Instr::SupFloat(_, _, y)
        | Instr::SupInt(_, _, y)
        | Instr::SupEqFloat(_, _, y)
        | Instr::SupEqInt(_, _, y)
        | Instr::InfFloat(_, _, y)
        | Instr::InfInt(_, _, y)
        | Instr::InfEqFloat(_, _, y)
        | Instr::InfEqInt(_, _, y)
        | Instr::BoolAnd(_, _, y)
        | Instr::BoolOr(_, _, y)
        | Instr::NegBool(_, y)
        | Instr::EmptyArray(y)
        | Instr::NegFloat(_, y)
        | Instr::NegInt(_, y)
        | Instr::CallLibFunc(_, _, y)
        | Instr::GetIndexArray(_, _, y)
        | Instr::GetFieldStruct(_, _, y)
        | Instr::GetSliceArray(_, _, y)
        | Instr::GetIndexString(_, _, y)
        | Instr::GetSliceString(_, _, y)
        | Instr::SaveFrame(_, y, _)
        | Instr::CallDynamicLibFunc(_, y)
        | Instr::IncIntTo(_, y)
        | Instr::DecIntTo(_, y) => *y = tgt_id,
        Instr::CallFuncRecursive(_, y_func) => {
            *y_func = tgt_id;
            for i in 1..x.len() - 1 {
                if let Some(Instr::SaveFrame(_, y_frame, _)) = x.get_mut(matching_elem_index - i) {
                    *y_frame = tgt_id;
                    break;
                }
            }
        }
        other => dev_error(
            "parser.rs",
            "move_to_id",
            format_args!("Tried to move {other:?} to tgt_id={tgt_id}"),
        ),
    }
}

/// Returns the ID of the register that will be modified by the given instruction
pub fn get_tgt_id(x: Instr) -> Option<u16> {
    match x {
        // ↓ INSTRUCTIONS THAT DON'T MODIFY ANY REGISTER ↓
        Instr::Print(_)
        | Instr::Jmp(_)
        | Instr::JmpBack(_)
        | Instr::IsFalseJmp(_, _)
        | Instr::IsTrueJmp(_, _)
        | Instr::NotEqJmp(_, _, _)
        | Instr::ObjNotEqJmp(_, _, _)
        | Instr::StrNotEqJmp(_, _, _)
        | Instr::EqJmp(_, _, _)
        | Instr::ObjEqJmp(_, _, _)
        | Instr::StrEqJmp(_, _, _)
        | Instr::SupFloatJmp(_, _, _)
        | Instr::SupIntJmp(_, _, _)
        | Instr::SupEqFloatJmp(_, _, _)
        | Instr::SupEqIntJmp(_, _, _)
        | Instr::InfEqFloatJmp(_, _, _)
        | Instr::InfEqIntJmp(_, _, _)
        | Instr::InfFloatJmp(_, _, _)
        | Instr::InfIntJmp(_, _, _)
        | Instr::InfIntJmpBack(_, _, _)
        // | Instr::IoDelete(_)
        | Instr::StoreFuncArg(_)
        | Instr::SetElementObj(_, _, _)
        | Instr::SetFieldStruct(_, _, _)
        | Instr::ObjElemMov(_, _, _)
        | Instr::Push(_, _)
        | Instr::Return(_) // Modifies a register, but this function doesn't know which one
        | Instr::RecursiveReturn(_) // Modifies a register, but this function doesn't know which one
        | Instr::VoidReturn
        | Instr::Remove(_, _)
        | Instr::CallLibFuncVoid(_, _, _)
        | Instr::Halt(_)
        | Instr::StopErrorCatch
        | Instr::ThrowError(_)
        => None,

        Instr::StartErrorCatch(_, y) if y == u16::MAX => None,

        Instr::Mov(_, y)
        | Instr::SetInt(y, _)
        | Instr::SetBool(y, _)
        | Instr::CallFunc(_, y)
        | Instr::CallFuncRecursive(_, y)
        | Instr::SaveFrame(_, y, _)
        | Instr::AddFloat(_, _, y)
        | Instr::AddInt(_, _, y)
        | Instr::AddArray(_, _, y)
        | Instr::AddStr(_, _, y)
        | Instr::MulFloat(_, _, y)
        | Instr::MulInt(_, _, y)
        | Instr::SubFloat(_, _, y)
        | Instr::SubInt(_, _, y)
        | Instr::DivFloat(_, _, y)
        | Instr::DivInt(_, _, y)
        | Instr::ModFloat(_, _, y)
        | Instr::ModInt(_, _, y)
        | Instr::PowFloat(_, _, y)
        | Instr::PowInt(_, _, y)
        | Instr::Eq(_, _, y)
        | Instr::ObjEq(_, _, y)
        | Instr::StrEq(_, _, y)
        | Instr::NotEq(_, _, y)
        | Instr::ObjNotEq(_, _, y)
        | Instr::StrNotEq(_, _, y)
        | Instr::SupFloat(_, _, y)
        | Instr::SupInt(_, _, y)
        | Instr::SupEqFloat(_, _, y)
        | Instr::SupEqInt(_, _, y)
        | Instr::InfFloat(_, _, y)
        | Instr::InfInt(_, _, y)
        | Instr::InfEqFloat(_, _, y)
        | Instr::InfEqInt(_, _, y)
        | Instr::BoolAnd(_, _, y)
        | Instr::BoolOr(_, _, y)
        | Instr::NegBool(_, y)
        | Instr::EmptyArray(y)
        | Instr::NegFloat(_, y)
        | Instr::NegInt(_, y)
        | Instr::CallLibFunc(_, _, y)
        | Instr::GetIndexArray(_, _, y)
        | Instr::GetFieldStruct(_, _, y)
        | Instr::GetSliceArray(_, _, y)
        | Instr::GetIndexString(_, _, y)
        | Instr::GetSliceString(_, _, y)
        | Instr::SetElementString(y, _, _)
        | Instr::CallDynamicLibFunc(_, y)
        | Instr::IncInt(y)
        | Instr::DecInt(y)
        | Instr::IncIntTo(_, y)
        | Instr::DecIntTo(_, y)
        | Instr::StartErrorCatch(_,y)
        | Instr::CloneStruct(_, y)
        | Instr::CloneArray(_, y, _) => Some(y),



    }
}

/// Returns the IDs of all the registers which are modified by the given instructions
pub fn get_tgt_ids(x: &[Instr]) -> Vec<u16> {
    let mut ids: Vec<u16> = x.iter().filter_map(|i| get_tgt_id(*i)).collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

pub fn get_last_tgt_id(x: &[Instr]) -> Option<u16> {
    debug_assert!(!(x.is_empty() || matches!(x.last().unwrap(), Instr::ObjElemMov(_, _, _))));
    for y in x.iter().rev() {
        if let Some(id) = get_tgt_id(*y) {
            return Some(id);
        }
    }
    None
}

pub fn alloc_register(registers: &mut Vec<Data>, free_registers: &mut Vec<u16>) -> u16 {
    if let Some(id) = free_registers.pop() {
        id
    } else {
        registers.push(NULL);
        (registers.len() - 1) as u16
    }
}

pub fn free_register(
    id: u16,
    free_registers: &mut Vec<u16>,
    v: &[Variable],
    const_registers: &HashMap<Data, u16>,
) {
    if !v.iter().any(|var| var.register_id == id)
        && !const_registers.values().any(|&reg| reg == id)
        && !free_registers.contains(&id)
    {
        free_registers.push(id);
    }
}

/// Frees registers that are written by instructions in scope_instrs & are not held by a variable & and are not in const_registers.
pub fn free_scope_registers(
    regs_before: u16,
    scope_instrs: &[Instr],
    free_registers: &mut Vec<u16>,
    v: &[Variable],
    const_registers: &HashMap<Data, u16>,
) {
    for id in get_tgt_ids(scope_instrs) {
        if id >= regs_before {
            free_register(id, free_registers, v, const_registers);
        }
    }
}

/// Similar to free_scope_registers, but also frees CloneArray template registers. Only call this after a loop ends.
pub fn free_loop_scope_registers(
    regs_before: u16,
    scope_instrs: &[Instr],
    free_registers: &mut Vec<u16>,
    v: &[Variable],
    const_registers: &HashMap<Data, u16>,
) {
    free_scope_registers(
        regs_before,
        scope_instrs,
        free_registers,
        v,
        const_registers,
    );
    // Free CloneArray template registers
    for instr in scope_instrs {
        if let Instr::CloneArray(template_reg, _, _) = instr
            && *template_reg >= regs_before
        {
            free_register(*template_reg, free_registers, v, const_registers);
        } else if let Instr::CloneStruct(template_reg, _) = instr
            && *template_reg >= regs_before
        {
            free_register(*template_reg, free_registers, v, const_registers);
        }
    }
}

pub fn is_reg_free(v: &[Variable], id: u16, name: &SmolStr) -> bool {
    !v.iter()
        .any(|var| &var.name != name && var.register_id == id)
}

pub fn for_each_read_reg(instr: Instr, mut f: impl FnMut(u16)) {
    match instr {
        Instr::AddFloat(a, b, _)
        | Instr::AddInt(a, b, _)
        | Instr::AddArray(a, b, _)
        | Instr::AddStr(a, b, _)
        | Instr::MulFloat(a, b, _)
        | Instr::MulInt(a, b, _)
        | Instr::SubFloat(a, b, _)
        | Instr::SubInt(a, b, _)
        | Instr::DivFloat(a, b, _)
        | Instr::DivInt(a, b, _)
        | Instr::ModFloat(a, b, _)
        | Instr::ModInt(a, b, _)
        | Instr::PowFloat(a, b, _)
        | Instr::PowInt(a, b, _)
        | Instr::Eq(a, b, _)
        | Instr::NotEq(a, b, _)
        | Instr::ObjEq(a, b, _)
        | Instr::ObjNotEq(a, b, _)
        | Instr::StrEq(a, b, _)
        | Instr::StrNotEq(a, b, _)
        | Instr::SupFloat(a, b, _)
        | Instr::SupInt(a, b, _)
        | Instr::SupEqFloat(a, b, _)
        | Instr::SupEqInt(a, b, _)
        | Instr::InfFloat(a, b, _)
        | Instr::InfInt(a, b, _)
        | Instr::InfEqFloat(a, b, _)
        | Instr::InfEqInt(a, b, _)
        | Instr::BoolAnd(a, b, _)
        | Instr::BoolOr(a, b, _)
        | Instr::GetIndexArray(a, b, _)
        | Instr::GetSliceArray(a, b, _)
        | Instr::GetIndexString(a, b, _)
        | Instr::GetSliceString(a, b, _)
        | Instr::NotEqJmp(a, b, _)
        | Instr::EqJmp(a, b, _)
        | Instr::ObjNotEqJmp(a, b, _)
        | Instr::ObjEqJmp(a, b, _)
        | Instr::StrNotEqJmp(a, b, _)
        | Instr::StrEqJmp(a, b, _)
        | Instr::SupFloatJmp(a, b, _)
        | Instr::SupIntJmp(a, b, _)
        | Instr::SupEqFloatJmp(a, b, _)
        | Instr::SupEqIntJmp(a, b, _)
        | Instr::InfFloatJmp(a, b, _)
        | Instr::InfIntJmp(a, b, _)
        | Instr::InfEqFloatJmp(a, b, _)
        | Instr::InfEqIntJmp(a, b, _)
        | Instr::InfIntJmpBack(a, b, _)
        | Instr::Push(a, b)
        | Instr::SetFieldStruct(a, b, _)
        | Instr::Remove(a, b) => {
            f(a);
            f(b);
        }

        Instr::SetElementObj(a, b, c) | Instr::SetElementString(a, b, c) => {
            f(a);
            f(b);
            f(c);
        }

        Instr::Mov(a, _)
        | Instr::IncInt(a)
        | Instr::DecInt(a)
        | Instr::IncIntTo(a, _)
        | Instr::DecIntTo(a, _)
        | Instr::NegFloat(a, _)
        | Instr::NegInt(a, _)
        | Instr::CallLibFunc(_, a, _)
        | Instr::Print(a)
        | Instr::StoreFuncArg(a)
        | Instr::Return(a)
        | Instr::RecursiveReturn(a)
        | Instr::IsFalseJmp(a, _)
        | Instr::IsTrueJmp(a, _)
        | Instr::ThrowError(a)
        | Instr::GetFieldStruct(a, _, _)
        | Instr::NegBool(a, _) => f(a),

        Instr::ObjElemMov(a, _, _) => f(a),

        Instr::CallLibFuncVoid(func, a, b) => {
            f(a);
            if matches!(func, LibFuncVoid::FsWrite | LibFuncVoid::FsAppend) {
                f(b);
            }
        }
        Instr::Halt(x) if x != 0 => f(x),
        Instr::Halt(_) => {}

        Instr::CloneArray(src, _, _) | Instr::CloneStruct(src, _) => f(src),

        Instr::Jmp(_)
        | Instr::JmpBack(_)
        | Instr::VoidReturn
        | Instr::CallFunc(_, _)
        | Instr::CallFuncRecursive(_, _)
        | Instr::SaveFrame(_, _, _)
        | Instr::CallDynamicLibFunc(_, _)
        | Instr::EmptyArray(_)
        | Instr::SetInt(_, _)
        | Instr::StartErrorCatch(_, _)
        | Instr::StopErrorCatch
        | Instr::SetBool(_, _) => {}
    }
}

/// Write v, located in the src_id register, into the dest_id register using the cheapest instruction
#[inline(always)]
pub fn move_reg_to_reg(output: &mut Vec<Instr>, src_id: u16, dest_id: u16, v: Data) {
    if v.is_int() {
        output.push(Instr::SetInt(dest_id, v.as_int()));
    } else if v.is_bool() {
        output.push(Instr::SetBool(dest_id, v.as_bool()));
    } else {
        output.push(Instr::Mov(src_id, dest_id));
    }
}
