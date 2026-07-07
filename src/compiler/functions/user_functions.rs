use crate::compiler::UnwrapId;
use crate::compiler::compile_expr;
use crate::compiler_data::Ctx;
use crate::compiler_data::FunctionImpl;
use crate::compiler_data::State;
use crate::compiler_data::Variable;
use crate::data::NULL;
use crate::expr::Expr;
use crate::expr::Span;
use crate::instr::Instr;
use crate::registers::get_tgt_ids;
use crate::registers::move_to_id;
use crate::type_system::DataType;
use crate::type_system::can_reach;
use crate::type_system::track_returns;
use crate::util::check_args;
use smol_strc::SmolStr;
use std::collections::HashSet;
use std::rc::Rc;

pub fn handle_user_function(
    fn_name: &str,
    fn_id: usize,
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    tgt_id: Option<u16>,
    args: &[Expr],
    markers: Span,
    args_indexes: &[Span],
) -> Option<u16> {
    // let fn_id = state
    //     .namespace
    //     .fns
    //     .iter_mut()
    //     .find(|(func_name, _)| func_name == fn_name)
    //     .unwrap_or_else(|| {
    //         throw_parser_error(src, markers, ErrType::UnknownFunction(fn_name));
    //     })
    //     .1 as usize;

    // Lazily resolve mutual recursion the first time this function is compiled
    let is_recursive = if let Some(is_recursive) = state.fns[fn_id].is_recursive {
        is_recursive
    } else {
        let name = state.fns[fn_id].name.clone();
        let mut visited = HashSet::new();
        visited.insert(name.clone());
        let is_recursive = can_reach(&name, &name, state.fns, &mut visited);
        state.fns[fn_id].is_recursive = Some(is_recursive);
        is_recursive
    };

    let fn_returns_null = state.fns[fn_id].returns_null;

    // Check if the arguments are correct
    let args_len = state.fns[fn_id].args.len();
    check_args(args, args_len, fn_name, ctx.src, markers);

    //This inlines dylib wrappers
    // Actual general function inlining is coming soon
    if state.fns[fn_id].code.len() == 1
        && let Expr::ReturnVal(ret) = &state.fns[fn_id].code[0]
        && let Some(Expr::FunctionCall(call_args, namespace, _, _)) = &**ret
        && namespace.len() >= 2
        && call_args.len() == args_len
        && call_args
            .iter()
            .zip(state.fns[fn_id].args.iter())
            .all(|(e, p)| matches!(e, Expr::Var(n, _) if n == p))
        && let Some(fn_sig) = state
            .dyn_libs
            .iter()
            .find(|lib| lib.name == namespace[namespace.len() - 2])
            .and_then(|lib| {
                lib.fns
                    .iter()
                    .find(|f| f.name == namespace[namespace.len() - 1])
            })
    {
        let dyn_id = fn_sig.id;
        let returns_null = fn_sig.return_type == DataType::Null;
        let expected_arg_types = fn_sig.args.clone();
        for (i, arg) in args.iter().enumerate() {
            let inferred = arg.infer_type(v, ctx, state);
            inferred.expect(&expected_arg_types[i], ctx.src, args_indexes[i]);
        }
        for arg in args {
            let arg_id = arg
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            output.push(Instr::StoreFuncArg(arg_id));
            state.free_reg(arg_id, v);
            *state.allocated_arg_count += 1;
        }

        let register_id = if returns_null {
            0
        } else {
            state.alloc_reg_tgt(tgt_id)
        };
        output.push(Instr::CallDynamicLibFunc(dyn_id, register_id));
        state.instr_src.push((
            Instr::CallDynamicLibFunc(dyn_id, register_id),
            markers,
            ctx.current_src_file,
        ));
        return Some(register_id);
    }

    // Infer arg types
    let infered_arg_types = args
        .iter()
        .map(|arg| arg.infer_type(v, ctx, state))
        .collect::<Vec<DataType>>();

    // Try to check if function has already been compiled for these specific arg types
    let fn_impl_idx = state.fns[fn_id]
        .impls
        .iter()
        .position(|fn_impl| *fn_impl.arg_types == infered_arg_types);

    if fn_impl_idx.is_none() {
        // If it hasn't, compile it (which adds it to the function's implementation list)

        // Clone only when actually compiling a new specialisation
        let fn_args: Box<[SmolStr]> = state.fns[fn_id].args.clone();
        let fn_code: Rc<[Expr]> = Rc::clone(&state.fns[fn_id].code);
        compile_function(
            output,
            v,
            ctx,
            state,
            fn_id,
            &fn_args,
            fn_name,
            &infered_arg_types,
            args,
            &fn_code,
            fn_id as u16,
            is_recursive,
        );
    }
    // Re-derive index after possible mutation
    let fn_impl_idx = fn_impl_idx.unwrap_or_else(|| state.fns[fn_id].impls.len() - 1);
    let loc = state.fns[fn_id].impls[fn_impl_idx].loc;
    let args_loc_len = state.fns[fn_id].impls[fn_impl_idx].args_loc.len();

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
    #[allow(clippy::needless_range_loop)]
    for i in 0..args_loc_len {
        let tgt_id = state.fns[fn_id].impls[fn_impl_idx].args_loc[i];

        if matches!(infered_arg_types[i], DataType::Fn(_)) {
            continue;
        }

        let start_len = output.len();
        let arg_id = args[i]
            .compile(v, ctx, state, output, Some(tgt_id), false, true)
            .unwrap_id();
        if output.len() == start_len {
            output.push(Instr::Mov(arg_id, tgt_id));
        } else {
            move_to_id(output, tgt_id);
        }
    }
    if !is_recursive {
        state
            .fn_registers
            .get_mut(fn_id)
            .unwrap()
            .extend(get_tgt_ids(&output[saveframe_loc..]));
    }

    let return_register_id = if fn_returns_null {
        0
    } else {
        state.alloc_reg_tgt(tgt_id)
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

    if fn_returns_null {
        None
    } else {
        Some(return_register_id)
    }
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
) {
    let src = ctx.src;
    let current_src_file = ctx.current_src_file;

    // Errors inside of the function body are reported using the function's file
    let fn_src_file = state.fns[function_id].src_file;

    let fn_src: (&str, &str) = if fn_src_file == current_src_file {
        (src.0, src.1)
    } else {
        (
            &state.sources[fn_src_file as usize].0.clone(),
            &state.sources[fn_src_file as usize].1.clone(),
        )
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
                var_type: infered_arg_types[i].clone(),
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
    let loc = fn_start as u16 + ctx.offset;

    let v_len_before_args = v.len();
    let fn_len = state.namespace.fns.len();
    infered_arg_types
        .iter()
        .enumerate()
        .for_each(|(i, infered_type)| {
            if let DataType::Fn(fn_id) = infered_type {
                state.namespace.fns.push((fn_args[i].clone(), *fn_id));
                v.push(Variable {
                    name: fn_args[i].clone(),
                    register_id: 0,
                    var_type: DataType::Fn(*fn_id),
                });
            } else {
                // 0 => placeholder id, it's never used
                v.push(Variable {
                    name: fn_args[i].clone(),
                    register_id: 0,
                    var_type: infered_type.clone(),
                });
            }
        });
    let fn_type = track_returns(fn_code, v, ctx, state, fn_name);
    let return_type = if fn_type.is_empty() {
        // If function doesn't return anything, return nothing
        DataType::Null
    } else {
        // If function returns anything, check if it returns the same thing each time
        DataType::Poly(Box::from(fn_type)).check_poly()
    };

    v.truncate(v_len_before_args);

    // Add this func specialization to the func's metadata
    let func = state.fns.get_mut(function_id).unwrap();
    func.impls.push(FunctionImpl {
        loc,
        args_loc: Box::from(args_loc.as_slice()),
        arg_types: Box::from(infered_arg_types),
    });
    // Cache the return type
    if !func
        .return_type_cache
        .iter()
        .any(|(args, _)| **args == *infered_arg_types)
    {
        func.return_type_cache
            .push((Box::from(infered_arg_types), return_type));
    }

    // Compile the function into instructions using local vars
    let parsed = compile_expr(
        fn_code,
        &mut v_temp,
        Ctx {
            is_parsing_recursive: is_recursive,
            src: fn_src,
            current_src_file: fn_src_file,
            single_run: false,
            offset: ctx.offset + output.len() as u16,
            ..ctx
        },
        state,
    );
    state.namespace.fns.truncate(fn_len);

    let mut reserved_registers = get_tgt_ids(&parsed);
    reserved_registers.extend(args_loc);
    for instr in &parsed {
        match instr {
            Instr::CloneArray(template_reg, _, _)
            | Instr::CloneStruct(template_reg, _)
            | Instr::CloneMap(template_reg, _) => {
                reserved_registers.push(*template_reg);
            }
            _ => {}
        }
    }
    reserved_registers.sort_unstable();
    reserved_registers.dedup();
    state.reserved_registers.extend(reserved_registers);
    state
        .free_registers
        .retain(|reg| !state.reserved_registers.contains(reg));

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
                    after_instr.for_each_read_reg(|reg| {
                        if all_written_regs.binary_search(&reg).is_ok() {
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
