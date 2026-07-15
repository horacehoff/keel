use crate::compiler::compile;
use crate::errors::BOLD;
use crate::errors::ErrorCtx;
use crate::errors::RED;
use crate::errors::RESET;
use crate::repl::repl;
use crate::vm::RegisterFile;
#[cfg(feature = "embed")]
use std::ffi::{CStr, CString, c_char};
use std::fs;
use std::hint::cold_path;
#[cfg(feature = "embed")]
use std::panic::catch_unwind;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[path = "./vm/gc/array_gc.rs"]
mod array_gc;
#[cfg(any(target_arch = "wasm32", feature = "embed"))]
mod captured_output;
#[path = "./compiler/compiler.rs"]
mod compiler;
#[path = "./data.rs"]
mod data;
#[path = "./util/errors.rs"]
mod errors;
#[path = "./instr.rs"]
mod instr;
#[path = "./vm/gc/map_gc.rs"]
mod map_gc;
#[path = "./parser/parser.rs"]
mod parser;
#[path = "./repl.rs"]
mod repl;
#[path = "./vm/gc/string_gc.rs"]
mod string_gc;
#[path = "./tests.rs"]
#[cfg(test)]
mod tests;
#[path = "./util/util.rs"]
mod util;
#[path = "./vm/vm.rs"]
mod vm;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_output() -> String {
    captured_output::CAPTURED_OUTPUT.with(|o| o.take())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn run(code: String) {
    captured_output::CAPTURED_OUTPUT.with(|o| o.borrow_mut().clear());
    let (
        instructions,
        mut registers,
        mut arrays,
        instr_src,
        fn_registers,
        fn_dyn_libs,
        allocated_arg_count,
        allocated_call_depth,
        sources,
        struct_fields,
    ) = compile(code, "playground.kl", false);
    vm::execute(
        &instructions,
        &mut registers,
        &mut arrays,
        &ErrorCtx { instr_src, sources },
        &fn_registers,
        &fn_dyn_libs,
        &struct_fields,
        allocated_arg_count,
        allocated_call_depth,
    );
}

#[cfg(feature = "embed")]
#[unsafe(no_mangle)]
#[allow(clippy::missing_safety_doc)] // WIP
pub unsafe extern "C" fn keel_run(code: *const c_char) -> *mut c_char {
    let code = unsafe { CStr::from_ptr(code) }
        .to_string_lossy()
        .to_string();
    captured_output::CAPTURED_OUTPUT.with(|o| o.borrow_mut().clear());
    let _ = catch_unwind(|| {
        let (
            instructions,
            mut registers,
            mut arrays,
            instr_src,
            fn_registers,
            fn_dyn_libs,
            allocated_arg_count,
            allocated_call_depth,
            sources,
            struct_fields,
        ) = compile(code, "embedded.kl", false);
        vm::execute(
            &instructions,
            &mut RegisterFile(registers),
            &mut arrays,
            &ErrorCtx { instr_src, sources },
            &fn_registers,
            &fn_dyn_libs,
            &struct_fields,
            allocated_arg_count,
            allocated_call_depth,
        );
    });
    let output = captured_output::CAPTURED_OUTPUT.with(|o| o.take());
    CString::new(output).unwrap_or_default().into_raw()
}

#[cfg(feature = "embed")]
#[unsafe(no_mangle)]
#[allow(clippy::missing_safety_doc)] // WIP
pub unsafe extern "C" fn keel_free_output(output: *mut c_char) {
    if !output.is_null() {
        #[allow(unused_must_use)]
        unsafe {
            CString::from_raw(output)
        };
    }
}

pub fn main() {
    #[cfg(not(debug_assertions))]
    std::panic::set_hook(Box::new(|info| {
        eprintln!("{RED}KEEL ERROR{RESET}\n{info}");
    }));

    let mut args = std::env::args().skip(1);

    if args.len() == 0 {
        cold_path();
        repl();
        return;
    }

    let next_arg = unsafe { args.next().unwrap_unchecked() };

    if next_arg == "--help" || next_arg == "-h" {
        cold_path();
        println!(
            "{}\nKeel is a fast, statically-typed interpreted language that aims to combine Rust-like syntax with Python's ease-of-use.\n\nUsage:\n  keel myfile.kl\n  keel [-v | --version]",
            util::KEEL_LOGO
        );
        return;
    }

    if next_arg == "--version" || next_arg == "-v" {
        cold_path();
        if args.len() > 1 {
            eprintln!(
                "{RED}KEEL ERROR{RESET}\nInvalid arguments\nUsage:\n  keel myfile.kl\n  keel [-v | --version]"
            );
            return;
        }
        println!("Keel {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let filename = &next_arg;

    let contents = fs::read_to_string(filename).unwrap_or_else(|_| {
        cold_path();
        eprintln!(
            "--------------\n{RED}KEEL RUNTIME ERROR:{RESET}\nCannot read {RED}{BOLD}{filename}{RESET}\n--------------",
        );
        std::process::exit(1);
    });

    #[cfg(debug_assertions)]
    {
        let next = args.next();
        if next == Some(String::from("--debug")) {
            let now = std::time::Instant::now();
            let (
                instructions,
                registers,
                mut arrays,
                instr_src,
                fn_registers,
                fn_dyn_libs,
                allocated_arg_count,
                allocated_call_depth,
                sources,
                struct_fields,
            ) = compile(contents, filename, true);
            println!("COMPILATION TIME: {:.2?}", now.elapsed());
            let now = std::time::Instant::now();
            vm::execute(
                &instructions,
                &mut RegisterFile(registers),
                &mut arrays,
                &ErrorCtx { instr_src, sources },
                &fn_registers,
                &fn_dyn_libs,
                &struct_fields,
                allocated_arg_count,
                allocated_call_depth,
            );
            println!(
                "EXECUTION TIME: {:.3}ms",
                now.elapsed().as_nanos() / 1_000_000
            );
            return;
        } else if next == Some(String::from("--debug-parser")) {
            compile(contents, filename, false);
            return;
        }
    }

    let (
        instructions,
        registers,
        mut arrays,
        instr_src,
        fn_registers,
        fn_dyn_libs,
        allocated_arg_count,
        allocated_call_depth,
        sources,
        struct_fields,
    ) = compile(contents, filename, false);
    vm::execute(
        &instructions,
        &mut RegisterFile(registers),
        &mut arrays,
        &ErrorCtx { instr_src, sources },
        &fn_registers,
        &fn_dyn_libs,
        &struct_fields,
        allocated_arg_count,
        allocated_call_depth,
    );
}
