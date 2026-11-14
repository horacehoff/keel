use crate::{
    ast::{Expr, FunctionCallExpr},
    compiler::{
        DataType,
        codegen::compile_reg_window,
        compiler_errors::{error_expected_function, error_function_arg_invalid_type},
        functions::{
            builtin::{BuiltIn, DefaultArg, ErrorSpan, Lowering, ReturnType, SignatureType},
            compile_function_impl,
        },
        registers::UnwrapId,
        state::{Ctx, State},
        types::{fn_args_match, fn_matches_signature, resolve_function_return_type},
    },
    data::{Data, FALSE},
    instr::Instr,
};

fn builtin_push<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    _tgt_id: Option<u16>,
) -> Option<u16> {
    let receiver = &function_call.args[0];
    let element = &function_call.args[1];
    if receiver.infer_type(ctx, state) == DataType::Array(None)
        && let Expr::Var(var_name, _) = receiver
    {
        let element_type = element.infer_type(ctx, state);
        if let Some(var) = state.find_var_mut(var_name.get_name()) {
            var.var_type = DataType::Array(Some(Box::new(element_type)));
        }
    }
    let src = receiver.compile(ctx, state, output, None, false, true).unwrap_id();
    let element_id = element.compile(ctx, state, output, None, false, true).unwrap_id();
    output.push(Instr::Push(src, element_id));
    state.free_reg(src);
    None
}

fn builtin_map_return_type<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
) -> DataType {
    let receiver_type = function_call.args[0].infer_type(ctx, state);
    if receiver_type == DataType::String {
        return DataType::String;
    }
    let DataType::Fn(fn_id) = function_call.args[1].infer_type(ctx, state) else { unreachable!() };
    let fn_id = fn_id as usize;
    let element_type = match receiver_type {
        DataType::Array(Some(element_type)) => *element_type,
        _ => state.functions[fn_id].args[0].1.clone().unwrap_or(DataType::Unknown),
    };
    DataType::Array(Some(Box::new(resolve_function_return_type(
        fn_id,
        &[element_type],
        "map",
        ctx,
        state,
    ))))
}

fn builtin_map_filter<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    tgt_id: Option<u16>,
) -> Option<u16> {
    let receiver_type = function_call.args[0].infer_type(ctx, state);
    let src = function_call.args[0].compile(ctx, state, output, None, false, true).unwrap_id();
    let args = &function_call.args[1..];
    let arg_spans = &function_call.get_arg_spans()[1..];
    let name = function_call.qualified_name.get_name();

    let fn_type = args[0].infer_type(ctx, state);
    let DataType::Fn(fn_id) = fn_type else {
        error_expected_function(&fn_type, arg_spans[0], ctx.file_idx, state.sources);
    };
    let fn_id = fn_id as usize;

    let is_filter = name == "filter";
    let is_str = receiver_type == DataType::String;

    let elem_type = match receiver_type {
        DataType::String => DataType::String,
        DataType::Array(Some(t)) => *t,
        _ => state.functions[fn_id].args[0].1.clone().unwrap_or(DataType::Unknown),
    };
    let return_type = if is_filter {
        DataType::Bool
    } else if is_str {
        DataType::String
    } else {
        DataType::Unknown
    };
    let signature = [elem_type.clone(), return_type];
    let matches_signature = if is_filter || is_str {
        fn_matches_signature(fn_id, &signature, ctx, state)
    } else {
        fn_args_match(fn_id, &signature[..1], state)
    };

    if !matches_signature {
        error_function_arg_invalid_type(
            &fn_type,
            &DataType::FnSignature(Box::from(signature)),
            arg_spans[0],
            name,
            None,
            ctx.file_idx,
            state.sources,
        );
    }

    let fn_impl_idx = compile_function_impl(state, fn_id, core::slice::from_ref(&elem_type));
    let unit = state.functions[fn_id].impls[fn_impl_idx].unit;

    let result_id = state.alloc_reg_tgt(tgt_id);
    if is_str {
        let data = Data::comp_str("", &mut state.pools.str_pool);
        let empty_str_id = state.value_reg(data);
        if empty_str_id != result_id {
            output.push(Instr::Mov(empty_str_id, result_id));
        }
    } else {
        output.push(Instr::EmptyArray(result_id));
    }

    let len_id = state.alloc_reg();
    output.push(Instr::Len(src, len_id));

    let index_id = if ctx.single_run {
        state.init_reg(Data::int(0), ctx)
    } else {
        let r = state.alloc_reg();
        output.push(Instr::SetInt(r, 0));
        r
    };

    let elem_id = if is_filter { state.alloc_reg() } else { u16::MAX };
    let false_reg = if is_filter { state.new_const_reg(FALSE) } else { u16::MAX };
    let frame_offset = state.call_frame_offset();
    state.reserve_call_window(frame_offset, 2);
    let arg_reg = frame_offset + 1;

    // Basically the same thing as compile_for_loop
    let jmp_start_idx = output.len();
    output.push(Instr::InfEqIntJmp(len_id, index_id, 0));

    let body_start = output.len();
    let elem_dest = if is_filter { elem_id } else { arg_reg };
    if is_str {
        output.push(Instr::GetIndexString(src, index_id, elem_dest));
    } else {
        output.push(Instr::GetIndexArray(src, index_id, elem_dest));
    }
    if is_filter {
        output.push(Instr::Mov(elem_id, arg_reg));
    }
    output.push(Instr::CallFunc { target: unit, frame_offset });
    let dest_reg_id = if is_filter {
        output.push(Instr::EqJmp(frame_offset, false_reg, 2));
        elem_id
    } else {
        frame_offset
    };
    if is_str {
        output.push(Instr::AddStrInPlace(result_id, dest_reg_id));
    } else {
        output.push(Instr::Push(result_id, dest_reg_id));
    }
    let jump_size = (output.len() - body_start) as u16;

    output.push(Instr::IncIntInfJmp {
        counter_reg: index_id,
        limit_reg: len_id,
        jump_size: jump_size.wrapping_neg(),
    });

    let exit_size = (output.len() - jmp_start_idx) as u16;
    output[jmp_start_idx] = Instr::InfEqIntJmp(len_id, index_id, exit_size);

    if ctx.single_run {
        state.free_reg(len_id);
        state.free_reg(index_id);
        state.free_reg(frame_offset);
        state.free_reg(arg_reg);
        if is_filter {
            state.free_reg(elem_id);
        }
    }
    state.free_reg(src);
    Some(result_id)
}

fn builtin_replace<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    tgt_id: Option<u16>,
) -> Option<u16> {
    let src = function_call.args[0].compile(ctx, state, output, None, false, true).unwrap_id();

    let old_reg = compile_reg_window(function_call.args.iter().skip(1), ctx, state, output);
    let output_id = state.alloc_reg_tgt(tgt_id);
    output.push(Instr::Replace(src, old_reg, output_id));
    state.free_reg_window(old_reg, 2);
    Some(output_id)
}

pub(super) static METHODS: [BuiltIn; 35] = [
    BuiltIn {
        name: "uppercase",
        receivers: &[SignatureType::String],
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::TwoOperands(Instr::Uppercase),
        ..
    },
    BuiltIn {
        name: "lowercase",
        receivers: &[SignatureType::String],
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::TwoOperands(Instr::Lowercase),
        ..
    },
    BuiltIn {
        name: "trim",
        receivers: &[SignatureType::String],
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::TwoOperands(Instr::Trim),
        ..
    },
    BuiltIn {
        name: "trim_left",
        receivers: &[SignatureType::String],
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::TwoOperands(Instr::TrimLeft),
        ..
    },
    BuiltIn {
        name: "trim_right",
        receivers: &[SignatureType::String],
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::TwoOperands(Instr::TrimRight),
        ..
    },
    BuiltIn {
        name: "is_float",
        receivers: &[SignatureType::String],
        return_type: ReturnType::Type(SignatureType::Bool),
        lowering: Lowering::TwoOperands(Instr::IsFloat),
        ..
    },
    BuiltIn {
        name: "is_int",
        receivers: &[SignatureType::String],
        return_type: ReturnType::Type(SignatureType::Bool),
        lowering: Lowering::TwoOperands(Instr::IsInt),
        ..
    },
    BuiltIn {
        name: "len",
        receivers: &[SignatureType::String, SignatureType::Array(None), SignatureType::AnyMap],
        return_type: ReturnType::Type(SignatureType::Int),
        lowering: Lowering::TwoOperands(Instr::Len),
        ..
    },
    BuiltIn {
        name: "sqrt",
        receivers: &[SignatureType::Float],
        return_type: ReturnType::Type(SignatureType::Float),
        lowering: Lowering::TwoOperands(Instr::SqrtFloat),
        ..
    },
    BuiltIn {
        name: "round",
        receivers: &[SignatureType::Float],
        return_type: ReturnType::Type(SignatureType::Float),
        lowering: Lowering::TwoOperands(Instr::Round),
        ..
    },
    BuiltIn {
        name: "floor",
        receivers: &[SignatureType::Float],
        return_type: ReturnType::Type(SignatureType::Float),
        lowering: Lowering::TwoOperands(Instr::Floor),
        ..
    },
    BuiltIn {
        name: "abs",
        receivers: &[SignatureType::Int, SignatureType::Float],
        return_type: ReturnType::Type(SignatureType::Receiver),
        lowering: Lowering::TwoOperands(Instr::Abs),
        ..
    },
    BuiltIn {
        name: "reverse",
        receivers: &[SignatureType::String],
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::TwoOperands(Instr::ReverseString),
        ..
    },
    BuiltIn {
        name: "reverse",
        receivers: &[SignatureType::Array(None)],
        lowering: Lowering::OneOperand(Instr::ReverseArray),
        ..
    },
    BuiltIn {
        name: "sort",
        receivers: &[SignatureType::Array(None)],
        lowering: Lowering::OneOperand(Instr::Sort),
        ..
    },
    BuiltIn {
        name: "starts_with",
        receivers: &[SignatureType::String],
        positional: &[&[SignatureType::String]],
        return_type: ReturnType::Type(SignatureType::Bool),
        lowering: Lowering::ThreeOperands(|src, prefix, dst| Instr::StartsWith {
            src,
            prefix,
            dst,
        }),
        ..
    },
    BuiltIn {
        name: "ends_with",
        receivers: &[SignatureType::String],
        positional: &[&[SignatureType::String]],
        return_type: ReturnType::Type(SignatureType::Bool),
        lowering: Lowering::ThreeOperands(|src, suffix, dst| Instr::EndsWith { src, suffix, dst }),
        ..
    },
    BuiltIn {
        name: "contains",
        receivers: &[SignatureType::String],
        positional: &[&[SignatureType::String]],
        return_type: ReturnType::Type(SignatureType::Bool),
        lowering: Lowering::ThreeOperands(|src, needle, dst| Instr::Contains { src, needle, dst }),
        ..
    },
    BuiltIn {
        name: "contains",
        receivers: &[SignatureType::Array(None)],
        positional: &[&[SignatureType::Any]],
        return_type: ReturnType::Type(SignatureType::Bool),
        lowering: Lowering::ThreeOperands(|src, needle, dst| Instr::Contains { src, needle, dst }),
        ..
    },
    BuiltIn {
        name: "trim_sequence",
        receivers: &[SignatureType::String],
        positional: &[&[SignatureType::String]],
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::ThreeOperands(|src: u16, seq, dst| Instr::TrimSequence {
            src,
            seq,
            dst,
        }),
        ..
    },
    BuiltIn {
        name: "trim_sequence_left",
        receivers: &[SignatureType::String],
        positional: &[&[SignatureType::String]],
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::ThreeOperands(|src: u16, seq, dst| Instr::TrimSequenceLeft {
            src,
            seq,
            dst,
        }),
        ..
    },
    BuiltIn {
        name: "trim_sequence_right",
        receivers: &[SignatureType::String],
        positional: &[&[SignatureType::String]],
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::ThreeOperands(|src, seq, dst| Instr::TrimSequenceRight {
            src,
            seq,
            dst,
        }),
        ..
    },
    BuiltIn {
        name: "find",
        receivers: &[SignatureType::String, SignatureType::Array(None)],
        positional: &[&[SignatureType::ReceiverElement]],
        return_type: ReturnType::Type(SignatureType::Int),
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::ThreeOperands(|src, needle, dst| Instr::Find { src, needle, dst }),
        ..
    },
    BuiltIn {
        name: "repeat",
        receivers: &[SignatureType::String, SignatureType::Array(None)],
        positional: &[&[SignatureType::Int]],
        return_type: ReturnType::Type(SignatureType::Receiver),
        lowering: Lowering::ThreeOperands(|src, count, dst| Instr::Repeat { src, count, dst }),
        ..
    },
    BuiltIn {
        name: "replace",
        receivers: &[SignatureType::String],
        positional: &[&[SignatureType::String], &[SignatureType::String]],
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::Custom(builtin_replace),
        ..
    },
    BuiltIn {
        name: "split",
        receivers: &[SignatureType::String],
        positional: &[&[SignatureType::String]],
        return_type: ReturnType::Type(SignatureType::Array(Some(&SignatureType::String))),
        lowering: Lowering::ThreeOperands(|src, sep, dst| Instr::Split { src, sep, dst }),
        ..
    },
    BuiltIn {
        name: "partition",
        receivers: &[SignatureType::Array(None)],
        positional: &[&[SignatureType::ReceiverElement]],
        return_type: ReturnType::Type(SignatureType::Array(Some(&SignatureType::Receiver))),
        lowering: Lowering::ThreeOperands(|src, sep, dst| Instr::Split { src, sep, dst }),
        ..
    },
    BuiltIn {
        name: "join",
        receivers: &[SignatureType::Array(Some(&SignatureType::String))],
        positional: &[&[SignatureType::String]],
        default: Some(DefaultArg::EmptyStr),
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::ThreeOperands(Instr::JoinStringArray),
        ..
    },
    BuiltIn {
        name: "remove",
        receivers: &[SignatureType::Array(None)],
        positional: &[&[SignatureType::Int]],
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::TwoOperands(Instr::Remove),
        ..
    },
    BuiltIn {
        name: "remove",
        receivers: &[SignatureType::AnyMap],
        positional: &[&[SignatureType::MapKey]],
        return_type: ReturnType::Type(SignatureType::Bool),
        lowering: Lowering::ThreeOperands(|map_reg_id, key_reg_id, dest_reg_id| Instr::MapRemove {
            map_reg_id,
            key_reg_id,
            dest_reg_id,
        }),
        ..
    },
    BuiltIn {
        name: "get",
        receivers: &[SignatureType::AnyMap],
        positional: &[&[SignatureType::MapKey]],
        return_type: ReturnType::Type(SignatureType::MapValue),
        error_span: ErrorSpan::Argument(0),
        lowering: Lowering::ThreeOperands(Instr::MapGet),
        ..
    },
    BuiltIn {
        name: "insert",
        receivers: &[SignatureType::AnyMap],
        positional: &[&[SignatureType::MapKey], &[SignatureType::MapValue]],
        lowering: Lowering::ThreeOperands(Instr::MapInsertReg),
        ..
    },
    BuiltIn {
        name: "push",
        receivers: &[SignatureType::Array(None)],
        positional: &[&[SignatureType::ReceiverElement]],
        lowering: Lowering::Custom(builtin_push),
        ..
    },
    BuiltIn {
        name: "map",
        receivers: &[SignatureType::String, SignatureType::Array(None)],
        positional: &[&[SignatureType::Function]],
        return_type: ReturnType::Computed(builtin_map_return_type),
        lowering: Lowering::Custom(builtin_map_filter),
        ..
    },
    BuiltIn {
        name: "filter",
        receivers: &[SignatureType::String, SignatureType::Array(None)],
        positional: &[&[SignatureType::Function]],
        return_type: ReturnType::Type(SignatureType::Receiver),
        lowering: Lowering::Custom(builtin_map_filter),
        ..
    },
];
