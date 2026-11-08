const _: () = assert!(size_of::<Instr>() == 8);

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[repr(u8)]
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

    EqIntImmJmp(u16, u16, u16),
    NotEqIntImmJmp(u16, u16, u16),
    InfIntImmJmp(u16, u16, u16),
    SupEqIntImmJmp(u16, u16, u16),

    /// Adds 1 to counter_reg and jumps if counter_reg < limit_reg
    IncIntInfJmp {
        counter_reg: u16,
        limit_reg: u16,
        jump_size: u16,
    },

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
        frame_offset: u16,
        target: u32,
    },
    /// Return(n)\
    /// Returns the data located in register n
    Return(u16),
    /// Copies the absolute register `reg` into `dest_reg`
    LoadGlobal {
        dest_reg: u16,
        reg: u32,
    },
    /// Copies `src_reg` into the absolute register `reg`
    StoreGlobal {
        src_reg: u16,
        reg: u32,
    },

    /// Copies `count` function constants from absolute register `src` into registers `dst..dst + count`.
    CopyConstants {
        dst: u16,
        /// Bits 0..24 are src\
        /// Bits 24..32 are count
        count_src: u32,
    },

    /// CallDynamicLibFunc(fn_id, dest_register_id)
    CallDynamicLibFunc {
        fn_id: u16,
        dst: u16,
        first_arg: u16,
    },

    // LibFunc
    Uppercase(u16, u16),
    Lowercase(u16, u16),
    Contains {
        src: u16,
        needle: u16,
        dst: u16,
    },
    Trim(u16, u16),
    TrimSequence {
        src: u16,
        seq: u16,
        dst: u16,
    },
    Find {
        src: u16,
        needle: u16,
        dst: u16,
    },
    IsFloat(u16, u16),
    IsInt(u16, u16),
    TrimLeft(u16, u16),
    TrimRight(u16, u16),
    TrimSequenceLeft {
        src: u16,
        seq: u16,
        dst: u16,
    },
    TrimSequenceRight {
        src: u16,
        seq: u16,
        dst: u16,
    },
    Repeat {
        src: u16,
        count: u16,
        dst: u16,
    },
    Round(u16, u16),
    Abs(u16, u16),
    Argv(u16),
    SqrtFloat(u16, u16),
    Float(u16, u16),
    Int(u16, u16),
    Str(u16, u16),
    Bool(u16, u16),
    Input(u16, u16),
    Floor(u16, u16),
    TheAnswer(u16),
    Len(u16, u16),
    StartsWith {
        src: u16,
        prefix: u16,
        dst: u16,
    },
    EndsWith {
        src: u16,
        suffix: u16,
        dst: u16,
    },
    Replace(u16, u16, u16),
    Split {
        src: u16,
        sep: u16,
        dst: u16,
    },
    Range(u16, u16, u16),
    JoinStringArray(u16, u16, u16),
    ReverseString(u16, u16),
    FsRead(u16, u16),
    FsExists(u16, u16),

    // LibFuncVoid
    ReverseArray(u16),
    FsWrite(u16, u16),
    FsAppend(u16, u16),
    FsDelete(u16),
    FsDeleteDir(u16),
    Sort(u16),

    /// StartErrorCatch(jump_size, error_register_id)
    StartErrorCatch(u16, u16),
    StopErrorCatch,

    /// ThrowError(error_register_id)
    /// Throws the error (a string) located in error_register_id
    ThrowError(u16),

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

impl Instr {
    /// Returns the discriminant of `instr`
    /// From https://doc.rust-lang.org/reference/items/enumerations.html#r-items.enum.discriminant.access-memory
    #[inline(always)]
    #[must_use]
    pub const fn discriminant(&self) -> u8 {
        unsafe { *core::ptr::from_ref(self).cast() }
    }
}

#[macro_export]
macro_rules! lib_func {
    ($a: pat, $b: pat) => {
        Instr::Uppercase($a, $b)
            | Instr::Lowercase($a, $b)
            | Instr::Trim($a, $b)
            | Instr::IsFloat($a, $b)
            | Instr::IsInt($a, $b)
            | Instr::TrimLeft($a, $b)
            | Instr::TrimRight($a, $b)
            | Instr::Round($a, $b)
            | Instr::Abs($a, $b)
            | Instr::SqrtFloat($a, $b)
            | Instr::Float($a, $b)
            | Instr::Int($a, $b)
            | Instr::Str($a, $b)
            | Instr::Bool($a, $b)
            | Instr::Input($a, $b)
            | Instr::Floor($a, $b)
            | Instr::Len($a, $b)
            | Instr::ReverseString($a, $b)
            | Instr::FsRead($a, $b)
            | Instr::FsExists($a, $b)
    };
}
