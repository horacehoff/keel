// The jump value is stored using offset binary representation
// In this case, the bias K = 65536 (u16) / 2 = 32768
pub const JMP_OFFSET: u16 = 32768;

impl Instr {
    /// Use this instead of Instr::Jmp!!
    #[must_use]
    #[inline(always)]
    pub const fn jump(offset: u16) -> Self {
        Self::Jmp(offset.wrapping_add(JMP_OFFSET))
    }
}

const _: () = debug_assert!(size_of::<Instr>() == 8);

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Instr {
    Print(u16),

    // LOGIC
    /// Jumps by x instructions
    Jmp(u16),
    InfEqFloatJmp(u16, u16, u16),
    InfEqIntJmp(u16, u16, u16),
    InfFloatJmp(u16, u16, u16),
    InfIntJmp(u16, u16, u16),
    NotInfFloatJmp(u16, u16, u16),
    NotInfEqFloatJmp(u16, u16, u16),
    NotEqJmp(u16, u16, u16),
    EqJmp(u16, u16, u16),
    ObjNotEqJmp(u16, u16, u16),
    ObjEqJmp(u16, u16, u16),

    Mov(u16, u16),
    /// SetInt(dest_reg_id, val)\
    /// Writes val directly into dest_reg_id
    SetInt(u16, i32),

    // OPS
    AddFloat(u16, u16, u16),
    AddInt(u16, u16, u16),
    AddArray(u16, u16, u16),
    AddStr(u16, u16, u16),
    MulFloat(u16, u16, u16),
    MulInt(u16, u16, u16),
    SubFloat(u16, u16, u16),
    SubInt(u16, u16, u16),
    DivFloat(u16, u16, u16),
    DivInt(u16, u16, u16),
    ModFloat(u16, u16, u16),
    ModInt(u16, u16, u16),
    PowFloat(u16, u16, u16),
    PowInt(u16, u16, u16),
    /// `imm` is an i16, wraps.
    AddIntImm {
        src: u16,
        dst: u16,
        imm: u16,
    },
    Eq(u16, u16, u16),
    NotEq(u16, u16, u16),
    ObjEq(u16, u16, u16),
    ObjNotEq(u16, u16, u16),
    InfFloat(u16, u16, u16),
    InfInt(u16, u16, u16),
    InfEqFloat(u16, u16, u16),
    InfEqInt(u16, u16, u16),
    BoolAnd(u16, u16, u16),
    BoolOr(u16, u16, u16),

    CallFunc {
        target: u32,
        return_reg: u16,
    },
    CallFuncRecursive {
        target: u32,
        return_reg: u16,
    },
    /// Jumps to the instruction right after the last CallFunc encountered by the interpreter
    VoidReturn,
    RecursiveVoidReturn,
    /// Return(n)\
    /// Returns the data located in register n
    Return(u16),
    /// RecursiveReturn(n,function_id)\
    /// Returns the data located in register n, and restores the function's register state
    RecursiveReturn(u16),
    /// SaveFrame(function_location, return_register, function_id)
    SaveFrame(u16, u16, u16),

    /// CallDynamicLibFunc(fn_id, dest_register_id)
    CallDynamicLibFunc {
        fn_id: u16,
        dest_reg_id: u16,
        last_arg_reg_id: u16,
    },

    StoreFuncArg(u16),
    /// CallLibFunc(function, src_register_id, dest_register_id)
    CallLibFunc(LibFunc, u16, u16),
    /// CallLibFuncVoid(function, src_register_1, src_register_2)
    /// For single-source ops, src register 2 is unused
    CallLibFuncVoid(LibFuncVoid, u16, u16),

    /// StartErrorCatch(jump_size, error_register_id)
    StartErrorCatch(u16, u16),
    StopErrorCatch,

    /// ThrowError(error_register_id)
    /// Throws the error (a string) located in error_register_id
    ThrowError(u16),

    /// CallFuncDynamic(fn_reg_id, return_reg_id)
    CallFuncDynamic(u16, u16),

    /// EmptyArray(array_reg_id)
    /// Allocates a fresh empty array and stores its address in array_reg_id
    EmptyArray(u16),

    /// CloneObj(src_reg, dest_reg, len)
    /// Allocates a fresh array with exact capacity len and clones the array in src_reg to dest_reg
    CloneObj(u16, u16, u16),

    /// SetElementObj(array_reg_id, new_elem_reg_id, idx)\
    /// Replaces the idx-th element in the array located in array_reg_id with the element located in new_elem_reg_id
    SetElementObj(u16, u16, u16),

    /// SetElementString(string_reg_id, new_str_reg_id, idx)\
    /// Replaces the idx-th char in the string located in string_reg_id with the string located in new_str_reg_id
    SetElementString(u16, u16, u16),

    /// SetFieldObj(struct_reg_id, new_elem_reg_id, idx)
    SetFieldObj(u16, u16, u16),

    /// GetIndexArray(array_reg_id, index_reg_id, output_reg_id)
    GetIndexArray(u16, u16, u16),

    /// GetIndexString(str_reg_id, index_reg_id, output_reg_id)
    GetIndexString(u16, u16, u16),

    /// GetFieldStruct(struct_reg_id, index, output_reg_id)
    GetFieldStruct(u16, u16, u16),

    /// GetSliceArray(array_reg_id, idx_start_reg_id, output_reg_id)\
    /// idx_end_reg_id is pulled from a StoreFuncArg
    GetSliceArray(u16, u16, u16),
    /// GetSliceString(str_reg_id, idx_start_reg_id, output_reg_id)\
    /// idx_end_reg_id is pulled from a StoreFuncArg
    GetSliceString(u16, u16, u16),

    /// Push(array_reg_id, elem_reg_id)
    /// Pushes `r[elem_reg_id]` to `obj_pool[r[array_reg_id]]`
    Push(u16, u16),

    /// Remove(array_reg_id, elem_index_reg_id)
    Remove(u16, u16),

    /// MapGet(map_pool_id, key_reg_id, dest_reg_id)
    MapGet(u16, u16, u16),
    MapInsertReg(u16, u16, u16),
    MapRemove {
        map_reg_id: u16,
        key_reg_id: u16,
        dest_reg_id: u16,
    },
    CloneMap(u16, u16),

    /// IsType(src_reg_id, type_idx, dest_reg_id)
    IsType(u16, u16, u16),
    IsNotTypeJmp {
        src_reg_id: u16,
        type_idx: u16,
        jump_size: u16,
    },

    /// Exits the program with the i32 code if it's != 0
    Halt(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibFunc {
    Uppercase = 0,
    Lowercase = 1,
    Contains = 2,
    Trim = 3,
    TrimSequence = 4,
    Find = 5,
    IsFloat = 6,
    IsInt = 7,
    TrimLeft = 8,
    TrimRight = 9,
    TrimSequenceLeft = 10,
    TrimSequenceRight = 11,
    Repeat = 12,
    Round = 13,
    Abs = 14,
    Argv = 15,
    SqrtFloat = 16,
    Float = 17,
    Int = 18,
    Str = 19,
    Bool = 20,
    Input = 21,
    Floor = 22,
    TheAnswer = 23,
    Len = 24,
    StartsWith = 25,
    EndsWith = 26,
    Replace = 27,
    Split = 28,
    Range = 29,
    JoinStringArray = 30,
    Reverse = 31,
    FsRead = 32,
    FsExists = 33,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibFuncVoid {
    Reverse = 0,
    FsWrite = 1,
    FsAppend = 2,
    FsDelete = 3,
    FsDeleteDir = 4,
    Sort = 5,
}
