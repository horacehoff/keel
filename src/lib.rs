use crate::compiler::compile;
use crate::errors::BOLD;
use crate::errors::RED;
use crate::errors::RESET;
use crate::repl::repl;
use bumpalo::Bump;
use const_format::formatcp;
#[cfg(feature = "embed")]
use std::ffi::{CStr, CString, c_char};
use std::fs;
use std::hint::cold_path;
#[cfg(feature = "embed")]
use std::panic::catch_unwind;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(any(target_arch = "wasm32", feature = "embed"))]
mod captured_output;
#[path = "./compiler/compiler.rs"]
mod compiler;
mod data;
mod errors;
mod instr;
#[path = "./parser/parser.rs"]
mod parser;
mod repl;
#[cfg(test)]
mod tests;
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
    let bump = Bump::with_capacity(code.len() * 10);
    let (
        instructions,
        mut registers,
        mut pools,
        err_ctx,
        fn_registers,
        fn_dyn_libs,
        allocated_arg_count,
        allocated_call_depth,
        struct_fields,
        types,
    ) = compile(&code, "playground.kl", false, &bump);
    vm::execute(
        &instructions,
        &mut registers,
        &mut pools,
        &err_ctx,
        &fn_registers,
        &fn_dyn_libs,
        &struct_fields,
        &types,
        allocated_arg_count,
        allocated_call_depth,
    );
}

#[cfg(feature = "embed")]
#[unsafe(no_mangle)]
#[allow(clippy::missing_safety_doc)] // WIP
pub unsafe extern "C" fn keel_run(code: *const c_char) -> *mut c_char {
    std::panic::set_hook(Box::new(|_| {}));
    let code = unsafe { CStr::from_ptr(code) }.to_string_lossy().to_string();
    captured_output::CAPTURED_OUTPUT.with(|o| o.borrow_mut().clear());
    let _ = catch_unwind(|| {
        let bump = Bump::with_capacity(code.len() * 10);
        let (
            instructions,
            mut registers,
            mut pools,
            err_ctx,
            fn_registers,
            fn_dyn_libs,
            allocated_arg_count,
            allocated_call_depth,
            struct_fields,
            types,
        ) = compile(&code, "embedded.kl", false, &bump);
        vm::execute(
            &instructions,
            &mut registers,
            &mut pools,
            &err_ctx,
            &fn_registers,
            &fn_dyn_libs,
            &struct_fields,
            &types,
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

const ARGS: &str = formatcp!(
    "[{RED}ERROR{RESET}] Unrecognized command.

Usage: keel [file.kl] [args...]
       keel <COMMAND>

For more information, use the `--help` flag."
);

const HELP: &str = formatcp!(
    "  \x1b[34m// /\x1b[0m
 \x1b[34m// /\x1b[0m  keel {}
\x1b[34m// /\x1b[0m

by Horace Hoff - keel-lang.com

Usage: keel [file.kl] [args...]
       keel <COMMAND>

Arguments:
 - file.kl  A program to compile and run. If no argument is supplied, Keel starts the REPL.
 - args... Arguments that are forwarded to the program, accessible through `argv()`.

Commands:
+-----------+---------------------------+-----------------------------------------+
|  command  |           value           |               description               |
+-----------+---------------------------+-----------------------------------------+
| check     | file.kl                   | Compile and check `file.kl` without     |
|           |                           | running it                              |
| install   | author/repository[@tag]   | Install a package system-wide           |
| uninstall | [author/]repository[@tag] | Uninstall a package                     |
| list      |                           | List installed packages                 |
+-----------+---------------------------+-----------------------------------------+",
    env!("CARGO_PKG_VERSION")
);

#[allow(clippy::missing_panics_doc)]
pub fn main() {
    #[cfg(not(debug_assertions))]
    std::panic::set_hook(Box::new(|info| {
        eprintln!(
            "[{RED}ERROR{RESET}] {info}. Please report this at github.com/horacehoff/keel/issues."
        );
    }));

    let mut args = std::env::args().skip(1);

    if args.len() == 0 {
        cold_path();
        repl();
        return;
    }

    let argument = unsafe { args.next().unwrap_unchecked() };
    match argument.as_str() {
        "--help" | "-h" => {
            cold_path();
            println!("{HELP}");
        }
        "--version" | "-v" => {
            cold_path();
            println!("{}", formatcp!("Keel {}", env!("CARGO_PKG_VERSION")));
        }
        "install" | "uninstall" | "list" => {
            // keel-pkg commands
            cold_path();

            let keel_pkg_args = std::env::args_os().skip(1);
            let keel_pkg_path = std::env::current_exe()
                .expect("Report this bug at github.com/horacehoff/keel/issues")
                .parent()
                .expect("Report this bug at github.com/horacehoff/keel/issues")
                .join("keel-pkg")
                .with_extension(std::env::consts::EXE_EXTENSION);

            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                let e = std::process::Command::new(keel_pkg_path).args(keel_pkg_args).exec();
                eprintln!(
                    "Failed to redirect to keel-pkg: {e}. Report this bug at github.com/horacehoff/keel/issues"
                );
                std::process::exit(1);
            }
            #[cfg(not(unix))]
            {
                let status = std::process::Command::new(keel_pkg_path).args(keel_pkg_args).status().expect(
                    "Failed to redirect to keel-pkg. Report this bug at github.com/horacehoff/keel/issues",
                );
                std::process::exit(status.code().unwrap_or(1))
            }
        }
        "check" => {
            cold_path();
            if args.len() != 1 {
                cold_path();
                eprintln!("{ARGS}");
                std::process::exit(1);
            }
            let filename = unsafe { args.next().unwrap_unchecked() };
            let contents = fs::read_to_string(&filename).unwrap_or_else(|_| {
                cold_path();
                eprintln!("[{RED}ERROR{RESET}] Failed to read {RED}{BOLD}{filename}{RESET}.");
                std::process::exit(1);
            });
            let bump = Bump::with_capacity(contents.len() * 5);
            compile(&contents, &filename, false, &bump);
        }
        #[allow(clippy::case_sensitive_file_extension_comparisons)]
        argument if !argument.ends_with(".kl") => {
            cold_path();
            eprintln!("{ARGS}");
            std::process::exit(1);
        }
        file => {
            let contents = fs::read_to_string(file).unwrap_or_else(|_| {
                cold_path();
                eprintln!("[{RED}ERROR{RESET}] Failed to read {RED}{BOLD}{file}{RESET}.");
                std::process::exit(1);
            });
            let bump = Bump::with_capacity(contents.len() * 5);

            #[cfg(debug_assertions)]
            {
                let next = args.next();
                if next == Some(String::from("--debug")) {
                    let now = std::time::Instant::now();
                    let (
                        instructions,
                        mut registers,
                        mut pools,
                        err_ctx,
                        fn_registers,
                        fn_dyn_libs,
                        allocated_arg_count,
                        allocated_call_depth,
                        struct_fields,
                        types,
                    ) = compile(&contents, file, true, &bump);
                    println!("COMPILATION TIME: {:.2?}", now.elapsed());
                    let now = std::time::Instant::now();
                    vm::execute(
                        &instructions,
                        &mut registers,
                        &mut pools,
                        &err_ctx,
                        &fn_registers,
                        &fn_dyn_libs,
                        &struct_fields,
                        &types,
                        allocated_arg_count,
                        allocated_call_depth,
                    );
                    println!("EXECUTION TIME: {:.3}ms", now.elapsed().as_nanos() / 1_000_000);
                    return;
                }
            }
            let (
                instructions,
                mut registers,
                mut arrays,
                err_ctx,
                fn_registers,
                fn_dyn_libs,
                allocated_arg_count,
                allocated_call_depth,
                struct_fields,
                types,
            ) = compile(&contents, file, false, &bump);
            vm::execute(
                &instructions,
                &mut registers,
                &mut arrays,
                &err_ctx,
                &fn_registers,
                &fn_dyn_libs,
                &struct_fields,
                &types,
                allocated_arg_count,
                allocated_call_depth,
            );
        }
    }
}
