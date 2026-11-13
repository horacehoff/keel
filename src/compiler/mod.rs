use crate::ast::Expr;
use crate::ast::FunctionDeclarationExpr;
use crate::ast::IfBlockExpr;
use crate::ast::IntForLoopExpr;
use crate::ast::Span;
use crate::ast::StructFieldAssignmentExpr;
use crate::ast::StructFieldExpr;
use crate::ast::VariableDeclarationExpr;
use crate::ast::code_modifies_variable;
use crate::compiler::compiler_errors::error_cannot_push_type_to_array;
use crate::compiler::compiler_errors::error_division_by_zero;
use crate::compiler::compiler_errors::error_invalid_index_type;
use crate::compiler::compiler_errors::error_invalid_type;
use crate::compiler::compiler_errors::error_range_invalid_type;
use crate::compiler::compiler_errors::error_struct_already_defined;
use crate::compiler::compiler_errors::error_type_not_indexable;
use crate::compiler::compiler_errors::error_unknown_namespace;
use crate::compiler::program::CodeUnit;
use crate::compiler::program::InstrSrc;
use crate::compiler::program::Program;
use crate::compiler::program::StructField;
use crate::compiler::registers::move_value_to;
use crate::compiler::registers::relocate_register;
use crate::compiler::registers::relocate_registers;
use crate::compiler::registers::to_nested_operand;
use crate::compiler::scope::Scope;
use crate::compiler::scope::Symbol;
use crate::compiler::state::CodeUnitRegisters;
use crate::compiler::state::Ctx;
pub use crate::compiler::state::Source;
use crate::compiler::state::State;
use crate::compiler::state::Variable;
use crate::compiler::toplevel::parse_toplevel;
use crate::compiler::toplevel::resolve_types;
use crate::compiler::type_system::var_type_is_compatible;
use crate::data::NULL;
use crate::errors::BLUE;
use crate::errors::BOLD;
use crate::errors::ERROR_PREFIX;
use crate::errors::ErrorCtx;
use crate::errors::RESET;
use crate::instr::Instr;
use crate::parser;
use crate::vm::Pool;
use bumpalo::Bump;
pub use codegen::compile_expr;
use core::cell::LazyCell;
use program::Dylib;
use program::DylibFn;
use program::Function;
use program::Pools;
use program::Struct;
use registers::UnwrapId;
use registers::move_reg_to_reg;
use rustc_hash::FxHashMap;
use std::path::PathBuf;
use type_system::DataType;
use type_system::TypeExpr;
use type_system::check_if_returns_void;
use type_system::collect_direct_fn_calls;
use type_system::struct_field_type_matches;

#[cfg(not(target_arch = "wasm32"))]
use libloading::Library;

#[cfg(target_arch = "wasm32")]
use crate::errors::wasm_error;

mod codegen;
mod compiler_errors;
#[path = "functions/functions.rs"]
mod functions;
#[path = "functions/methods.rs"]
mod methods;
pub mod program;
mod registers;
mod scope;
mod state;
mod toplevel;
pub mod type_system;

pub fn compile<'arena>(
    contents: &str,
    filename: &'arena str,
    #[allow(unused_variables)] debug: bool,
    bump: &'arena Bump,
) -> Program<'arena> {
    #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
    let now = std::time::Instant::now();

    let main_src_contents = bump.alloc_str(contents);

    let main_src = Source { filename, contents: main_src_contents };

    let code = parser::parse(main_src.contents, main_src, bump);

    #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
    if debug {
        println!("PARSING TIME: {:.2?}", now.elapsed());
    }

    let mut variables: Vec<Variable> = Vec::new();
    let mut globals: Vec<Variable> = Vec::new();
    let mut pools: Pools = Pools {
        obj_pool: Pool::with_capacity(5),
        map_pool: Pool::with_capacity(0),
        str_pool: Pool::with_capacity(5),
    };
    let mut instr_src: Vec<InstrSrc> = Vec::new();
    let mut functions: Vec<Function> = Vec::new();
    let mut code_units: Vec<CodeUnit> = Vec::new();
    let mut structs: Vec<Struct> = Vec::new();
    let mut dyn_libs: Vec<Dylib> = Vec::new();
    let mut dylib_fns: Vec<DylibFn> = Vec::new();
    let mut types: Vec<DataType> = Vec::new();

    let mut sources: Vec<Source> = vec![main_src];
    let mut scope = Scope::default();

    let mut files: FxHashMap<PathBuf, Scope> = FxHashMap::default();
    let mut file_scopes: Vec<Scope> = Vec::new();
    let mut pending_structs = Vec::new();
    let mut pending_fns = Vec::with_capacity(2);
    #[cfg(not(target_arch = "wasm32"))]
    let mut pending_dylibs = Vec::new();
    let mut pending_globals: Vec<(VariableDeclarationExpr, u16)> = Vec::new();

    let keel_home_libs =
        LazyCell::new(|| std::env::home_dir().map(|p| p.join(".keel").join("libs/")));

    parse_toplevel(
        bump,
        code,
        &PathBuf::from(filename),
        0,
        &mut functions,
        &mut structs,
        &mut dyn_libs,
        &mut sources,
        &mut scope,
        &mut files,
        &mut file_scopes,
        &mut pending_structs,
        &mut pending_fns,
        #[cfg(not(target_arch = "wasm32"))]
        &mut pending_dylibs,
        &mut pending_globals,
        &keel_home_libs,
    );
    resolve_types(
        &mut structs,
        &mut functions,
        pending_structs,
        pending_fns,
        #[cfg(not(target_arch = "wasm32"))]
        pending_dylibs,
        &file_scopes,
        &mut dylib_fns,
        &mut dyn_libs,
        &sources,
    );

    let ctx = Ctx {
        in_function: false,
        file_idx: 0,
        single_run: true,
        offset: 0,
        try_depth: 0,
        loop_try_depth: 0,
        current_unit: None,
    };
    let mut state = State {
        v: &mut variables,
        globals: &mut globals,
        regs: CodeUnitRegisters::default(),
        functions: &mut functions,
        code_units: &mut code_units,
        structs: &mut structs,
        pools: &mut pools,
        instr_src: &mut instr_src,
        dylibs: &mut dyn_libs,
        function_constants: Vec::new(),
        max_frame_regs: 0,
        sources: &mut sources,
        file_scopes: &mut file_scopes,
        types: &mut types,
        bump,
    };
    let mut instructions: Vec<Instr> = Vec::with_capacity(4);
    for (var_declaration, file_idx) in pending_globals {
        let var_ctx = ctx.with_file_idx(file_idx);
        let value_type = var_declaration.value.infer_type(var_ctx, &mut state);
        let declared_type = if let Some(v_t) = var_declaration.var_type {
            let declared_var_type =
                v_t.0.to_datatype(file_idx, state.scope(file_idx), state.sources);
            if !var_type_is_compatible(&declared_var_type, &value_type) {
                error_invalid_type(
                    &declared_var_type,
                    &value_type,
                    v_t.1,
                    None,
                    None,
                    file_idx,
                    state.sources,
                );
            }
            declared_var_type
        } else {
            value_type.clone()
        };
        let register_id = var_declaration
            .value
            .compile(var_ctx, &mut state, &mut instructions, None, true, true)
            .unwrap_id();
        state.reserve_register(register_id);
        state.globals.push(Variable {
            name: var_declaration.name,
            register_id,
            declared_type,
            var_type: value_type,
        });
    }
    let program_instructions = compile_expr(
        state
            .functions
            .iter()
            .find(|func| func.name == "main" && func.src_file_idx == 0)
            .unwrap_or_else(|| {
                #[cfg(target_arch = "wasm32")]
                wasm_error("Cannot find main function");

                eprintln!("{ERROR_PREFIX} Cannot find {BLUE}{BOLD}main{RESET} function");
                std::process::exit(1);
            })
            .code,
        // &mut variables,
        ctx.with_offset(instructions.len() as u32),
        &mut state,
    );
    instructions.reserve_exact(program_instructions.len() + 1);
    instructions.extend(program_instructions);
    instructions.push(Instr::Halt(u16::MAX));

    let CodeUnitRegisters { temp_count, permanent_values: mut top_level_values, .. } = state.regs;
    let mut initial_stack = state.function_constants;
    let top_level_permanent_count = top_level_values.len() as u16;
    let max_frame_regs =
        state.max_frame_regs.max(u32::from(temp_count + top_level_permanent_count));
    relocate_registers(&mut instructions, 0, top_level_permanent_count);
    let top_level_len = instructions.len() as u32;

    let top_level_base = initial_stack.len() as u32;
    // Appends every code unit after the top-level code
    let mut unit_addresses: Vec<u32> = Vec::with_capacity(code_units.len());
    instructions.reserve_exact(code_units.iter().map(|unit| unit.instructions.len()).sum());
    for CodeUnit { instructions: mut unit_instructions, instr_src: unit_instr_src } in code_units {
        let base = instructions.len() as u32;
        unit_addresses.push(base);
        instr_src.extend(
            unit_instr_src
                .into_iter()
                .map(|src| InstrSrc { instr_idx: src.instr_idx + base, ..src }),
        );
        instructions.append(&mut unit_instructions);
    }
    for instr in &mut instructions {
        match instr {
            Instr::CallFunc { target, .. } => *target = unit_addresses[*target as usize],
            Instr::LoadGlobal { reg, .. } | Instr::StoreGlobal { reg, .. } => {
                *reg = top_level_base
                    + u32::from(relocate_register(*reg as u16, 0, top_level_permanent_count));
            }
            _ => {}
        }
    }

    initial_stack.reserve_exact(top_level_values.len() + temp_count as usize);
    initial_stack.append(&mut top_level_values);
    initial_stack.resize(initial_stack.len() + temp_count as usize, NULL);

    #[cfg(debug_assertions)]
    if debug {
        println!("---- DEBUG ----");
        if !pools.obj_pool.is_empty() {
            println!("---  OBJECTS  ---");
            for (i, data) in pools.obj_pool.iter().enumerate() {
                println!(" {i} {data:?}");
            }
        }
        println!("-- REGISTERS --");
        for (i, data) in initial_stack.iter().enumerate() {
            println!(
                " [{i}] {}",
                data.format(&pools.obj_pool, &pools.str_pool, &pools.map_pool, &structs, true)
            );
        }
        if !instructions.is_empty() {
            println!("-- INSTRUCTIONS --");
            for (i, instr) in instructions.iter().enumerate() {
                println!(" {i}: {instr:?}");
            }
        }
        println!("------------------");
    }

    Program {
        instructions,
        top_level_len,
        initial_stack,
        top_level_base,
        max_frame_regs,
        pools,
        err_ctx: ErrorCtx { instr_src, sources },
        dylib_fns,
        structs,
        types,
    }

    // VmData {
    //     instructions,
    //     registers: RegisterFile(registers),
    //     pools,
    //     err_ctx: ErrorCtx { instr_src, sources },
    //     fn_registers,
    //     dylib_fns,
    //     allocated_arg_count,
    //     allocated_call_depth,
    //     structs,
    // }
}
