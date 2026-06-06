use crate::data::Data;
use crate::expr::Expr;
use crate::expr::Span;
use crate::instr::Instr;
use crate::parser::Namespace;
use crate::type_system::DataType;
use crate::vm::ObjectPool;
use crate::vm::StringPool;
use ahash::RandomState;
#[cfg(not(target_arch = "wasm32"))]
use libloading::Library;
use smol_strc::SmolStr;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

#[derive(Debug)]
pub struct ErrorCatch {
    pub catch_loc: u32,
    pub error_reg: u16,
    pub call_frames_len: u32,
    pub args_len: u32,
}

#[derive(Debug)]
pub struct Function {
    pub name: SmolStr,
    pub args: Box<[SmolStr]>,
    pub code: Rc<[Expr]>,
    pub impls: Vec<FunctionImpl>,
    pub is_recursive: Option<bool>,
    pub returns_void: bool,
    pub src_file: u16,
    /// Cache of return types from track_returns, keyed by Box<arg types>
    pub return_type_cache: Vec<(Box<[DataType]>, DataType)>,
    pub direct_calls: Box<[SmolStr]>,
}

#[derive(Debug)]
pub struct FunctionImpl {
    pub loc: u16,
    pub args_loc: Box<[u16]>,
    pub arg_types: Box<[DataType]>,
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
    /// [ return_type, arg_types... ]
    pub types: Box<[DataType]>,
    #[cfg(not(target_arch = "wasm32"))]
    pub _lib: Library,
    #[cfg(not(target_arch = "wasm32"))]
    pub ptr: libffi::middle::CodePtr,
    #[cfg(not(target_arch = "wasm32"))]
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

#[derive(Debug)]
pub struct Struct {
    pub name: SmolStr,
    pub fields: Box<[(SmolStr, DataType)]>,
    pub id: u16,
}

pub struct Pools {
    pub obj_pool: ObjectPool,
    pub string_pool: StringPool,
}

#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub block_id: u16,
    pub src: (&'a str, &'a str),
    pub is_parsing_recursive: bool,
    pub current_src_file: u16,
}

pub struct State<'a> {
    pub registers: &'a mut Vec<Data>,
    pub fns: &'a mut Vec<Function>,
    pub structs: &'a mut Vec<Struct>,
    pub struct_fields: &'a mut Vec<(SmolStr, Vec<SmolStr>)>,
    pub pools: &'a mut Pools,
    /// Vec<(instruction, markers, file_id)>
    pub instr_src: &'a mut Vec<(Instr, Span, u16)>,
    pub fn_registers: &'a mut Vec<Vec<u16>>,
    pub dyn_libs: &'a mut Vec<Dynamiclib>,
    pub allocated_arg_count: &'a mut usize,
    pub allocated_call_depth: &'a mut usize,
    pub const_registers: &'a mut HashMap<Data, u16, RandomState>,
    pub free_registers: &'a mut Vec<u16>,
    pub sources: &'a mut Vec<(SmolStr, Rc<String>)>,
    pub reserved_registers: HashSet<u16, RandomState>,
    pub namespace: &'a mut Namespace,
}

#[derive(Debug)]
pub struct Variable {
    pub name: SmolStr,
    pub register_id: u16,
    pub var_type: DataType,
}
