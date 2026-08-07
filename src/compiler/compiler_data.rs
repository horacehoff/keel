use super::expr::Expr;
use super::expr::Span;
use super::registers::get_tgt_ids;
use super::type_system::DataType;
use crate::compiler::Scope;
use crate::data::Data;
use crate::data::NULL;
use crate::instr::Instr;
use crate::vm::MapPool;
use crate::vm::ObjectPool;
use crate::vm::StringPool;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use smol_strc::SmolStr;
use std::rc::Rc;

#[cfg(not(target_arch = "wasm32"))]
use libloading::Library;

#[derive(Debug)]
pub struct ErrorCatch {
    pub catch_loc: u16,
    pub error_reg: u16,
    pub call_frames_len: u16,
    pub args_len: u16,
}

#[derive(Debug)]
pub struct Function {
    pub name: SmolStr,
    pub args: Box<[(SmolStr, Option<DataType>)]>,
    pub code: Rc<[Expr]>,
    pub impls: Vec<FunctionImpl>,
    pub is_recursive: Option<bool>,
    pub returns_null: bool,
    pub src_file: u16,
    /// Cache of return types from track_returns, keyed by Box<arg types>
    pub return_type_cache: Vec<(Box<[DataType]>, DataType)>,
    pub direct_calls: Box<[SmolStr]>,
    pub name_span: Span,
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
pub struct Dylib {
    pub name: SmolStr,
    pub fns: Box<[FnSignature]>,
}

#[derive(Debug)]
pub struct DylibFn {
    /// [ return_type, arg_types... ]
    pub types: Box<[DataType]>,
    #[cfg(not(target_arch = "wasm32"))]
    pub _lib: Rc<Library>,
    #[cfg(not(target_arch = "wasm32"))]
    pub ptr: libffi::middle::CodePtr,
    #[cfg(not(target_arch = "wasm32"))]
    pub cif: libffi::middle::Cif,
}

impl DylibFn {
    #[inline(always)]
    pub fn get_return_type(&self) -> &DataType {
        unsafe { self.types.get_unchecked(0) }
    }
}

#[derive(Debug)]
pub struct StructField {
    pub name: SmolStr,
    pub field_type: DataType,
    pub span: Span,
}

#[derive(Debug)]
pub struct Struct {
    pub name: SmolStr,
    pub fields: Box<[StructField]>,
    pub id: u16,
    pub name_span: Span,
}

#[allow(clippy::struct_field_names)]
pub struct Pools {
    pub obj_pool: ObjectPool,
    pub map_pool: MapPool,
    pub str_pool: StringPool,
}

pub struct Source {
    pub filename: SmolStr,
    pub contents: String,
}

#[derive(Clone, Copy)]
pub struct Ctx {
    pub block_id: u16,
    /// Whether the code being compiled is within a recursive function
    pub is_compiling_recursive: bool,
    /// Whether the code being compiled is guaranteed to run at most once
    pub single_run: bool,
    /// Index of the current file in State's `sources`
    pub file_idx: u16,
    /// Instruction offset that's only used when compiling a function
    pub offset: u16,
}

impl Ctx {
    #[inline(always)]
    pub const fn no_single_run(self) -> Self {
        Self { single_run: false, ..self }
    }
    #[inline(always)]
    pub const fn advance_offset(self, output_len: u16) -> Self {
        Self { offset: self.offset + output_len, ..self }
    }
    #[inline(always)]
    pub const fn with_offset(self, offset: u16) -> Self {
        Self { offset, ..self }
    }
    #[inline(always)]
    pub const fn with_file_idx(self, file_idx: u16) -> Self {
        Self { file_idx, ..self }
    }
}

#[derive(Copy, Clone)]
pub struct InstrSrc {
    pub instr: Instr,
    pub span: Span,
    pub file_id: u16,
}

pub struct State<'a> {
    pub v: &'a mut Vec<Variable>,
    pub globals: &'a mut Vec<Variable>,
    pub registers: &'a mut Vec<Data>,
    pub functions: &'a mut Vec<Function>,
    pub structs: &'a mut Vec<Struct>,
    pub pools: &'a mut Pools,
    pub instr_src: &'a mut Vec<InstrSrc>,
    pub fn_registers: &'a mut Vec<Vec<u16>>,
    pub dylibs: &'a mut Vec<Dylib>,
    pub allocated_arg_count: &'a mut usize,
    pub allocated_call_depth: &'a mut usize,
    pub const_registers: &'a mut FxHashMap<Data, u16>,
    pub free_registers: &'a mut Vec<u16>,
    pub sources: &'a mut Vec<Source>,
    pub reserved_registers: FxHashSet<u16>,
    pub file_scopes: &'a mut Vec<Scope>,
}

impl State<'_> {
    #[must_use]
    #[inline(always)]
    pub fn scope(&self, file_idx: u16) -> &Scope {
        unsafe { self.file_scopes.get_unchecked(file_idx as usize) }
    }
    #[must_use]
    #[inline(always)]
    pub fn scope_mut(&mut self, file_idx: u16) -> &mut Scope {
        unsafe { self.file_scopes.get_unchecked_mut(file_idx as usize) }
    }
    #[must_use]
    pub fn find_var(&self, var_name: &str) -> Option<&Variable> {
        self.v.iter().rfind(|variable| variable.name.as_str() == var_name)
    }
    #[must_use]
    pub fn find_var_mut(&mut self, var_name: &str) -> Option<&mut Variable> {
        self.v.iter_mut().rfind(|variable| variable.name.as_str() == var_name)
    }
    #[must_use]
    pub fn find_var_idx(&self, var_name: &str) -> Option<usize> {
        self.v.iter().rposition(|variable| variable.name.as_str() == var_name)
    }
    #[inline(always)]
    pub fn new_var(&mut self, name: SmolStr, register_id: u16, var_type: DataType) {
        self.v.push(Variable { name, register_id, var_type });
    }
    /// Creates a brand new register containing `data` and returns its index.
    #[must_use]
    pub fn new_reg(&mut self, data: Data) -> u16 {
        let register_id = self.registers.len() as u16;
        self.registers.push(data);
        register_id
    }
    /// Allocates a brand new constant register containing `data` and returns its index.
    /// If a constant register containing `data` already exists, it simply returns its index and doesn't create a new register.
    #[must_use]
    pub fn new_const_reg(&mut self, data: Data) -> u16 {
        if let Some(&id) = self.const_registers.get(&data) {
            id
        } else {
            let register_id = self.registers.len() as u16;
            self.const_registers.insert(data, register_id);
            self.registers.push(data);
            register_id
        }
    }
    /// Marks a register as free, allowing it to later be reused by `alloc_reg`.
    /// The register is marked as free iff:
    /// - the register isn't tied to any variable
    /// - the register isn't a constant register
    /// - the register isn't reserved in `reserved_registers`
    /// - the register isn't already marked as free
    pub fn free_reg(&mut self, id: u16) {
        if !self.v.iter().any(|var| var.register_id == id)
            && !self.const_registers.values().any(|&reg| reg == id)
            && !self.reserved_registers.contains(&id)
            && !self.free_registers.contains(&id)
        {
            self.free_registers.push(id);
        }
    }
    /// Allocates a register. It `free_registers` isn't empty, it will reuse the latest one. Else, it will allocate a new one.
    #[must_use]
    pub fn alloc_reg(&mut self) -> u16 {
        if let Some(reg) = self.free_registers.pop() { reg } else { self.new_reg(NULL) }
    }
    /// Allocates a register, reusing `tgt_id` if it holds some register id.
    /// If `tgt_id == None`, it calls `alloc_reg()`.
    #[must_use]
    #[inline(always)]
    pub fn alloc_reg_tgt(&mut self, tgt_id: Option<u16>) -> u16 {
        if let Some(id) = tgt_id { id } else { self.alloc_reg() }
    }
    /// Frees registers that are written by instructions in scope_instrs.
    pub fn free_scope_registers(&mut self, regs_before: u16, scope_instrs: &[Instr]) {
        for id in get_tgt_ids(scope_instrs) {
            if id >= regs_before {
                self.free_reg(id);
            }
        }
    }

    /// Similar to free_scope_registers, but also frees CloneArray template registers. Only call this after a loop ends.
    pub fn free_loop_scope_registers(&mut self, regs_before: u16, scope_instrs: &[Instr]) {
        self.free_scope_registers(regs_before, scope_instrs);
        // Free CloneArray template registers
        for instr in scope_instrs {
            if let Instr::CloneArray(template_reg, _, _) = instr
                && *template_reg >= regs_before
            {
                self.free_reg(*template_reg);
            } else if let Instr::CloneStruct(template_reg, _) = instr
                && *template_reg >= regs_before
            {
                self.free_reg(*template_reg);
            }
        }
    }
    /// Associates the last instruction in `output` with `span` and adds the `InstrSrc` to `instr_src`.
    /// This allows runtime errors to be traced back to `span` in the source code.
    #[inline(always)]
    pub fn add_to_src(&mut self, ctx: Ctx, output: &[Instr], span: Span) {
        self.instr_src.push(InstrSrc {
            instr: unsafe { *output.last().unwrap_unchecked() },
            span,
            file_id: ctx.file_idx,
        });
    }
}

#[derive(Debug)]
pub struct Variable {
    pub name: SmolStr,
    pub register_id: u16,
    pub var_type: DataType,
}
