use crate::Data;
use crate::Instr;
use crate::expr::Expr;
use crate::type_system::DataType;
use crate::vm::ArrayPool;
use crate::vm::StringPool;
use libloading::Library;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct Function {
    pub name: SmolStr,
    pub args: Box<[SmolStr]>,
    pub code: Rc<[Expr]>,
    pub impls: Vec<FunctionImpl>,
    pub is_recursive: bool,
    pub id: u16,
    pub returns_void: bool,
    /// Index into the sources table for error reporting
    pub src_file: u16,
    /// Cache of return types from track_returns, keyed by arg types
    pub return_type_cache: Vec<(Box<[DataType]>, DataType)>,
}

#[derive(Debug, Clone)]
pub struct FunctionImpl {
    pub loc: u16,
    pub args_loc: Box<[u16]>,
    pub arg_types: Box<[DataType]>,
    pub return_type: DataType,
}

#[derive(Debug)]
pub struct FnSignature {
    pub name: SmolStr,
    pub args: Box<[DataType]>,
    pub return_type: DataType,
    pub id: u16,
}

#[derive(Debug)]
pub struct Dynamiclib {
    pub name: SmolStr,
    pub fns: Box<[FnSignature]>,
}

#[derive(Debug)]
pub struct DynamicLibFn {
    // [ return_type, arg_types... ]
    pub types: Box<[DataType]>,
    pub _lib: Library,
    pub ptr: libffi::middle::CodePtr,
    pub cif: libffi::middle::Cif,
}

impl DynamicLibFn {
    #[inline(always)]
    pub fn get_return_type(&self) -> &DataType {
        &self.types[0]
    }
    #[inline(always)]
    pub fn get_nth_arg_type(&self, idx: usize) -> &DataType {
        &self.types[1 + idx]
    }
}

#[derive(Clone)]
pub struct Pools {
    pub array_pool: ArrayPool,
    pub string_pool: StringPool,
}

#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub block_id: u16,
    pub src: (&'a str, &'a str),
    pub is_parsing_recursive: bool,
    pub current_src_file: u16,
}

/// Shared mutable compilation state. Passed as `&mut State` — no unsafe needed.
pub struct State<'a> {
    pub registers: &'a mut Vec<Data>,
    pub fns: &'a mut Vec<Function>,
    pub pools: &'a mut Pools,
    /// Vec<(instruction, markers, file_id)>
    pub instr_src: &'a mut Vec<(Instr, (usize, usize), u16)>,
    pub fn_registers: &'a mut Vec<Vec<u16>>,
    pub dyn_libs: &'a mut Vec<Dynamiclib>,
    pub allocated_arg_count: &'a mut usize,
    pub allocated_call_depth: &'a mut usize,
    pub const_registers: &'a mut HashMap<Data, u16>,
    pub free_registers: &'a mut Vec<u16>,
    pub sources: &'a mut Vec<(SmolStr, String)>,
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: SmolStr,
    pub register_id: u16,
    pub infered_type: DataType,
}
