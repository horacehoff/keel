use crate::check_args;
use crate::compiler::Namespace;
use crate::compiler::get_id;
use crate::compiler_data::Ctx;
use crate::compiler_data::State;
use crate::compiler_data::Variable;
use crate::data::NULL;
use crate::errors::ErrType;
use crate::errors::throw_compiler_error;
#[cfg(target_arch = "wasm32")]
use crate::errors::wasm_error;
use crate::expr::Expr;
use crate::expr::Span;
use crate::fs_lib_functions::fs_lib_functions;
use crate::instr::Instr;
use crate::registers::free_register;
use crate::std_lib_functions::std_lib_functions;
use crate::type_system::DataType;
use crate::type_system::infer_type;
use crate::user_functions::handle_user_function;
use smol_strc::SmolStr;
use std::slice;

pub fn check_arg_type(
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    args: &[Expr],
    args_indexes: &[Span],
    arg_idx: usize,
    expected: &[DataType],
) {
    let inferred = infer_type(&args[arg_idx], v, ctx, state);
    let matches = if let DataType::Poly(polytype) = &inferred {
        polytype.iter().all(|x| expected.contains(x))
    } else {
        expected.contains(&inferred)
    };
    if !matches {
        throw_compiler_error(
            ctx.src,
            args_indexes[arg_idx],
            ErrType::InvalidArgType(expected, inferred),
        );
    }
}

fn walk_namespace_fn(root: &Namespace, path: &[SmolStr], fn_name: &str) -> Option<u16> {
    let mut current = root;
    for sub in path {
        current = current.children.iter().find(|n| n.name == *sub)?;
    }
    current
        .fns
        .iter()
        .find(|(n, _)| n.as_str() == fn_name)
        .map(|(_, id)| *id)
}

pub fn handle_functions(
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,

    // method call data
    args: &[Expr],
    namespace: &[SmolStr],
    markers: Span,
    args_indexes: &[Span],
    offset: u16,
    single_run: bool,
) -> Option<u16> {
    let src = ctx.src;
    let current_src_file = ctx.current_src_file;
    let len = namespace.len() - 1;
    let name = namespace[len].as_str();
    let namespace = &namespace[0..len];
    if namespace.is_empty() {
        return std_lib_functions(
            name,
            output,
            v,
            ctx,
            state,
            args,
            markers,
            args_indexes,
            offset,
            single_run,
        );
    } else if namespace == ["fs"] {
        #[cfg(target_arch = "wasm32")]
        wasm_error("WASM does not support the file system library");

        fs_lib_functions(
            name,
            output,
            v,
            ctx,
            state,
            args,
            markers,
            args_indexes,
            offset,
            single_run,
        );
    } else if let Some((fn_args, returns_null, dyn_id)) = state
        .dyn_libs
        .iter()
        .find(|l| l.name == namespace[0])
        .and_then(|lib| lib.fns.iter().find(|x| x.name == name))
        .map(|sig| (sig.args.clone(), sig.return_type == DataType::Null, sig.id))
    {
        check_args!(args, fn_args.len(), name, src, markers);
        for (i, a) in fn_args.iter().enumerate() {
            check_arg_type(v, ctx, state, args, args_indexes, i, slice::from_ref(a));
        }

        for arg in args {
            let arg_id = get_id(arg, v, ctx, state, output, None, false, offset, single_run);
            output.push(Instr::StoreFuncArg(arg_id));
            // This may break stuff
            free_register(
                arg_id,
                state.free_registers,
                v,
                state.const_registers,
                &state.reserved_registers,
            );
            *state.allocated_arg_count += 1;
        }

        let register_id = if returns_null {
            0
        } else {
            state.registers.push(NULL);
            (state.registers.len() - 1) as u16
        };
        output.push(Instr::CallDynamicLibFunc(dyn_id, register_id));
        state.instr_src.push((
            Instr::CallDynamicLibFunc(dyn_id, register_id),
            markers,
            current_src_file,
        ));
    } else if let Some(fn_id) = walk_namespace_fn(state.namespace, namespace, name) {
        return Some(handle_user_function(
            name,
            fn_id as usize,
            output,
            v,
            ctx,
            state,
            args,
            markers,
            offset,
            single_run,
        ));
    } else {
        throw_compiler_error(
            src,
            markers,
            ErrType::UnknownNamespace(
                namespace
                    .iter()
                    .map(|x| (*x).to_string())
                    .collect::<Vec<String>>()
                    .join("::")
                    .as_str(),
            ),
        );
    }
    None
}
