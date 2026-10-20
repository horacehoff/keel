use fixedbitset::FixedBitSet;

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
        | Instr::CallFunc(_, id)
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
        | Instr::SaveFrame(_, id, _)
        | Instr::CallDynamicLibFunc { dest_reg_id: id, .. }
        | Instr::MapGet(_, _, id)
        | Instr::IsType(_, _, id)
        | Instr::CallFuncDynamic(_, id)
        | Instr::CloneObj(_, id, _)
        | Instr::MapRemove { dest_reg_id: id, .. }
        | Instr::CloneMap(_, id) => *id = tgt_id,
        Instr::CallFuncRecursive(_, y_func) => {
            *y_func = tgt_id;
            for i in 1..x.len() - 1 {
                if let Some(Instr::SaveFrame(_, y_frame, _)) = x.get_mut(matching_elem_index - i) {
                    *y_frame = tgt_id;
                    break;
                }
            }
        }
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
            | Self::StoreFuncArg(_)
            | Self::SetElementObj(_, _, _)
            | Self::SetFieldObj(_, _, _)
            | Self::MapInsertReg(_, _, _)
            | Self::Push(_, _)
            | Self::Return(_) // Modifies a register, but this function doesn't know which one
            | Self::RecursiveReturn(_) // Modifies a register, but this function doesn't know which one
            | Self::VoidReturn
            | Self::RecursiveVoidReturn
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
            | Self::CallFunc(_, y)
            | Self::CallFuncDynamic(_, y)
            | Self::CallFuncRecursive(_, y)
            | Self::SaveFrame(_, y, _)
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

    pub fn for_each_read_reg(self, mut f: impl FnMut(u16)) {
        match self {
            Self::AddFloat(a, b, _)
            | Self::AddInt(a, b, _)
            | Self::AddArray(a, b, _)
            | Self::AddStr(a, b, _)
            | Self::MulFloat(a, b, _)
            | Self::MulInt(a, b, _)
            | Self::SubFloat(a, b, _)
            | Self::SubInt(a, b, _)
            | Self::DivFloat(a, b, _)
            | Self::DivInt(a, b, _)
            | Self::ModFloat(a, b, _)
            | Self::ModInt(a, b, _)
            | Self::PowFloat(a, b, _)
            | Self::PowInt(a, b, _)
            | Self::Eq(a, b, _)
            | Self::NotEq(a, b, _)
            | Self::ObjEq(a, b, _)
            | Self::ObjNotEq(a, b, _)
            | Self::NotInfFloatJmp(a, b, _)
            | Self::NotInfEqFloatJmp(a, b, _)
            | Self::InfFloat(a, b, _)
            | Self::InfInt(a, b, _)
            | Self::InfEqFloat(a, b, _)
            | Self::InfEqInt(a, b, _)
            | Self::BoolAnd(a, b, _)
            | Self::BoolOr(a, b, _)
            | Self::GetIndexArray(a, b, _)
            | Self::GetSliceArray(a, b, _)
            | Self::GetIndexString(a, b, _)
            | Self::GetSliceString(a, b, _)
            | Self::NotEqJmp(a, b, _)
            | Self::EqJmp(a, b, _)
            | Self::ObjNotEqJmp(a, b, _)
            | Self::ObjEqJmp(a, b, _)
            | Self::InfFloatJmp(a, b, _)
            | Self::InfIntJmp(a, b, _)
            | Self::InfEqFloatJmp(a, b, _)
            | Self::InfEqIntJmp(a, b, _)
            | Self::Push(a, b)
            | Self::SetFieldObj(a, b, _)
            | Self::MapGet(a, b, _)
            | Self::MapRemove { map_reg_id: a, key_reg_id: b, .. }
            | Self::Remove(a, b) => {
                f(a);
                f(b);
            }

            Self::SetElementObj(a, b, c)
            | Self::SetElementString(a, b, c)
            | Self::MapInsertReg(a, b, c) => {
                f(a);
                f(b);
                f(c);
            }

            Self::Mov(a, _)
            | Self::CallLibFunc(_, a, _)
            | Self::IsType(a, _, _)
            | Self::IsNotTypeJmp { src_reg_id: a, .. }
            | Self::Print(a)
            | Self::StoreFuncArg(a)
            | Self::Return(a)
            | Self::RecursiveReturn(a)
            | Self::ThrowError(a)
            | Self::GetFieldStruct(a, _, _)
            | Self::AddIntImm { src: a, .. }
            | Self::CallFuncDynamic(a, _) => f(a),

            Self::CallDynamicLibFunc { last_arg_reg_id, .. } if last_arg_reg_id == u16::MAX => {}
            Self::CallDynamicLibFunc { last_arg_reg_id, .. } => f(last_arg_reg_id),

            Self::CallLibFuncVoid(func, a, b) => {
                f(a);
                if matches!(func, LibFuncVoid::FsWrite | LibFuncVoid::FsAppend) {
                    f(b);
                }
            }
            Self::Halt(x) if x != u16::MAX => f(x),

            Self::CloneObj(src, _, _) | Self::CloneMap(src, _) => {
                f(src);
            }

            Self::Halt(_)
            | Self::Jmp(_)
            | Self::VoidReturn
            | Self::RecursiveVoidReturn
            | Self::CallFunc(_, _)
            | Self::CallFuncRecursive(_, _)
            | Self::SaveFrame(_, _, _)
            | Self::EmptyArray(_)
            | Self::SetInt(_, _)
            | Self::StartErrorCatch(_, _)
            | Self::StopErrorCatch => {}
        }
    }
}

#[must_use]
pub fn get_tgt_ids(instructions: &[Instr], registers_len: usize) -> FixedBitSet {
    let mut written_regs = FixedBitSet::with_capacity(registers_len);
    for instr in instructions {
        if let Some(id) = instr.get_tgt_id() {
            unsafe { written_regs.insert_unchecked(id as usize) }
        }
    }
    written_regs
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
