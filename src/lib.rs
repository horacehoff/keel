#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![warn(clippy::implicit_clone)]
#![warn(clippy::redundant_clone)]

use crate::errors::ErrorCtx;
use crate::parser::parse;
use crate::repl::repl;
use inline_colorization::*;
#[cfg(feature = "embed")]
use std::ffi::{CStr, CString, c_char};
use std::fs;
use std::hint::cold_path;
#[cfg(feature = "embed")]
use std::panic::catch_unwind;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[path = "./vm/array_gc.rs"]
mod array_gc;
#[path = "./benchmark.rs"]
mod benchmark;
#[cfg(any(target_arch = "wasm32", feature = "embed"))]
mod captured_output;
#[path = "./data.rs"]
mod data;
#[path = "./util/display.rs"]
mod display;
#[path = "./util/errors.rs"]
mod errors;
#[path = "./parser/expr.rs"]
mod expr;
#[path = "./parser/functions/fs_lib_functions.rs"]
mod fs_lib_functions;
#[path = "./parser/functions/functions.rs"]
mod functions;
#[path = "./instr.rs"]
mod instr;
#[path = "./parser/functions/methods.rs"]
mod methods;
#[path = "./parser/parser.rs"]
mod parser;
#[path = "./parser/parser_data.rs"]
mod parser_data;
#[path = "./parser/registers.rs"]
mod registers;
#[path = "./repl.rs"]
mod repl;
#[path = "./parser/functions/std_lib_functions.rs"]
mod std_lib_functions;
#[path = "./parser/functions/std_lib_methods.rs"]
mod std_lib_methods;
#[path = "./vm/string_gc.rs"]
mod string_gc;
#[path = "./tests.rs"]
#[cfg(test)]
mod tests;
#[path = "./type_system.rs"]
mod type_system;
#[path = "./parser/functions/user_functions.rs"]
mod user_functions;
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
pub fn run(code: &str) {
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
    ) = parse(code, "playground.kl", false);
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
    let code = &unsafe { CStr::from_ptr(code) }
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
        ) = parse(code, "embedded.kl", false);
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
        eprintln!("{color_red}KEEL ERROR{color_reset}\n{info}");
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
            "{}\nKeel is a fast, statically-typed interpreted language that aims to combine Rust-like syntax with Python's ease-of-use.\n\nUsage:\n  keel [-v | --version]\n  keel file.kl [--bench [--verbose]]",
            util::KEEL_LOGO
        );
        return;
    }

    if next_arg == "--version" || next_arg == "-v" {
        cold_path();
        if args.len() > 1 {
            eprintln!(
                "{color_red}KEEL ERROR{color_reset}\nInvalid arguments\nUsage:\n  keel -v\n  keel program.kl [--bench [--verbose]]"
            );
            return;
        }
        println!("Keel {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if next_arg == "--bench" {
        cold_path();
        crate::benchmark::benchmark();
        return;
    }

    let filename = &next_arg;

    let contents = fs::read_to_string(filename).unwrap_or_else(|_| {
        cold_path();
        eprintln!(
            "--------------\n{color_red}KEEL RUNTIME ERROR:{color_reset}\nCannot read {color_bright_red}{style_bold}{filename}{style_reset}{color_reset}\n--------------",
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
                mut registers,
                mut arrays,
                instr_src,
                fn_registers,
                fn_dyn_libs,
                allocated_arg_count,
                allocated_call_depth,
                sources,
                struct_fields,
            ) = parse(contents, filename, true);
            println!("COMPILATION TIME: {:.2?}", now.elapsed());
            let now = std::time::Instant::now();
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
            println!(
                "EXECUTION TIME: {:.3}ms",
                now.elapsed().as_nanos() / 1000000
            );
            return;
        } else if next == Some(String::from("--debug-parser")) {
            parse(contents, filename, false);
            return;
        }
    }

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
    ) = parse(contents, filename, false);
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
