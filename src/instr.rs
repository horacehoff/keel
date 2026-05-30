#[derive(Debug, Clone, PartialEq, Copy)]
#[repr(u8)]
pub enum Instr {
    Print(u16),

    // LOGIC
    /// Jumps x instructions forwards
    Jmp(u16),
    /// Jumps x instructions backwards
    JmpBack(u16),
    IsFalseJmp(u16, u16),
    IsTrueJmp(u16, u16),
    SupEqFloatJmp(u16, u16, u16),
    SupEqIntJmp(u16, u16, u16),
    SupFloatJmp(u16, u16, u16),
    SupIntJmp(u16, u16, u16),
    InfEqFloatJmp(u16, u16, u16),
    InfEqIntJmp(u16, u16, u16),
    InfFloatJmp(u16, u16, u16),
    InfIntJmp(u16, u16, u16),
    InfIntJmpBack(u16, u16, u16),
    NotEqJmp(u16, u16, u16),
    EqJmp(u16, u16, u16),
    ArrayNotEqJmp(u16, u16, u16),
    ArrayEqJmp(u16, u16, u16),
    StrNotEqJmp(u16, u16, u16),
    StrEqJmp(u16, u16, u16),

    Mov(u16, u16),
    /// SetInt(dest_reg_id, val)\
    /// Writes val directly into dest_reg_id
    SetInt(u16, i32),
    /// SetBool(dest_reg_id, val)\
    /// Writes val directly into dest_reg_id
    SetBool(u16, bool),

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
    /// Increments the integer in-place by 1
    IncInt(u16),
    /// Decrements the integer in-place by 1
    DecInt(u16),
    /// IncIntTo(src, dst)\
    /// dst = src + 1
    IncIntTo(u16, u16),
    /// DecIntTo(src, dst)\
    /// dst = src - 1
    DecIntTo(u16, u16),
    Eq(u16, u16, u16),
    NotEq(u16, u16, u16),
    ArrayEq(u16, u16, u16),
    ArrayNotEq(u16, u16, u16),
    StrEq(u16, u16, u16),
    StrNotEq(u16, u16, u16),
    SupFloat(u16, u16, u16),
    SupInt(u16, u16, u16),
    SupEqFloat(u16, u16, u16),
    SupEqInt(u16, u16, u16),
    InfFloat(u16, u16, u16),
    InfInt(u16, u16, u16),
    InfEqFloat(u16, u16, u16),
    InfEqInt(u16, u16, u16),
    BoolAnd(u16, u16, u16),
    BoolOr(u16, u16, u16),
    NegBool(u16, u16),
    NegFloat(u16, u16),
    NegInt(u16, u16),

    /// CallFunc(n,y)\
    /// Jumps to the nth instruction, and adds y as a slot to be set by the Return instruction\
    /// VoidReturn/Return will jump back to this location
    CallFunc(u16, u16),
    /// CallFuncRecursive(n, register_id)\
    /// Jumps to the nth instruction, register_id is only used during parsing.
    CallFuncRecursive(u16, u16),
    /// Jumps to the instruction right after the last CallFunc encountered by the interpreter
    VoidReturn,
    /// Return(n)\
    /// Returns the data located in register n
    Return(u16),
    /// RecursiveReturn(n,function_id)\
    /// Returns the data located in register n, and restores the function's register state
    RecursiveReturn(u16),
    /// SaveFrame(function_location, return_register, function_id)
    SaveFrame(u16, u16, u16),

    /// CallDynamicLibFunc(fn_id, dest_register_id)
    CallDynamicLibFunc(u16, u16),

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

    /// ArrayMov(new_elem_reg_id, array_id, idx)\
    /// Replaces the idx-th element in the array (with the id array_id) with the element located in new_elem_reg_id
    ArrayElemMov(u16, u16, u16),

    /// EmptyArray(array_reg_id)
    /// Allocates a fresh empty array and stores its address in array_reg_id
    EmptyArray(u16),

    /// CloneArray(src_reg, dest_reg, len)
    /// Allocates a fresh array with exact capacity len and clones the array in src_reg to dest_reg
    CloneArray(u16, u16, u16),

    /// SetElementArray(array_reg_id, new_elem_reg_id, idx)\
    /// Replaces the idx-th element in the array located in array_reg_id with the element located in new_elem_reg_id
    SetElementArray(u16, u16, u16),

    /// SetElementString(string_reg_id, new_str_reg_id, idx)\
    /// Replaces the idx-th char in the string located in string_reg_id with the string located in new_str_reg_id
    SetElementString(u16, u16, u16),

    /// GetIndexArray(array_reg_id, index_reg_id, output_reg_id)
    GetIndexArray(u16, u16, u16),

    /// GetIndexString(str_reg_id, index_reg_id, output_reg_id)
    GetIndexString(u16, u16, u16),

    /// GetSliceArray(array_reg_id, idx_start_reg_id, output_reg_id)\
    /// idx_end_reg_id is pulled from a StoreFuncArg
    GetSliceArray(u16, u16, u16),
    /// GetSliceString(str_reg_id, idx_start_reg_id, output_reg_id)\
    /// idx_end_reg_id is pulled from a StoreFuncArg
    GetSliceString(u16, u16, u16),

    /// Push(array_reg_id, elem_reg_id)
    Push(u16, u16),

    /// Remove(array_reg_id, elem_index_reg_id)
    Remove(u16, u16),

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
