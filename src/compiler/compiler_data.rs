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
use bumpalo::Bump;
use fixedbitset::FixedBitSet;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use std::hint::unreachable_unchecked;
use std::rc::Rc;

#[cfg(not(target_arch = "wasm32"))]
use libloading::Library;

pub struct ErrorCatch {
    pub catch_loc: u16,
    pub error_reg: u16,
    pub call_frames_len: u16,
    pub args_len: u16,
}

pub struct Function<'arena> {
    pub name: &'arena str,
    pub args: Box<[(&'arena str, Option<DataType>)]>,
    pub code: &'arena [Expr<'arena>],
    pub impls: Vec<FunctionImpl<'arena>>,
    pub is_recursive: Option<bool>,
    pub returns_null: bool,
    pub src_file: u16,
    /// Cache of return types from track_returns, keyed by Box<arg types>
    pub return_type_cache: Vec<(Box<[DataType]>, DataType)>,
    pub direct_calls: &'arena [&'arena str],
    pub name_span: Span,
}

pub struct FunctionImpl<'arena> {
    pub loc: u16,
    pub args_loc: &'arena [u16],
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
    #[must_use]
    pub fn get_argument_type(&self, index: usize) -> &DataType {
        unsafe { self.types.get_unchecked(index + 1) }
    }
    #[inline(always)]
    #[must_use]
    pub fn get_return_type(&self) -> &DataType {
        unsafe { self.types.get_unchecked(0) }
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
}

#[allow(clippy::struct_field_names)]
pub struct Pools {
    pub obj_pool: ObjectPool,
    pub map_pool: MapPool,
    pub str_pool: StringPool,
}

#[derive(Copy, Clone)]
pub struct Source<'a> {
    pub filename: &'a str,
    pub contents: &'a str,
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

pub struct InstrSrc {
    pub instr: Instr,
    pub span: Span,
    pub file_id: u16,
}

pub struct State<'arena, 'compiler> {
    pub v: &'compiler mut Vec<Variable<'arena>>,
    pub globals: &'compiler mut Vec<Variable<'arena>>,
    pub registers: &'compiler mut Vec<Data>,
    pub functions: &'compiler mut Vec<Function<'arena>>,
    pub structs: &'compiler mut Vec<Struct<'arena>>,
    pub pools: &'compiler mut Pools,
    pub instr_src: &'compiler mut Vec<InstrSrc>,
    pub fn_registers: &'compiler mut Vec<Vec<u16>>,
    pub dylibs: &'compiler mut Vec<Dylib<'arena>>,
    pub allocated_arg_count: &'compiler mut usize,
    pub allocated_call_depth: &'compiler mut usize,
    pub const_registers: &'compiler mut FxHashMap<Data, u16>,
    pub const_registers_bitset: &'compiler mut FixedBitSet,
    pub free_registers: &'compiler mut Vec<u16>,
    pub free_registers_bitset: &'compiler mut FixedBitSet,
    pub sources: &'compiler mut Vec<Source<'arena>>,
    pub reserved_registers: FxHashSet<u16>,
    pub file_scopes: &'compiler mut Vec<Scope<'arena>>,
    pub types: &'compiler mut Vec<DataType>,
    pub bump: &'arena Bump,
}

impl<'arena> State<'arena, '_> {
    #[must_use]
    #[inline(always)]
    pub fn scope(&self, file_idx: u16) -> &Scope<'_> {
        unsafe { self.file_scopes.get_unchecked(file_idx as usize) }
    }
    #[must_use]
    #[inline(always)]
    pub fn scope_mut(&mut self, file_idx: u16) -> &mut Scope<'arena> {
        unsafe { self.file_scopes.get_unchecked_mut(file_idx as usize) }
    }
    #[must_use]
    pub fn compile_type(&mut self, t: DataType) -> u16 {
        if let Some(i) = self.types.iter().position(|x| *x == t) {
            i as u16
        } else {
            self.types.push(t);
            (self.types.len() - 1) as u16
        }
    }
    #[must_use]
    pub fn find_var(&self, var_name: &str) -> Option<&Variable<'_>> {
        self.v.iter().rfind(|variable| variable.name == var_name)
    }
    #[must_use]
    pub fn find_var_mut(&mut self, var_name: &str) -> Option<&mut Variable<'arena>> {
        self.v.iter_mut().rfind(|variable| variable.name == var_name)
    }
    #[must_use]
    pub fn find_var_idx(&self, var_name: &str) -> Option<usize> {
        self.v.iter().rposition(|variable| variable.name == var_name)
    }
    #[inline(always)]
    pub fn new_var(&mut self, name: &'arena str, register_id: u16, var_type: DataType) {
        self.v.push(Variable { name, register_id, declared_type: var_type.clone(), var_type });
    }
    #[inline(always)]
    pub fn new_var_with_type(
        &mut self,
        name: &'arena str,
        register_id: u16,
        var_type: DataType,
        declared_type: DataType,
    ) {
        self.v.push(Variable { name, register_id, declared_type, var_type });
    }
    /// Creates a brand new register containing `data` and returns its index.
    #[must_use]
    pub fn new_reg(&mut self, data: Data) -> u16 {
        let register_id = self.registers.len();
        self.free_registers_bitset.grow(register_id + 1);
        self.const_registers_bitset.grow(register_id + 1);
        self.registers.push(data);
        register_id as u16
    }
    /// Allocates a brand new constant register containing `data` and returns its index.
    /// If a constant register containing `data` already exists, it simply returns its index and doesn't create a new register.
    #[must_use]
    pub fn new_const_reg(&mut self, data: Data) -> u16 {
        if let Some(&id) = self.const_registers.get(&data) {
            id
        } else {
            let register_id = self.new_reg(data);
            self.const_registers.insert(data, register_id);
            unsafe { self.const_registers_bitset.insert_unchecked(register_id as usize) };
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
            && !unsafe { self.const_registers_bitset.contains_unchecked(id as usize) }
            && !self.reserved_registers.contains(&id)
            && !unsafe { self.free_registers_bitset.contains_unchecked(id as usize) }
        {
            unsafe { self.free_registers_bitset.insert_unchecked(id as usize) };
            self.free_registers.push(id);
        }
    }
    /// Allocates a register. It `free_registers` isn't empty, it will reuse the latest one. Else, it will allocate a new one.
    #[must_use]
    pub fn alloc_reg(&mut self) -> u16 {
        if let Some(reg) = self.free_registers.pop() {
            unsafe {
                self.free_registers_bitset.remove_unchecked(reg as usize);
            };
            reg
        } else {
            self.new_reg(NULL)
        }
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
    /// Unfree register `id` if it's been freed (if it hasn't, do nothing).
    pub fn unfree_register(&mut self, id: u16) {
        if unsafe { self.free_registers_bitset.contains_unchecked(id as usize) } {
            unsafe { self.free_registers_bitset.remove_unchecked(id as usize) };
            let Some(free_reg_id_pos) =
                self.free_registers.iter().rposition(|&reg_id| reg_id == id)
            else {
                unsafe { unreachable_unchecked() }
            };
            self.free_registers.swap_remove(free_reg_id_pos);
        }
    }
    /// Unfree all the registers in `free_registers` who are also in `reserved_registers`.
    pub fn unfree_reserved_registers(&mut self) {
        self.free_registers.retain(|reg| {
            if self.reserved_registers.contains(reg) {
                unsafe {
                    self.free_registers_bitset.remove_unchecked(*reg as usize);
                }
                false
            } else {
                true
            }
        });
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

pub struct Variable<'arena> {
    pub name: &'arena str,
    pub register_id: u16,
    /// Fixed at var declaration and never changes
    pub declared_type: DataType,
    /// Can change as long as it's compatible with `declared_type`
    pub var_type: DataType,
}
