#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

use crate::array_gc::alloc_array;
use crate::data::Data;
use crate::data::FALSE;
use crate::data::NULL;
use crate::display::format_data;
use crate::instr::Instr;
use crate::instr::LibFunc;
use crate::parser::parse;
use crate::parser_data::DynamicLibFn;
use crate::repl::repl;
use inline_colorization::*;
use mimalloc::MiMalloc;
use parser::*;
use std::fs;
use std::hint::cold_path;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[path = "./vm/array_gc.rs"]
mod array_gc;
#[path = "./benchmark.rs"]
mod benchmark;
#[path = "./data.rs"]
mod data;
#[path = "./util/display.rs"]
mod display;
#[path = "./util/errors.rs"]
mod errors;
#[path = "./parser/expr.rs"]
mod expr;
#[path = "./parser/functions/functions.rs"]
mod functions;
#[path = "./instr.rs"]
mod instr;
#[path = "./parser/functions/method_calls.rs"]
mod method_calls;
#[path = "./parser/parser.rs"]
mod parser;
#[path = "./parser/parser_data.rs"]
mod parser_data;
#[path = "./parser/registers.rs"]
mod registers;
#[path = "./repl.rs"]
mod repl;
#[path = "./vm/string_gc.rs"]
mod string_gc;
#[path = "./tests.rs"]
#[cfg(test)]
mod tests;
#[path = "./type_system.rs"]
mod type_system;
#[path = "./util/util.rs"]
mod util;
#[path = "./vm/vm.rs"]
mod vm;

/// Live long and prosper
fn main() {
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

    // #[cfg(debug_assertions)]
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
            ) = parse(&contents, filename, true);
            println!("COMPILATION TIME: {:.2?}", now.elapsed());
            let now = std::time::Instant::now();
            vm::execute(
                &instructions,
                &mut registers,
                &mut arrays,
                &instr_src,
                &sources,
                &fn_registers,
                &fn_dyn_libs,
                allocated_arg_count,
                allocated_call_depth,
            );
            println!(
                "EXECUTION TIME: {:.3}ms",
                now.elapsed().as_nanos() / 1000000
            );
            return;
        } else if next == Some(String::from("--debug-parser")) {
            parse(&contents, filename, false);
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
    ) = parse(&contents, filename, false);
    vm::execute(
        &instructions,
        &mut registers,
        &mut arrays,
        &instr_src,
        &sources,
        &fn_registers,
        &fn_dyn_libs,
        allocated_arg_count,
        allocated_call_depth,
    );
}
