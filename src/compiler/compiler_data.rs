use crate::compiler::Namespace;
use crate::data::Data;
use crate::data::NULL;
use crate::expr::Expr;
use crate::expr::Span;
use crate::instr::Instr;
use crate::registers::get_tgt_ids;
use crate::type_system::DataType;
use crate::vm::MapPool;
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
    pub returns_null: bool,
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
    pub _lib: Rc<Library>,
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
    pub objs: ObjectPool,
    pub maps: MapPool,
    pub strings: StringPool,
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

impl State<'_> {
    pub fn free_reg(&mut self, id: u16, v: &[Variable]) {
        if !v.iter().any(|var| var.register_id == id)
            && !self.const_registers.values().any(|&reg| reg == id)
            && !self.reserved_registers.contains(&id)
            && !self.free_registers.contains(&id)
        {
            self.free_registers.push(id);
        }
    }
    pub fn alloc_reg(&mut self) -> u16 {
        if self.reserved_registers.is_empty() {
            self.free_registers.pop().unwrap_or_else(|| {
                self.registers.push(NULL);
                (self.registers.len() - 1) as u16
            })
        } else if let Some(pos) = self
            .free_registers
            .iter()
            .rposition(|reg| !self.reserved_registers.contains(reg))
        {
            self.free_registers.swap_remove(pos)
        } else {
            self.registers.push(NULL);
            (self.registers.len() - 1) as u16
        }
    }
    /// Frees registers that are written by instructions in scope_instrs & are not held by a variable & and are not in const_registers.
    pub fn free_scope_registers(
        &mut self,
        regs_before: u16,
        scope_instrs: &[Instr],
        v: &[Variable],
    ) {
        for id in get_tgt_ids(scope_instrs) {
            if id >= regs_before && !self.reserved_registers.contains(&id) {
                self.free_reg(id, v);
            }
        }
    }

    /// Similar to free_scope_registers, but also frees CloneArray template registers. Only call this after a loop ends.
    pub fn free_loop_scope_registers(
        &mut self,
        regs_before: u16,
        scope_instrs: &[Instr],
        v: &[Variable],
    ) {
        self.free_scope_registers(regs_before, scope_instrs, v);
        // Free CloneArray template registers
        for instr in scope_instrs {
            if let Instr::CloneArray(template_reg, _, _) = instr
                && *template_reg >= regs_before
                && !self.reserved_registers.contains(template_reg)
            {
                self.free_reg(*template_reg, v);
            } else if let Instr::CloneStruct(template_reg, _) = instr
                && *template_reg >= regs_before
                && !self.reserved_registers.contains(template_reg)
            {
                self.free_reg(*template_reg, v);
            }
        }
    }
}

#[derive(Debug)]
pub struct Variable {
    pub name: SmolStr,
    pub register_id: u16,
    pub var_type: DataType,
}
