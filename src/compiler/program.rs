use super::expr::Expr;
use super::expr::Span;
use super::type_system::DataType;
use crate::compiler::expr::QualifiedName;
use crate::compiler::type_system::VmType;
use crate::data::Data;
use crate::errors::ErrorCtx;
use crate::instr::Instr;
use crate::vm::MapPool;
use crate::vm::ObjectPool;
use crate::vm::StringPool;
use std::rc::Rc;

#[cfg(not(target_arch = "wasm32"))]
use libloading::Library;

pub struct ErrorCatch {
    pub catch_loc: u32,
    pub frame_ptr: *mut Data,
    pub error_reg: u16,
}

pub struct InstrSrc {
    pub instr_idx: u32,
    pub span: Span,
    pub file_id: u16,
}

/// One compiled function specialization
#[derive(Default)]
pub struct CodeUnit {
    pub instructions: Vec<Instr>,
    pub instr_src: Vec<InstrSrc>, // relative to the start of the unit
}

pub struct Function<'arena> {
    pub name: &'arena str,
    pub args: Box<[(&'arena str, Option<DataType>)]>,
    pub code: &'arena [Expr<'arena>],
    pub impls: Vec<FunctionImpl>,
    pub returns_null: bool,
    pub src_file_idx: u16,
    /// Cache of return types from track_returns, keyed by Box<arg types>
    pub return_type_cache: Vec<(Box<[DataType]>, DataType)>,
    pub direct_calls: &'arena [QualifiedName<'arena>],
    pub name_span: Span,
}

pub struct FunctionImpl {
    pub unit: u32,
    pub arg_types: Box<[DataType]>,
}

pub struct FnSignature<'arena> {
    pub name: &'arena str,
    pub args: Box<[DataType]>,
    pub return_type: DataType,
    pub id: u16,
}

pub struct Dylib<'arena> {
    pub name: &'arena str,
    pub fns: Box<[FnSignature<'arena>]>,
}

pub struct DylibFn {
    pub args_len: usize,
    /// [ return_type, arg_types... ]
    pub types: Box<[VmType]>,
    #[cfg(not(target_arch = "wasm32"))]
    pub _lib: Rc<Library>,
    #[cfg(not(target_arch = "wasm32"))]
    pub ptr: libffi::middle::CodePtr,
    #[cfg(not(target_arch = "wasm32"))]
    pub cif: libffi::middle::Cif,
}

impl DylibFn {
    #[inline(always)]
    #[must_use]
    pub fn get_argument_type(&self, index: usize) -> VmType {
        unsafe { *self.types.get_unchecked(index + 1) }
    }
    #[inline(always)]
    #[must_use]
    pub fn get_return_type(&self) -> VmType {
        unsafe { *self.types.get_unchecked(0) }
    }
}

pub struct StructField<'arena> {
    pub name: &'arena str,
    pub field_type: DataType,
    pub span: Span,
}

pub struct Struct<'arena> {
    pub name: &'arena str,
    pub fields: Box<[StructField<'arena>]>,
    pub id: u16,
    pub name_span: Span,
    pub src_file_idx: u16,
}

pub struct Program<'arena> {
    pub instructions: Vec<Instr>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub top_level_len: u32,
    pub initial_stack: Vec<Data>,
    pub top_level_base: u32,
    pub max_frame_regs: u32,
    pub pools: Pools,
    pub err_ctx: ErrorCtx<'arena>,
    pub dylib_fns: Vec<DylibFn>,
    pub structs: Vec<Struct<'arena>>,
    pub types: Vec<DataType>,
}

#[allow(clippy::struct_field_names)]
pub struct Pools {
    pub obj_pool: ObjectPool,
    pub map_pool: MapPool,
    pub str_pool: StringPool,
}
