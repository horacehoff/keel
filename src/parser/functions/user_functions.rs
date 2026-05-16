use crate::Instr;
use crate::check_args;
use crate::data::NULL;
use crate::errors::ErrType;
use crate::errors::throw_parser_error;
use crate::expr::Expr;
use crate::get_id;
use crate::parser::compile_expr;
use crate::parser_data::Ctx;
use crate::parser_data::FunctionImpl;
use crate::parser_data::State;
use crate::parser_data::Variable;
use crate::registers::alloc_register;
use crate::registers::for_each_read_reg;
use crate::registers::get_tgt_ids;
use crate::registers::move_to_id;
use crate::type_system::DataType;
use crate::type_system::check_poly;
use crate::type_system::infer_type;
use crate::type_system::track_returns;
use smol_str::SmolStr;
use std::rc::Rc;

pub fn handle_user_function(
    fn_name: &str,
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    args: &[Expr],
    markers: &(usize, usize),
    offset: u16,
    single_run: bool,
) -> u16 {
    let src = ctx.src;
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
    let fn_impl_idx = fn_impl_idx.unwrap_or_else(|| state.fns[function_id].impls.len() - 1);
    let loc = state.fns[function_id].impls[fn_impl_idx].loc;
    let args_loc_len = state.fns[function_id].impls[fn_impl_idx].args_loc.len();

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
        let tgt_id = state.fns[function_id].impls[fn_impl_idx].args_loc[i];

        let start_len = output.len();
        let arg_id = get_id(
            &args[i],
            v,
            ctx,
            state,
            output,
            Some(tgt_id),
            false,
            offset,
            single_run,
        );
        if output.len() != start_len {
            move_to_id(output, tgt_id);
        } else {
            output.push(Instr::Mov(arg_id, tgt_id))
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

    return return_register_id;
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
    let func = state.fns.get_mut(function_id).unwrap();
    func.impls.push(FunctionImpl {
        loc,
        args_loc: Box::from(args_loc),
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
