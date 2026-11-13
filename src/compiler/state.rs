use crate::{
    ast::Span,
    compiler::scope::{Scope, Symbol},
};
use bumpalo::Bump;
use core::hint::unreachable_unchecked;
use fixedbitset::FixedBitSet;
use rustc_hash::FxHashMap;

use crate::{
    compiler::{
        program::{CodeUnit, Dylib, Function, InstrSrc, Pools, Struct},
        registers::{PERMANENT_REG_FLAG, is_permanent_register},
        types::DataType,
    },
    data::{Data, NULL},
    instr::Instr,
};

#[derive(Copy, Clone)]
pub struct Source<'a> {
    pub filename: &'a str,
    pub contents: &'a str,
}

#[derive(Clone, Copy)]
pub struct Ctx {
    pub in_function: bool,
    /// Whether the code being compiled is guaranteed to run at most once
    pub single_run: bool,
    /// Index of the current file in State's `sources`
    pub file_idx: u16,
    /// Instruction offset that's only used when compiling a function
    pub offset: u32,
    pub try_depth: u16,
    pub loop_try_depth: u16,
    /// None = top level code
    pub current_unit: Option<u32>,
}

impl Ctx {
    #[inline(always)]
    pub const fn no_single_run(self) -> Self {
        Self { single_run: false, loop_try_depth: self.try_depth, ..self }
    }
    #[inline(always)]
    pub const fn advance_offset(self, output_len: u32) -> Self {
        Self { offset: self.offset + output_len, ..self }
    }
    #[inline(always)]
    pub const fn with_offset(self, offset: u32) -> Self {
        Self { offset, ..self }
    }
    #[inline(always)]
    pub const fn with_file_idx(self, file_idx: u16) -> Self {
        Self { file_idx, ..self }
    }
}

#[derive(Default)]
pub struct CodeUnitRegisters {
    pub temp_count: u16,
    pub permanent_values: Vec<Data>,
    pub const_registers: FxHashMap<Data, u16>,
    pub const_registers_bitset: FixedBitSet,
    pub free_registers: Vec<u16>,
    pub free_registers_bitset: FixedBitSet,
    pub reserved_registers: FixedBitSet,
}

pub struct State<'arena, 'compiler> {
    pub v: &'compiler mut Vec<Variable<'arena>>,
    pub globals: &'compiler mut Vec<Variable<'arena>>,
    pub regs: CodeUnitRegisters,
    pub functions: &'compiler mut Vec<Function<'arena>>,
    pub code_units: &'compiler mut Vec<CodeUnit>,
    pub structs: &'compiler mut Vec<Struct<'arena>>,
    pub pools: &'compiler mut Pools,
    pub instr_src: &'compiler mut Vec<InstrSrc>,
    pub dylibs: &'compiler mut Vec<Dylib<'arena>>,
    pub function_constants: Vec<Data>,
    pub max_frame_regs: u32,
    pub sources: &'compiler mut Vec<Source<'arena>>,
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
    /// Hides the functions declared by the code currently being compiled
    pub fn enter_function_scope(
        &mut self,
        file_idx: u16,
        fn_id: usize,
    ) -> Vec<((&'arena str, Symbol), u16)> {
        let fn_name = self.functions[fn_id].name;
        let scope = self.scope_mut(file_idx);
        let hidden_symbols: Vec<((&str, Symbol), u16)> =
            scope.symbols.drain(scope.toplevel_count..).collect();
        // Only function names are hidden (avoids a nasty anonymous function name conflict)
        scope.symbols.extend(
            hidden_symbols.iter().filter(|((_, kind), _)| *kind == Symbol::Struct).copied(),
        );
        scope.symbols.insert((fn_name, Symbol::Fn), fn_id as u16);
        hidden_symbols
    }
    /// Restores all the symbols (used after `enter_function_scope`).
    pub fn exit_function_scope(
        &mut self,
        file_idx: u16,
        hidden_symbols: Vec<((&'arena str, Symbol), u16)>,
    ) {
        let scope = self.scope_mut(file_idx);
        scope.symbols.truncate(scope.toplevel_count);
        scope.symbols.extend(hidden_symbols);
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
    #[inline(always)]
    pub fn find_var(&self, var_name: &str) -> Option<&Variable<'_>> {
        self.v.iter().rfind(|variable| variable.name == var_name)
    }
    #[must_use]
    #[inline(always)]
    pub fn find_var_mut(&mut self, var_name: &str) -> Option<&mut Variable<'arena>> {
        self.v.iter_mut().rfind(|variable| variable.name == var_name)
    }
    #[must_use]
    #[inline(always)]
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
    /// Creates a brand new register and returns its index.
    #[must_use]
    pub fn scratch_reg(&mut self) -> u16 {
        let register_id = self.regs.temp_count;
        assert!(register_id < PERMANENT_REG_FLAG, "Your program is too big. Please report this.");
        self.regs.temp_count += 1;
        self.regs.free_registers_bitset.grow(register_id as usize + 1);
        self.regs.reserved_registers.grow(register_id as usize + 1);
        register_id
    }
    /// Creates a brand new permanent register holding the compile-time value `data`.
    /// Permanent registers are placed before every temp register once the unit is compiled, so a call can never overwrite them.
    /// Functions copy them into their frame on frame initialization.
    #[must_use]
    fn new_permanent_reg(&mut self, data: Data, is_const: bool) -> u16 {
        let idx = self.regs.permanent_values.len();
        assert!(
            idx < (u16::MAX - PERMANENT_REG_FLAG) as usize,
            "Your program is too big. Please report this."
        );
        // Don't grow `free_registers` because permanent registers can never be freed
        self.regs.permanent_values.push(data);
        self.regs.const_registers_bitset.grow(idx + 1);
        self.regs.const_registers_bitset.set(idx, is_const);
        PERMANENT_REG_FLAG + idx as u16
    }
    /// Allocates a brand new constant register containing `data` and returns its index.
    /// If a constant register containing `data` already exists, it simply returns its index and doesn't create a new register.
    #[must_use]
    pub fn new_const_reg(&mut self, data: Data) -> u16 {
        if let Some(&id) = self.regs.const_registers.get(&data) {
            id
        } else {
            let register_id = self.new_permanent_reg(data, true);
            self.regs.const_registers.insert(data, register_id);
            register_id
        }
    }
    /// Creates a new register holding the immutable compile-time value `data`.
    /// It's never shared, so it may become a variable's storage.
    #[must_use]
    #[inline(always)]
    pub fn value_reg(&mut self, data: Data) -> u16 {
        self.new_permanent_reg(data, false)
    }
    /// Creates a new register whose initial value `data` is known to be modified at run time.
    /// Only valid for top-level code that runs once.
    #[must_use]
    #[inline(always)]
    pub fn init_reg(&mut self, data: Data, ctx: Ctx) -> u16 {
        debug_assert!(ctx.single_run && !ctx.in_function);
        self.new_permanent_reg(data, false)
    }
    /// Returns the compile-time value of a const register or of a top-level initialized register.
    #[must_use]
    #[inline(always)]
    pub fn register_value(&self, id: u16) -> Data {
        if is_permanent_register(id) {
            self.regs.permanent_values[(id - PERMANENT_REG_FLAG) as usize]
        } else {
            NULL
        }
    }
    /// Marks a register as free, allowing it to later be reused by `alloc_reg`.
    /// The register is marked as free iff:
    /// - the register isn't tied to any variable
    /// - the register isn't permanent (a constant or compile-time value)
    /// - the register isn't reserved in `reserved_registers`
    /// - the register isn't already marked as free
    pub fn free_reg(&mut self, id: u16) {
        if !is_permanent_register(id)
            && unsafe { !self.regs.free_registers_bitset.contains_unchecked(id as usize) }
            && unsafe { !self.regs.reserved_registers.contains_unchecked(id as usize) }
            && !self.v.iter().any(|var| var.register_id == id)
        {
            unsafe { self.regs.free_registers_bitset.insert_unchecked(id as usize) };
            self.regs.free_registers.push(id);
        }
    }
    /// Allocates a register. It `free_registers` isn't empty, it will reuse the latest one. Else, it will allocate a new one.
    #[must_use]
    pub fn alloc_reg(&mut self) -> u16 {
        if let Some(reg) = self.regs.free_registers.pop() {
            unsafe {
                self.regs.free_registers_bitset.remove_unchecked(reg as usize);
            };
            reg
        } else {
            self.scratch_reg()
        }
    }
    /// Prevents `free_reg` from freeing `id`.
    pub fn reserve_register(&mut self, id: u16) {
        if !is_permanent_register(id) {
            unsafe { self.regs.reserved_registers.insert_unchecked(id as usize) };
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
        for instr in scope_instrs {
            if let Some(id) = instr.get_tgt_id()
                && id >= regs_before
            {
                self.free_reg(id);
            }
        }
    }
    /// Unfree register `id` if it's been freed (if it hasn't, do nothing).
    pub fn unfree_register(&mut self, id: u16) {
        if !is_permanent_register(id)
            && unsafe { self.regs.free_registers_bitset.contains_unchecked(id as usize) }
        {
            unsafe { self.regs.free_registers_bitset.remove_unchecked(id as usize) };
            let Some(free_reg_id_pos) =
                self.regs.free_registers.iter().rposition(|&reg_id| reg_id == id)
            else {
                unsafe { unreachable_unchecked() }
            };
            self.regs.free_registers.swap_remove(free_reg_id_pos);
        }
    }
    #[inline(always)]
    #[must_use]
    pub fn is_register_const(&self, id: u16) -> bool {
        is_permanent_register(id)
            && self.regs.const_registers_bitset.contains((id - PERMANENT_REG_FLAG) as usize)
    }
    pub fn reserve_call_window(&mut self, first: u16, len: u16) {
        for id in first..first + len {
            if id < self.regs.temp_count {
                self.unfree_register(id);
            } else {
                let new_id = self.scratch_reg();
                debug_assert_eq!(new_id, id);
            }
        }
    }
    #[inline(always)]
    #[must_use]
    pub fn alloc_reg_window(&mut self, len: u16) -> u16 {
        let first = self.call_frame_offset();
        self.reserve_call_window(first, len);
        first
    }
    #[inline(always)]
    pub fn free_reg_window(&mut self, first: u16, len: u16) {
        for id in first..first + len {
            self.free_reg(id);
        }
    }
    #[must_use]
    pub fn call_frame_offset(&self) -> u16 {
        (0..self.regs.temp_count)
            .rev()
            .find(|&id| unsafe { !self.regs.free_registers_bitset.contains_unchecked(id as usize) })
            .map_or(0, |id| id + 1)
    }
    /// Associates the last instruction in `output` with `span` and adds the `InstrSrc` to `instr_src`.
    /// This allows runtime errors to be traced back to `span` in the source code.
    #[inline(always)]
    pub fn add_to_src(&mut self, ctx: Ctx, output: &[Instr], span: Span) {
        let new_src = InstrSrc {
            instr_idx: ctx.offset + output.len() as u32 - 1,
            span,
            file_id: ctx.file_idx,
        };
        let instr_sources = match ctx.current_unit {
            Some(unit) => &mut self.code_units[unit as usize].instr_src,
            None => self.instr_src,
        };
        // Makes it so an instruction rewritten in place (GetNestedIndexArray for example) keeps a single source
        match instr_sources.last_mut() {
            Some(last) if last.instr_idx == new_src.instr_idx => *last = new_src,
            _ => instr_sources.push(new_src),
        }
    }
    /// Shifts instruction sources starting at `start_idx` by `shift`
    pub fn shift_instr_src(&mut self, ctx: Ctx, start_idx: u32, shift: u32) {
        let instr_sources = match ctx.current_unit {
            Some(unit) => &mut self.code_units[unit as usize].instr_src,
            None => self.instr_src,
        };
        instr_sources
            .iter_mut()
            .filter(|src| src.instr_idx >= start_idx)
            .for_each(|src| src.instr_idx += shift);
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
