use crate::Instr;
use crate::check_args;
use crate::check_args_range;
use crate::data::Data;
use crate::data::NULL;
use crate::errors::ErrType;
use crate::errors::throw_parser_error;
use crate::get_id;
use crate::instr::LibFunc;
use crate::instr::LibFuncVoid;
use crate::parser::Expr;
use crate::parser::alloc_register;
use crate::parser::compile_expr;
use crate::parser::for_each_read_reg;
use crate::parser::free_register;
use crate::parser::get_tgt_ids;
use crate::parser::move_to_id;
use crate::parser_data::Ctx;
use crate::parser_data::FunctionImpl;
use crate::parser_data::State;
use crate::parser_data::Variable;
use crate::type_system::DataType;
use crate::type_system::check_poly;
use crate::type_system::infer_type;
use crate::type_system::track_returns;
use inline_colorization::*;
use smol_str::SmolStr;
use smol_str::ToSmolStr;
use std::rc::Rc;
use std::slice;

pub fn handle_functions(
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,

    // method call data
    args: &[Expr],
    namespace: &[SmolStr],
    markers: &(usize, usize),
    args_indexes: &[(usize, usize)],
    offset: u16,
    single_run: bool,
) -> Option<u16> {
    let src = ctx.src;
    let current_src_file = ctx.current_src_file;

    let mut check_arg_type = |arg_idx: usize, expected: &[DataType]| {
        let infered = infer_type(&args[arg_idx], v, state.fns, src, state.dyn_libs);
        if !{
            if let DataType::Poly(polytype) = &infered {
                polytype.iter().all(|x| expected.contains(x))
            } else {
                expected.contains(&infered)
            }
        } {
            throw_parser_error(
                src,
                &args_indexes[arg_idx],
                ErrType::Custom(
                format_args!(
                    "Expected {color_bright_blue}{style_bold}{}{color_reset}{style_reset}, found {color_bright_red}{style_bold}{}{color_reset}{style_reset}",
                    expected
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<String>>()
                        .join("{color_reset}{style_reset} or {color_bright_blue}{style_bold}"),
                    infered
                ).to_smolstr())
            );
        }
    };
    let len = namespace.len() - 1;
    let name = namespace[len].as_str();
    let namespace = &namespace[0..len];
    if namespace.is_empty() {
        match name {
            "print" => {
                for arg in args {
                    let id = get_id(arg, v, ctx, state, output, None, false, offset, single_run);
                    output.push(Instr::Print(id));
                    free_register(id, state.free_registers, v, state.const_registers);
                }
            }
            "type" => {
                check_args!(args, 1, name, src, markers);
                let infered = infer_type(&args[0], v, state.fns, src, state.dyn_libs);
                state.registers.push(Data::p_str(
                    &infered.to_string(),
                    &mut state.pools.string_pool,
                ));
            }
            "float" => {
                check_args!(args, 1, name, src, markers);
                check_arg_type(0, &[DataType::String, DataType::Int]);
                let id = get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                );
                free_register(id, state.free_registers, v, state.const_registers);
                output.push(Instr::CallLibFunc(
                    LibFunc::Float,
                    id,
                    alloc_register(state.registers, state.free_registers),
                ));
                state
                    .instr_src
                    .push((*output.last().unwrap(), *markers, current_src_file));
            }
            "int" => {
                check_args!(args, 1, name, src, markers);
                check_arg_type(0, &[DataType::String, DataType::Float]);
                let id = get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                );
                free_register(id, state.free_registers, v, state.const_registers);
                output.push(Instr::CallLibFunc(
                    LibFunc::Int,
                    id,
                    alloc_register(state.registers, state.free_registers),
                ));
                state
                    .instr_src
                    .push((*output.last().unwrap(), *markers, current_src_file));
            }
            "str" => {
                check_args!(args, 1, name, src, markers);
                let id = get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                );
                free_register(id, state.free_registers, v, state.const_registers);
                output.push(Instr::CallLibFunc(
                    LibFunc::Str,
                    id,
                    alloc_register(state.registers, state.free_registers),
                ));
            }
            "bool" => {
                check_args!(args, 1, name, src, markers);
                check_arg_type(0, &[DataType::String]);
                let id = get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                );
                free_register(id, state.free_registers, v, state.const_registers);
                output.push(Instr::CallLibFunc(
                    LibFunc::Bool,
                    id,
                    alloc_register(state.registers, state.free_registers),
                ));
                state
                    .instr_src
                    .push((*output.last().unwrap(), *markers, current_src_file));
            }
            "input" => {
                check_args_range!(args, 0, 1, name, src, markers);
                check_arg_type(0, &[DataType::String]);
                let id = if args.is_empty() {
                    state
                        .registers
                        .push(Data::p_str("", &mut state.pools.string_pool));
                    (state.registers.len() - 1) as u16
                } else {
                    get_id(
                        &args[0], v, ctx, state, output, None, false, offset, single_run,
                    )
                };
                free_register(id, state.free_registers, v, state.const_registers);
                output.push(Instr::CallLibFunc(
                    LibFunc::Input,
                    id,
                    alloc_register(state.registers, state.free_registers),
                ));
            }
            "range" => {
                check_args_range!(args, 1, 2, name, src, markers);
                check_arg_type(0, &[DataType::Int]);
                if args.len() != 1 {
                    check_arg_type(1, &[DataType::Int]);
                }

                let id_first_arg = get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                );
                let source_reg_id = if args.len() == 1 {
                    id_first_arg
                } else {
                    let id_second_arg = get_id(
                        &args[1], v, ctx, state, output, None, false, offset, single_run,
                    );
                    output.push(Instr::StoreFuncArg(id_first_arg));
                    *state.allocated_arg_count += 1;
                    id_second_arg
                };
                free_register(id_first_arg, state.free_registers, v, state.const_registers);
                free_register(
                    source_reg_id,
                    state.free_registers,
                    v,
                    state.const_registers,
                );
                output.push(Instr::CallLibFunc(
                    LibFunc::Range,
                    source_reg_id,
                    alloc_register(state.registers, state.free_registers),
                ));
            }
            "the_answer" => {
                check_args!(args, 0, name, src, markers);
                output.push(Instr::CallLibFunc(
                    LibFunc::TheAnswer,
                    0,
                    alloc_register(state.registers, state.free_registers),
                ));
            }
            "argv" => {
                check_args!(args, 0, name, src, markers);
                output.push(Instr::CallLibFunc(
                    LibFunc::Argv,
                    0,
                    alloc_register(state.registers, state.free_registers),
                ));
            }
            "exit" => {
                check_args_range!(args, 0, 1, name, src, markers);
                let halt_code = if !args.is_empty() {
                    check_arg_type(0, &[DataType::Int]);
                    get_id(
                        &args[0], v, ctx, state, output, None, false, offset, single_run,
                    )
                } else {
                    0
                };
                output.push(Instr::Halt(halt_code));
            }
            fn_name => {
                // Lookup function by name in function registry
                // Registry is (fn_name, fn_args, fn_code, fn_data (per implementation: loc, args_loc, arg_types) )
                let function_id = state
                    .fns
                    .iter_mut()
                    .position(|func| func.name == fn_name)
                    .unwrap_or_else(|| {
                        throw_parser_error(src, markers, ErrType::UnknownFunction(fn_name));
                    });
                // Retrieve list of args, code, and function data (loc, args_loc, arg_types)
                let fn_id = state.fns[function_id].id;
                let is_recursive = state.fns[function_id].is_recursive;
                let fn_returns_void = state.fns[function_id].returns_void;

                // Check if the arguments are correct
                let args_len = state.fns[function_id].args.len();
                check_args!(args, args_len, fn_name, src, markers);

                // Infer arg types
                let infered_arg_types = args
                    .iter()
                    .map(|x| infer_type(x, v, state.fns, src, state.dyn_libs))
                    .collect::<Vec<DataType>>();

                // Try to check if function has already been compiled for these specific arg types
                let fn_impl_idx = state.fns[function_id]
                    .impls
                    .iter()
                    .position(|fn_impl| *fn_impl.arg_types == infered_arg_types);

                if fn_impl_idx.is_none() {
                    // If it hasn't, compile it (which adds it to the function's implementation list)

                    // Clone only when actually compiling a new specialisation
                    let fn_args: Box<[SmolStr]> = state.fns[function_id].args.clone();
                    let fn_code: Rc<[Expr]> = Rc::clone(&state.fns[function_id].code);
                    compile_function(
                        output,
                        v,
                        ctx,
                        state,
                        function_id,
                        &fn_args,
                        fn_name,
                        &infered_arg_types,
                        args,
                        &fn_code,
                        fn_id,
                        is_recursive,
                        offset,
                    );
                }
                // Re-derive index after possible mutation
                let fn_impl_idx =
                    fn_impl_idx.unwrap_or_else(|| state.fns[function_id].impls.len() - 1);
                let loc = state.fns[function_id].impls[fn_impl_idx].loc;
                let args_loc: Vec<u16> =
                    state.fns[function_id].impls[fn_impl_idx].args_loc.to_vec();

                let saveframe_loc = output.len();
                let callsite_id = if is_recursive {
                    let id = state.fn_registers.len() as u16;
                    state.fn_registers.push(Vec::new());
                    output.push(Instr::SaveFrame(0, 0, 0));
                    *state.allocated_call_depth += 2;
                    Some(id)
                } else {
                    None
                };
                // Move evaluated call args into the expected arg slots
                for (x, tgt_id) in args_loc.iter().enumerate() {
                    let start_len = output.len();

                    let arg_id = get_id(
                        &args[x],
                        v,
                        ctx,
                        state,
                        output,
                        Some(*tgt_id),
                        false,
                        offset,
                        single_run,
                    );
                    if output.len() != start_len {
                        move_to_id(output, *tgt_id);
                    } else {
                        output.push(Instr::Mov(arg_id, *tgt_id))
                    }
                }
                if !is_recursive {
                    state
                        .fn_registers
                        .get_mut(fn_id as usize)
                        .unwrap()
                        .extend(get_tgt_ids(&output[saveframe_loc..]));
                }

                let return_register_id = if !fn_returns_void {
                    alloc_register(state.registers, state.free_registers)
                } else {
                    0
                };
                if is_recursive {
                    output.push(Instr::CallFuncRecursive(loc, return_register_id));
                } else {
                    output.push(Instr::CallFunc(loc, return_register_id));
                    *state.allocated_call_depth += 2;
                }

                if is_recursive {
                    output[saveframe_loc] = Instr::SaveFrame(
                        (output.len() - 1 - saveframe_loc) as u16,
                        return_register_id,
                        callsite_id.unwrap(),
                    );
                }

                return Some(return_register_id);
            }
        }
    } else if namespace == ["fs"] {
        match name {
            "read" => {
                check_args!(args, 1, name, src, markers);
                check_arg_type(0, &[DataType::String]);
                let id = get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                );
                free_register(id, state.free_registers, v, state.const_registers);
                output.push(Instr::CallLibFunc(
                    LibFunc::FsRead,
                    id,
                    alloc_register(state.registers, state.free_registers),
                ));
                state
                    .instr_src
                    .push((*output.last().unwrap(), *markers, current_src_file));
            }
            "exists" => {
                check_args!(args, 1, name, src, markers);
                check_arg_type(0, &[DataType::String]);
                let id = get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                );
                free_register(id, state.free_registers, v, state.const_registers);
                output.push(Instr::CallLibFunc(
                    LibFunc::FsExists,
                    id,
                    alloc_register(state.registers, state.free_registers),
                ));
                state
                    .instr_src
                    .push((*output.last().unwrap(), *markers, current_src_file));
            }
            "write" => {
                check_args!(args, 2, name, src, markers);
                check_arg_type(0, &[DataType::String]);
                check_arg_type(1, &[DataType::String]);
                let filepath = get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                );
                let contents = get_id(
                    &args[1], v, ctx, state, output, None, false, offset, single_run,
                );
                free_register(filepath, state.free_registers, v, state.const_registers);
                free_register(contents, state.free_registers, v, state.const_registers);
                output.push(Instr::CallLibFuncVoid(
                    LibFuncVoid::FsWrite,
                    filepath,
                    contents,
                ));
                state
                    .instr_src
                    .push((*output.last().unwrap(), *markers, current_src_file));
            }
            "append" => {
                check_args!(args, 2, name, src, markers);
                check_arg_type(0, &[DataType::String]);
                check_arg_type(1, &[DataType::String]);
                let filepath = get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                );
                let contents = get_id(
                    &args[1], v, ctx, state, output, None, false, offset, single_run,
                );
                free_register(filepath, state.free_registers, v, state.const_registers);
                free_register(contents, state.free_registers, v, state.const_registers);
                output.push(Instr::CallLibFuncVoid(
                    LibFuncVoid::FsAppend,
                    filepath,
                    contents,
                ));
                state
                    .instr_src
                    .push((*output.last().unwrap(), *markers, current_src_file));
            }
            "delete" => {
                check_args!(args, 1, name, src, markers);
                check_arg_type(0, &[DataType::String]);
                let path = get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                );
                free_register(path, state.free_registers, v, state.const_registers);
                output.push(Instr::CallLibFuncVoid(LibFuncVoid::FsDelete, path, 0));
                state
                    .instr_src
                    .push((*output.last().unwrap(), *markers, current_src_file));
            }
            "delete_dir" => {
                check_args!(args, 1, name, src, markers);
                check_arg_type(0, &[DataType::String]);
                let path = get_id(
                    &args[0], v, ctx, state, output, None, false, offset, single_run,
                );
                free_register(path, state.free_registers, v, state.const_registers);
                output.push(Instr::CallLibFuncVoid(LibFuncVoid::FsDeleteDir, path, 0));
                state
                    .instr_src
                    .push((*output.last().unwrap(), *markers, current_src_file));
            }
            name => {
                throw_parser_error(src, markers, ErrType::UnknownFunction(name));
            }
        }
    } else if let Some((fn_args, returns_void, dyn_id)) = state
        .dyn_libs
        .iter()
        .find(|l| l.name == namespace[0])
        .and_then(|lib| lib.fns.iter().find(|x| x.name == name))
        .map(|sig| (sig.args.clone(), sig.return_type == DataType::Null, sig.id))
    {
        check_args!(args, fn_args.len(), name, src, markers);
        for (i, a) in fn_args.iter().enumerate() {
            check_arg_type(i, slice::from_ref(a));
        }

        for arg in args {
            let arg_id = get_id(arg, v, ctx, state, output, None, false, offset, single_run);
            output.push(Instr::StoreFuncArg(arg_id));
            // This may break stuff
            free_register(arg_id, state.free_registers, v, state.const_registers);
            *state.allocated_arg_count += 1;
        }

        let register_id = if returns_void {
            0u16
        } else {
            state.registers.push(NULL);
            (state.registers.len() - 1) as u16
        };
        output.push(Instr::CallDynamicLibFunc(dyn_id, register_id));
        state.instr_src.push((
            Instr::CallDynamicLibFunc(dyn_id, register_id),
            *markers,
            current_src_file,
        ));
    } else {
        throw_parser_error(
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

fn compile_function(
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    function_id: usize,
    fn_args: &[SmolStr],
    fn_name: &str,
    infered_arg_types: &[DataType],
    _args: &[Expr],
    fn_code: &[Expr],
    fn_id: u16,
    is_recursive: bool,
    offset: u16,
) {
    let src = ctx.src;
    let current_src_file = ctx.current_src_file;

    // Use the function's own source file for error reporting inside the function body
    let fn_src_file = state.fns[function_id].src_file;

    let fn_src: (&str, &str) = if fn_src_file != current_src_file {
        (
            &state.sources[fn_src_file as usize].0.clone(),
            &state.sources[fn_src_file as usize].1.clone(),
        )
    } else {
        (src.0, src.1)
    };

    // Local vector vars and recorded_types to allow the inner body to type-check correctly
    let mut v_temp: Vec<Variable> = fn_args
        .iter()
        .enumerate()
        .map(|(i, x)| {
            // Allocate a registers slot for each func arg
            state.registers.push(NULL);
            Variable {
                name: x.clone(),
                register_id: (state.registers.len() - 1) as u16,
                infered_type: infered_arg_types[i].clone(),
            }
        })
        .collect();

    // Get the arg destination ids
    let args_loc = v_temp.iter().map(|x| x.register_id).collect::<Vec<u16>>();

    // Temporarily jump over function to prevent executing it right now
    // This is a placeholder that's modified later on
    output.push(Instr::Jmp(0));
    let jump_idx = output.len() - 1;

    // Record start location for the compiled func body
    let fn_start = output.len();
    let loc = fn_start as u16 + offset;

    let v_len_before_args = v.len();
    infered_arg_types
        .iter()
        .enumerate()
        .for_each(|(i, infered_type)| {
            // 0 => placeholder id, it's never used
            v.push(Variable {
                name: fn_args[i].clone(),
                register_id: 0,
                infered_type: infered_type.clone(),
            });
        });
    let fn_type = track_returns(fn_code, v, state.fns, fn_src, fn_name, state.dyn_libs);
    let return_type = if !fn_type.is_empty() {
        // If function returns anything, check if it returns the same thing each time
        check_poly(DataType::Poly(Box::from(fn_type)))
    } else {
        // If function doesn't return anything, return nothing
        DataType::Null
    };

    v.truncate(v_len_before_args);

    // Add this func specialization to the func's metadata, storing start location, location of args, and infered arg types
    state
        .fns
        .get_mut(function_id)
        .unwrap()
        .impls
        .push(FunctionImpl {
            loc,
            args_loc: Box::from(args_loc),
            arg_types: Box::from(infered_arg_types),
            return_type,
        });

    // Compile the function into instructions using local vars
    let parsed = compile_expr(
        fn_code,
        &mut v_temp,
        Ctx {
            is_parsing_recursive: is_recursive,
            src: fn_src,
            current_src_file: fn_src_file,
            ..ctx
        },
        state,
        offset + output.len() as u16,
        false,
    );

    if is_recursive {
        let all_written_regs: Vec<u16> = get_tgt_ids(&parsed);

        // For each recursive call, only save registers that are read between that call's return and the end of the function
        for (pos, instr) in parsed.iter().enumerate() {
            if matches!(instr, Instr::CallFuncRecursive(_, _)) {
                // Walk backwards to find this call's SaveFrame and its callsite_id
                let callsite_id = parsed[..pos]
                    .iter()
                    .rev()
                    .find_map(|i| match i {
                        Instr::SaveFrame(_, _, cid) => Some(*cid),
                        _ => None,
                    })
                    .unwrap();

                let mut live_regs: Vec<u16> = Vec::new();
                for after_instr in &parsed[pos + 1..] {
                    for_each_read_reg(*after_instr, |reg| {
                        if all_written_regs.contains(&reg) {
                            live_regs.push(reg);
                        }
                    });
                }
                live_regs.sort_unstable();
                live_regs.dedup();
                *state.fn_registers.get_mut(callsite_id as usize).unwrap() = live_regs;
            }
        }
    } else {
        state
            .fn_registers
            .get_mut(fn_id as usize)
            .unwrap()
            .extend(get_tgt_ids(&parsed));
    }

    output.extend(parsed);

    output.push(Instr::VoidReturn);

    // Fix the placeholder Jmp(0) to skip over the function body
    *output.get_mut(jump_idx).unwrap() = Instr::Jmp((output.len() - fn_start + 1) as u16);
}
