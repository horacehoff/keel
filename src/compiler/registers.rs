use crate::data::Data;
use crate::instr::Instr;
use crate::instr::LibFuncVoid;
use core::hint::unreachable_unchecked;

pub fn move_value_to(output: &mut Vec<Instr>, src_instr_idx: usize, value_idx: u16, tgt_idx: u16) {
    if value_idx != tgt_idx {
        let instrs = &mut output[src_instr_idx..];
        if instrs.last().and_then(|instr| instr.get_tgt_id()) == Some(value_idx) {
            move_to_id(instrs, tgt_idx);
            if instrs.last().and_then(|instr| instr.get_tgt_id()) == Some(tgt_idx) {
                return;
            }
        }
        output.push(Instr::Mov(value_idx, tgt_idx));
    }
}

pub fn move_to_id(x: &mut [Instr], tgt_id: u16) {
    if x.is_empty()
        || matches!(
            x.last().unwrap(),
            Instr::SetFieldObj(_, _, _)
                | Instr::SetElementString(_, _, _)
                | Instr::StartErrorCatch(_, _)
        )
        || matches!(x.last().unwrap(), Instr::AddIntImm { src, dst, .. } if src==dst)
    {
        return;
    }
    let matching_elem_index =
        x.iter().rposition(|w| w.get_tgt_id().is_some()).unwrap_or(x.len() - 1);
    let matching_elem = x.get_mut(matching_elem_index).unwrap();
    match matching_elem {
        Instr::Mov(_, id)
        | Instr::SetInt(id, _)
        | Instr::AddFloat(_, _, id)
        | Instr::AddInt(_, _, id)
        | Instr::AddArray(_, _, id)
        | Instr::AddStr(_, _, id)
        | Instr::MulFloat(_, _, id)
        | Instr::MulInt(_, _, id)
        | Instr::SubFloat(_, _, id)
        | Instr::SubInt(_, _, id)
        | Instr::DivFloat(_, _, id)
        | Instr::DivInt(_, _, id)
        | Instr::ModFloat(_, _, id)
        | Instr::ModInt(_, _, id)
        | Instr::PowFloat(_, _, id)
        | Instr::PowInt(_, _, id)
        | Instr::Eq(_, _, id)
        | Instr::ObjEq(_, _, id)
        | Instr::NotEq(_, _, id)
        | Instr::ObjNotEq(_, _, id)
        | Instr::InfFloat(_, _, id)
        | Instr::InfInt(_, _, id)
        | Instr::InfEqFloat(_, _, id)
        | Instr::InfEqInt(_, _, id)
        | Instr::BoolAnd(_, _, id)
        | Instr::BoolOr(_, _, id)
        | Instr::AddIntImm { dst: id, .. }
        | Instr::EmptyArray(id)
        | Instr::CallLibFunc(_, _, id)
        | Instr::GetIndexArray(_, _, id)
        | Instr::GetFieldStruct(_, _, id)
        | Instr::GetSliceArray(_, _, id)
        | Instr::GetIndexString(_, _, id)
        | Instr::GetSliceString(_, _, id)
        | Instr::CallDynamicLibFunc { dest_reg_id: id, .. }
        | Instr::LoadGlobal { dest_reg: id, .. }
        | Instr::MapGet(_, _, id)
        | Instr::IsType(_, _, id)
        | Instr::CloneObj(_, id, _)
        | Instr::MapRemove { dest_reg_id: id, .. }
        | Instr::CloneMap(_, id) => *id = tgt_id,
        Instr::CallFunc { .. } => {}
        _ => unsafe { unreachable_unchecked() },
    }
}

impl Instr {
    /// Returns the ID of the register that will be modified by the given instruction
    #[must_use]
    pub const fn get_tgt_id(self) -> Option<u16> {
        match self {
            // ↓ INSTRUCTIONS THAT DON'T MODIFY ANY REGISTER ↓
            Self::Print(_)
            | Self::Jmp(_)
            | Self::NotEqJmp(_, _, _)
            | Self::ObjNotEqJmp(_, _, _)
            | Self::IsNotTypeJmp {..}
            | Self::EqJmp(_, _, _)
            | Self::ObjEqJmp(_, _, _)
            | Self::InfEqFloatJmp(_, _, _)
            | Self::InfEqIntJmp(_, _, _)
            | Self::InfFloatJmp(_, _, _)
            | Self::InfIntJmp(_, _, _)
            | Self::NotInfEqFloatJmp(..)
            | Self::NotInfFloatJmp(..)
            | Self::EqIntImmJmp(..)
            | Self::NotEqIntImmJmp(..)
            | Self::SupEqIntImmJmp(..)
            | Self::InfIntImmJmp(..)
            | Self::IncIntInfJmp {..}
            | Self::StoreFuncArg(_)
            | Self::SetElementObj(_, _, _)
            | Self::SetFieldObj(_, _, _)
            | Self::MapInsertReg(_, _, _)
            | Self::Push(_, _)
            | Self::Return(_) // Modifies a register, but this function doesn't know which one
            | Self::VoidReturn
            | Self::StoreGlobal { .. }
            | Self::CopyConstants { .. }
            | Self::Remove(_, _)
            | Self::CallLibFuncVoid(_, _, _)
            | Self::Halt(_)
            | Self::StopErrorCatch
            | Self::ThrowError(_)
            => None,

            Self::StartErrorCatch(_, y) if y == u16::MAX => None,

            Self::Mov(_, y)
            | Self::SetInt(y, _)
            | Self::AddIntImm {dst: y, .. }
            | Self::CallFunc { frame_offset: y, .. }
            | Self::LoadGlobal { dest_reg: y, .. }
            | Self::AddFloat(_, _, y)
            | Self::AddInt(_, _, y)
            | Self::AddArray(_, _, y)
            | Self::AddStr(_, _, y)
            | Self::MulFloat(_, _, y)
            | Self::MulInt(_, _, y)
            | Self::SubFloat(_, _, y)
            | Self::SubInt(_, _, y)
            | Self::DivFloat(_, _, y)
            | Self::DivInt(_, _, y)
            | Self::ModFloat(_, _, y)
            | Self::ModInt(_, _, y)
            | Self::PowFloat(_, _, y)
            | Self::PowInt(_, _, y)
            | Self::Eq(_, _, y)
            | Self::ObjEq(_, _, y)
            | Self::NotEq(_, _, y)
            | Self::ObjNotEq(_, _, y)
            | Self::InfFloat(_, _, y)
            | Self::InfInt(_, _, y)
            | Self::InfEqFloat(_, _, y)
            | Self::InfEqInt(_, _, y)
            | Self::BoolAnd(_, _, y)
            | Self::BoolOr(_, _, y)
            | Self::EmptyArray(y)
            | Self::CallLibFunc(_, _, y)
            | Self::IsType(_, _, y)
            | Self::GetIndexArray(_, _, y)
            | Self::GetFieldStruct(_, _, y)
            | Self::MapGet(_, _, y)
            | Self::MapRemove { dest_reg_id: y, .. }
            | Self::GetSliceArray(_, _, y)
            | Self::GetIndexString(_, _, y)
            | Self::GetSliceString(_, _, y)
            | Self::SetElementString(y, _, _)
            | Self::CallDynamicLibFunc {dest_reg_id: y,..}
            | Self::StartErrorCatch(_,y)
            | Self::CloneMap(_, y)
            | Self::CloneObj(_, y, _) => Some(y),
        }
    }
    pub fn for_each_reg_mut(&mut self, mut f: impl FnMut(&mut u16)) {
        match self {
            Self::InfEqFloatJmp(a, b, _)
            | Self::InfEqIntJmp(a, b, _)
            | Self::InfFloatJmp(a, b, _)
            | Self::InfIntJmp(a, b, _)
            | Self::NotInfFloatJmp(a, b, _)
            | Self::NotInfEqFloatJmp(a, b, _)
            | Self::NotEqJmp(a, b, _)
            | Self::EqJmp(a, b, _)
            | Self::ObjNotEqJmp(a, b, _)
            | Self::ObjEqJmp(a, b, _)
            | Self::Mov(a, b)
            | Self::AddIntImm { src: a, dst: b, .. }
            | Self::IncIntInfJmp { counter_reg: a, limit_reg: b, .. }
            | Self::CloneObj(a, b, _)
            | Self::SetFieldObj(a, b, _)
            | Self::GetFieldStruct(a, _, b)
            | Self::Push(a, b)
            | Self::Remove(a, b)
            | Self::CloneMap(a, b)
            | Self::IsType(a, _, b)
            | Self::CallLibFunc(_, a, b) => {
                f(a);
                f(b);
            }
            Self::AddFloat(a, b, c)
            | Self::AddInt(a, b, c)
            | Self::AddArray(a, b, c)
            | Self::AddStr(a, b, c)
            | Self::MulFloat(a, b, c)
            | Self::MulInt(a, b, c)
            | Self::SubFloat(a, b, c)
            | Self::SubInt(a, b, c)
            | Self::DivFloat(a, b, c)
            | Self::DivInt(a, b, c)
            | Self::ModFloat(a, b, c)
            | Self::ModInt(a, b, c)
            | Self::PowFloat(a, b, c)
            | Self::PowInt(a, b, c)
            | Self::Eq(a, b, c)
            | Self::NotEq(a, b, c)
            | Self::ObjEq(a, b, c)
            | Self::ObjNotEq(a, b, c)
            | Self::InfFloat(a, b, c)
            | Self::InfInt(a, b, c)
            | Self::InfEqFloat(a, b, c)
            | Self::InfEqInt(a, b, c)
            | Self::BoolAnd(a, b, c)
            | Self::BoolOr(a, b, c)
            | Self::SetElementObj(a, b, c)
            | Self::SetElementString(a, b, c)
            | Self::GetIndexArray(a, b, c)
            | Self::GetIndexString(a, b, c)
            | Self::GetSliceArray(a, b, c)
            | Self::GetSliceString(a, b, c)
            | Self::MapGet(a, b, c)
            | Self::MapInsertReg(a, b, c)
            | Self::MapRemove { map_reg_id: a, key_reg_id: b, dest_reg_id: c } => {
                f(a);
                f(b);
                f(c);
            }
            Self::Print(a)
            | Self::SetInt(a, _)
            | Self::CallFunc { frame_offset: a, .. }
            | Self::Return(a)
            | Self::LoadGlobal { dest_reg: a, .. }
            | Self::StoreGlobal { src_reg: a, .. }
            | Self::CopyConstants { dst: a, .. }
            | Self::StoreFuncArg(a)
            | Self::ThrowError(a)
            | Self::EmptyArray(a)
            | Self::EqIntImmJmp(a, _, _)
            | Self::NotEqIntImmJmp(a, _, _)
            | Self::InfIntImmJmp(a, _, _)
            | Self::SupEqIntImmJmp(a, _, _)
            | Self::IsNotTypeJmp { src_reg_id: a, .. } => f(a),
            Self::CallDynamicLibFunc { dest_reg_id, last_arg_reg_id, .. } => {
                f(dest_reg_id);
                if *last_arg_reg_id != u16::MAX {
                    f(last_arg_reg_id);
                }
            }
            Self::CallLibFuncVoid(func, a, b) => {
                f(a);
                if matches!(func, LibFuncVoid::FsWrite | LibFuncVoid::FsAppend) {
                    f(b);
                }
            }
            Self::StartErrorCatch(_, a) | Self::Halt(a) => {
                if *a != u16::MAX {
                    f(a);
                }
            }
            Self::Jmp(_) | Self::VoidReturn | Self::StopErrorCatch => {}
        }
    }
}

/// Write v, located in the src_id register, into the dest_id register using the cheapest instruction
#[inline(always)]
pub fn move_reg_to_reg(output: &mut Vec<Instr>, src_id: u16, dest_id: u16, v: Data) {
    if v.is_int() {
        output.push(Instr::SetInt(dest_id, v.as_int()));
    } else if src_id != dest_id {
        output.push(Instr::Mov(src_id, dest_id));
    }
}

pub const PERMANENT_REG_FLAG: u16 = 1 << 15;

#[inline(always)]
#[must_use]
pub const fn is_permanent_register(id: u16) -> bool {
    id >= PERMANENT_REG_FLAG && id != u16::MAX
}

#[must_use]
pub const fn relocate_register(id: u16, fixed_count: u16, permanent_count: u16) -> u16 {
    if is_permanent_register(id) {
        fixed_count + (id - PERMANENT_REG_FLAG)
    } else if id >= fixed_count {
        id + permanent_count
    } else {
        id
    }
}

pub fn relocate_registers(instructions: &mut [Instr], fixed_count: u16, permanent_count: u16) {
    for instr in instructions {
        instr.for_each_reg_mut(|id| *id = relocate_register(*id, fixed_count, permanent_count));
    }
}
