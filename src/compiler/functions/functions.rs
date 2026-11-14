#![allow(clippy::unnecessary_wraps)]
use crate::ast::Expr;
use crate::ast::FunctionCallExpr;
use crate::ast::Span;
use crate::compiler::Ctx;
use crate::compiler::DataType;
use crate::compiler::codegen::compile_reg_window;
use crate::compiler::compiler_errors::check_args_length;
use crate::compiler::compiler_errors::check_args_range;
use crate::compiler::compiler_errors::error_expected_function;
use crate::compiler::compiler_errors::error_function_arg_invalid_type;
use crate::compiler::compiler_errors::error_function_arg_invalid_type_multiple;
use crate::compiler::compiler_errors::error_invalid_obj_type;
use crate::compiler::compiler_errors::error_unknown_function;
use crate::compiler::compiler_errors::error_unknown_function_in_namespace;
use crate::compiler::registers::UnwrapId;
use crate::compiler::scope::Scope;
use crate::compiler::state::State;
use crate::compiler::types::c_arg_matches;
use crate::compiler::types::fn_args_match;
use crate::compiler::types::fn_matches_signature;
use crate::compiler::types::resolve_function_return_type;
use crate::data::Data;
use crate::data::FALSE;
use crate::instr::Instr;
use core::hint::cold_path;
use std::hint::unreachable_unchecked;
pub(super) use user_functions::compile_function_impl;
use user_functions::handle_user_function;

mod user_functions;

#[cfg(target_arch = "wasm32")]
use crate::errors::wasm_error;

#[derive(Clone, Copy)]
enum SignatureType {
    Int,
    Float,
    Bool,
    String,
    Array(Option<&'static Self>),
    Function,
    MapKey,
    MapValue,
    AnyMap,
    Receiver,
    ReceiverElement,
    Any,
}

impl SignatureType {
    fn accepts(self, actual: &DataType, receiver: Option<&DataType>) -> bool {
        match self {
            Self::Any => true,
            Self::Int => matches!(actual, DataType::Int),
            Self::Float => matches!(actual, DataType::Float),
            Self::Bool => matches!(actual, DataType::Bool),
            Self::String => matches!(actual, DataType::String),
            Self::Array(elem_type) => match actual {
                DataType::Array(Some(actual_elem_type)) => {
                    elem_type.is_none_or(|elem_type| elem_type.accepts(actual_elem_type, receiver))
                }
                DataType::Array(None) => true,
                _ => false,
            },
            Self::AnyMap => matches!(actual, DataType::Map(_)),
            Self::Function => matches!(actual, DataType::Fn(_)),
            Self::Receiver | Self::ReceiverElement | Self::MapKey | Self::MapValue => {
                let expected = self.to_datatype(receiver);
                matches!(expected, DataType::Unknown) || expected == *actual
            }
        }
    }
    fn to_datatype(self, receiver: Option<&DataType>) -> DataType {
        match self {
            Self::Int => DataType::Int,
            Self::Float => DataType::Float,
            Self::Bool => DataType::Bool,
            Self::String => DataType::String,
            Self::Any | Self::Function => DataType::Unknown,
            Self::Array(elem_type) => DataType::Array(
                elem_type.map(|elem_type| Box::new(elem_type.to_datatype(receiver))),
            ),
            Self::AnyMap => DataType::Map(Box::new((None, None))),
            Self::Receiver => receiver.cloned().unwrap_or(DataType::Unknown),
            Self::ReceiverElement => match receiver {
                Some(DataType::Array(Some(element))) => (**element).clone(),
                Some(DataType::String) => DataType::String,
                _ => DataType::Unknown,
            },
            Self::MapKey => match receiver {
                Some(DataType::Map(types)) => types.0.clone().unwrap_or(DataType::Unknown),
                _ => DataType::Unknown,
            },
            Self::MapValue => match receiver {
                Some(DataType::Map(types)) => types.1.clone().unwrap_or(DataType::Unknown),
                _ => DataType::Unknown,
            },
        }
    }
}

fn is_accepted(accepted: &[SignatureType], actual: &DataType, receiver: Option<&DataType>) -> bool {
    match actual {
        DataType::Union(members) => {
            members.iter().all(|member| is_accepted(accepted, member, receiver))
        }
        _ => accepted.iter().any(|formal| formal.accepts(actual, receiver)),
    }
}

#[derive(Clone, Copy)]
enum DefaultArg {
    EmptyStr,
    Int(i32),
}

enum ReturnType {
    Null,
    Type(SignatureType),
    Computed(
        for<'arena> fn(&'arena FunctionCallExpr<'arena>, Ctx, &mut State<'arena, '_>) -> DataType,
    ),
}

enum Lowering {
    OneOperand(fn(u16) -> Instr),
    TwoOperands(fn(u16, u16) -> Instr),
    ThreeOperands(fn(u16, u16, u16) -> Instr),
    Custom(
        for<'arena> fn(
            &'arena FunctionCallExpr<'arena>,
            &mut Vec<Instr>,
            Ctx,
            &mut State<'arena, '_>,
            Option<u16>,
        ) -> Option<u16>,
    ),
}

enum ErrorSpan {
    None,
    Callsite,
    Argument(u8),
}

struct BuiltIn {
    name: &'static str,
    receivers: &'static [SignatureType] = &[] as &[SignatureType], // fixes a rust-analyzer bug
    positional: &'static [&'static [SignatureType]] = &[] as &[&[SignatureType]],
    /// Applies to the last positional parameter
    default: Option<DefaultArg> = None,
    return_type: ReturnType = ReturnType::Null,
    error_span: ErrorSpan = ErrorSpan::None,
    lowering: Lowering,
}

impl BuiltIn {
    fn get_return_type<'arena>(
        &'static self,
        function_call: &'arena FunctionCallExpr<'arena>,
        receiver_type: Option<&DataType>,
        ctx: Ctx,
        state: &mut State<'arena, '_>,
    ) -> DataType {
        match self.return_type {
            ReturnType::Null => DataType::Null,
            ReturnType::Type(t) => t.to_datatype(receiver_type),
            ReturnType::Computed(f) => {
                check_builtin_call(self, function_call, receiver_type, ctx, state);
                f(function_call, ctx, state)
            }
        }
    }
}

struct Library {
    name: &'static str,
    functions: &'static [BuiltIn],
    #[allow(unused)]
    supported_on_wasm: bool,
}

const STRING: &[SignatureType] = &[SignatureType::String];

static FS_FUNCTIONS: &[BuiltIn] = &[
    BuiltIn {
        name: "read",
        positional: &[STRING],
        return_type: ReturnType::Type(SignatureType::String),
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::TwoOperands(Instr::FsRead),
        ..
    },
    BuiltIn {
        name: "exists",
        positional: &[STRING],
        return_type: ReturnType::Type(SignatureType::Bool),
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::TwoOperands(Instr::FsExists),
        ..
    },
    BuiltIn {
        name: "write",
        positional: &[STRING, STRING],
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::TwoOperands(Instr::FsWrite),
        ..
    },
    BuiltIn {
        name: "append",
        positional: &[STRING, STRING],
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::TwoOperands(Instr::FsAppend),
        ..
    },
    BuiltIn {
        name: "delete",
        positional: &[STRING],
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::OneOperand(Instr::FsDelete),
        ..
    },
    BuiltIn {
        name: "delete_dir",
        positional: &[STRING],
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::OneOperand(Instr::FsDeleteDir),
        ..
    },
];

static LIBRARIES: [Library; 1] =
    [Library { name: "fs", functions: FS_FUNCTIONS, supported_on_wasm: false }];

fn builtin_print<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    _tgt_id: Option<u16>,
) -> Option<u16> {
    for arg in function_call.args {
        let id = arg.compile(ctx, state, output, None, false, true).unwrap_id();
        output.push(Instr::Print(id));
        state.free_reg(id);
    }
    None
}

fn builtin_type<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    _output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    _tgt_id: Option<u16>,
) -> Option<u16> {
    let inferred = function_call.args[0].infer_type(ctx, state);
    let arg_type =
        Data::comp_str(inferred.format_detailed(state).as_str(), &mut state.pools.str_pool);
    Some(state.value_reg(arg_type))
}

fn builtin_string<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    tgt_id: Option<u16>,
) -> Option<u16> {
    let arg = &function_call.args[0];
    let id = arg.compile(ctx, state, output, None, false, true).unwrap_id();
    if arg.infer_type(ctx, state) == DataType::String {
        return Some(id);
    }
    state.free_reg(id);
    let output_id = state.alloc_reg_tgt(tgt_id);
    output.push(Instr::Str(id, output_id));
    Some(output_id)
}

fn builtin_range<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    tgt_id: Option<u16>,
) -> Option<u16> {
    let args = function_call.args;
    let (min_reg, max_expr) = if args.len() == 1 {
        (state.new_const_reg(Data::int(0)), &args[0])
    } else {
        (args[0].compile(ctx, state, output, None, false, true).unwrap_id(), &args[1])
    };
    let max_reg = max_expr.compile(ctx, state, output, None, false, true).unwrap_id();
    state.free_reg(min_reg);
    state.free_reg(max_reg);
    let output_id = state.alloc_reg_tgt(tgt_id);
    output.push(Instr::Range(min_reg, max_reg, output_id));
    Some(output_id)
}

fn builtin_exit<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    _tgt_id: Option<u16>,
) -> Option<u16> {
    let halt_code = if function_call.args.is_empty() {
        u16::MAX
    } else {
        function_call.args[0].compile(ctx, state, output, None, false, true).unwrap_id()
    };
    output.push(Instr::Halt(halt_code));
    None
}

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

static GLOBAL_FUNCTIONS: [BuiltIn; 12] = [
    BuiltIn { name: "print", lowering: Lowering::Custom(builtin_print), .. },
    BuiltIn {
        name: "type",
        positional: &[&[SignatureType::Any]],
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::Custom(builtin_type),
        ..
    },
    BuiltIn {
        name: "string",
        positional: &[&[SignatureType::Any]],
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::Custom(builtin_string),
        ..
    },
    BuiltIn {
        name: "float",
        positional: &[&[SignatureType::String, SignatureType::Int]],
        return_type: ReturnType::Type(SignatureType::Float),
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::TwoOperands(Instr::Float),
        ..
    },
    BuiltIn {
        name: "int",
        positional: &[&[SignatureType::String, SignatureType::Float]],
        return_type: ReturnType::Type(SignatureType::Int),
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::TwoOperands(Instr::Int),
        ..
    },
    BuiltIn {
        name: "bool",
        positional: &[&[SignatureType::String]],
        return_type: ReturnType::Type(SignatureType::Bool),
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::TwoOperands(Instr::Bool),
        ..
    },
    BuiltIn {
        name: "input",
        positional: &[&[SignatureType::String]],
        default: Some(DefaultArg::EmptyStr),
        return_type: ReturnType::Type(SignatureType::String),
        lowering: Lowering::TwoOperands(Instr::Input),
        ..
    },
    BuiltIn {
        name: "range",
        positional: &[&[SignatureType::Int], &[SignatureType::Int]],
        default: Some(DefaultArg::Int(0)),
        return_type: ReturnType::Type(SignatureType::Array(Some(&SignatureType::Int))),
        lowering: Lowering::Custom(builtin_range),
        ..
    },
    BuiltIn {
        name: "the_answer",
        return_type: ReturnType::Type(SignatureType::Int),
        lowering: Lowering::OneOperand(Instr::TheAnswer),
        ..
    },
    BuiltIn {
        name: "argv",
        return_type: ReturnType::Type(SignatureType::Array(Some(&SignatureType::String))),
        lowering: Lowering::OneOperand(Instr::Argv),
        ..
    },
    BuiltIn {
        name: "exit",
        positional: &[&[SignatureType::Int]],
        default: Some(DefaultArg::Int(0)),
        lowering: Lowering::Custom(builtin_exit),
        ..
    },
    BuiltIn {
        name: "throw",
        positional: &[&[SignatureType::String]],
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::OneOperand(Instr::ThrowError),
        ..
    },
];

static METHODS: [BuiltIn; 35] = [
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

#[inline(always)]
fn find_library(namespace: &[&str]) -> Option<&'static Library> {
    let [name] = namespace else { return None };
    LIBRARIES.iter().find(|library| library.name == *name)
}

#[inline(always)]
fn find_builtin(builtins: &'static [BuiltIn], name: &str) -> Option<&'static BuiltIn> {
    builtins.iter().find(|function| function.name == name)
}

fn find_method<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
) -> (&'static BuiltIn, DataType) {
    let name = function_call.qualified_name.get_name();
    let receiver_type = function_call.args[0].infer_type(ctx, state);
    let candidates = METHODS.iter().filter(|method| method.name == name); // Future-proof
    if let Some(method) =
        candidates.clone().find(|method| is_accepted(method.receivers, &receiver_type, None))
    {
        return (method, receiver_type);
    }
    cold_path();
    let expected: Vec<DataType> =
        candidates.flat_map(|method| method.receivers).map(|t| t.to_datatype(None)).collect();
    if expected.is_empty() {
        error_unknown_function(
            name,
            function_call.get_call_span(),
            &Scope::default(),
            ctx.file_idx,
            state.sources,
        );
    }
    error_invalid_obj_type(
        &expected,
        &receiver_type,
        name,
        function_call.get_nth_arg_span(0),
        state.sources,
        ctx.file_idx,
    );
}

pub fn compile_method_call<'arena>(
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    tgt_id: Option<u16>,
    function_call: &'arena FunctionCallExpr<'arena>,
) -> Option<u16> {
    let (method, receiver_type) = find_method(function_call, ctx, state);
    check_builtin_call(method, function_call, Some(&receiver_type), ctx, state);
    compile_builtin_call(method, function_call, output, ctx, state, tgt_id)
}

pub fn get_method_return_type<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
) -> DataType {
    let (method, receiver_type) = find_method(function_call, ctx, state);
    method.get_return_type(function_call, Some(&receiver_type), ctx, state)
}

pub fn get_builtin_function_return_type<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
) -> Option<DataType> {
    let name = function_call.qualified_name.get_name();
    let namespace = function_call.qualified_name.get_namespace();
    let function = if namespace.is_empty() {
        find_builtin(&GLOBAL_FUNCTIONS, name)?
    } else {
        find_builtin(find_library(namespace)?.functions, name)?
    };
    Some(function.get_return_type(function_call, None, ctx, state))
}

fn check_builtin_call<'arena>(
    builtin: &'static BuiltIn,
    function_call: &'arena FunctionCallExpr<'arena>,
    receiver_type: Option<&DataType>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
) {
    let receiver_count = usize::from(!builtin.receivers.is_empty());
    let args = &function_call.args[receiver_count..];
    let arg_spans = &function_call.get_arg_spans()[receiver_count..];
    let name = function_call.qualified_name.get_name();
    let span = function_call.get_call_span();

    let minimum_arg_count = builtin.positional.len() - usize::from(builtin.default.is_some());
    let maximum_arg_count =
        if builtin.positional.is_empty() && matches!(builtin.lowering, Lowering::Custom(_)) {
            debug_assert_eq!(builtin.name, "print");
            usize::MAX
        } else {
            builtin.positional.len()
        };
    if minimum_arg_count == maximum_arg_count {
        check_args_length(args, minimum_arg_count, name, span, state.sources, ctx.file_idx);
    } else {
        check_args_range(
            args,
            minimum_arg_count,
            maximum_arg_count,
            name,
            arg_spans,
            ctx.file_idx,
            state.sources,
            span,
        );
    }
    for ((arg, &arg_span), accepted) in args.iter().zip(arg_spans).zip(builtin.positional) {
        let actual = arg.infer_type(ctx, state);
        if !is_accepted(accepted, &actual, receiver_type) {
            let expected: Vec<DataType> =
                accepted.iter().map(|f| f.to_datatype(receiver_type)).collect();
            error_function_arg_invalid_type_multiple(
                &actual,
                &expected,
                arg_span,
                name,
                None,
                ctx.file_idx,
                state.sources,
            );
        }
    }
}

fn compile_builtin_call<'arena>(
    builtin: &'static BuiltIn,
    function_call: &'arena FunctionCallExpr<'arena>,
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    tgt_id: Option<u16>,
) -> Option<u16> {
    if let Lowering::Custom(f) = builtin.lowering {
        return f(function_call, output, ctx, state, tgt_id);
    }
    let has_receiver = usize::from(!builtin.receivers.is_empty());
    let arg_count = has_receiver + builtin.positional.len();
    // cheaper and simpler than a Vec
    let mut operands = [0u16; 3];
    for (idx, operand) in operands[..arg_count].iter_mut().enumerate() {
        *operand = match function_call.args.get(idx) {
            Some(arg) => arg.compile(ctx, state, output, None, false, true).unwrap_id(),
            None => match unsafe { builtin.default.unwrap_unchecked() } {
                DefaultArg::EmptyStr => {
                    let empty_str = Data::comp_str("", &mut state.pools.str_pool);
                    state.value_reg(empty_str)
                }
                DefaultArg::Int(i) => state.new_const_reg(Data::int(i)),
            },
        };
    }
    for &operand in &operands[..arg_count] {
        state.free_reg(operand);
    }
    let dst = (!matches!(builtin.return_type, ReturnType::Null)).then(|| {
        operands[arg_count] = state.alloc_reg_tgt(tgt_id);
        operands[arg_count]
    });

    let [o1, o2, o3] = operands;
    output.push(match builtin.lowering {
        Lowering::OneOperand(f) => f(o1),
        Lowering::TwoOperands(f) => f(o1, o2),
        Lowering::ThreeOperands(f) => f(o1, o2, o3),
        Lowering::Custom(_) => unsafe { unreachable_unchecked() },
    });

    match builtin.error_span {
        ErrorSpan::None => {}
        ErrorSpan::Callsite => state.add_to_src(ctx, output, function_call.get_call_span()),
        ErrorSpan::Argument(index) => {
            let span = function_call.get_nth_arg_span(has_receiver + index as usize);
            state.add_to_src(ctx, output, span);
        }
    }
    dst
}

pub fn check_user_fn_arg_types(
    fn_id: usize,
    fn_name: &str,
    inferred_arg_types: &[DataType],
    args_indexes: &[Span],
    ctx: Ctx,
    state: &mut State<'_, '_>,
) {
    let args_len = state.functions[fn_id].args.len();
    for i in 0..args_len {
        let t = state.functions[fn_id].args[i].1.clone();
        if let Some(t) = &t
            && if let (DataType::FnSignature(expected_sig), DataType::Fn(concrete_id)) =
                (t, &inferred_arg_types[i])
            {
                !fn_matches_signature(*concrete_id as usize, expected_sig, ctx, state)
            } else {
                inferred_arg_types[i] != *t
            }
        {
            error_function_arg_invalid_type(
                &inferred_arg_types[i],
                t,
                args_indexes[i],
                fn_name,
                Some((state.functions[fn_id].name_span, state.functions[fn_id].src_file_idx)),
                ctx.file_idx,
                state.sources,
            );
        }
    }
}

pub fn compile_function_call<'arena>(
    function_call: &'arena FunctionCallExpr<'arena>,
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    tgt_id: Option<u16>,
) -> Option<u16> {
    let namespace = function_call.qualified_name.get_namespace();
    let name = function_call.qualified_name.get_name();
    let span = function_call.get_call_span();
    if namespace.is_empty() {
        if let Some(function) = GLOBAL_FUNCTIONS.iter().find(|function| function.name == name) {
            check_builtin_call(function, function_call, None, ctx, state);
            compile_builtin_call(function, function_call, output, ctx, state, tgt_id)
        } else if let Some(var) = state.find_var(name) {
            let DataType::Fn(fn_id) = var.var_type else {
                error_expected_function(&var.var_type, span, ctx.file_idx, state.sources);
            };
            handle_user_function(function_call, fn_id as usize, output, ctx, state, tgt_id)
        } else if let Some(fn_id) =
            state.scope(ctx.file_idx).find_function(&[], name, span, ctx.file_idx, state.sources)
        {
            handle_user_function(function_call, fn_id, output, ctx, state, tgt_id)
        } else {
            error_unknown_function(
                name,
                span,
                state.scope(ctx.file_idx),
                ctx.file_idx,
                state.sources,
            )
        }
    } else if let Some(library) = find_library(namespace) {
        #[cfg(target_arch = "wasm32")]
        if !library.supported_on_wasm {
            wasm_error(hformat!("WASM does not support the {} library", library.name));
        }

        let Some(function) = find_builtin(library.functions, name) else {
            error_unknown_function(name, span, &Scope::default(), ctx.file_idx, state.sources)
        };

        check_builtin_call(function, function_call, None, ctx, state);
        compile_builtin_call(function, function_call, output, ctx, state, tgt_id)
    } else if let Some((fn_args, returns_null, dyn_fn_id)) = state
        .dylibs
        .iter()
        .find(|l| l.name == namespace[0])
        .and_then(|lib| lib.fns.iter().find(|x| x.name == name))
        .map(|sig| (sig.args.clone(), sig.return_type == DataType::Null, sig.id))
    {
        check_args_length(
            function_call.args,
            fn_args.len(),
            name,
            span,
            state.sources,
            ctx.file_idx,
        );
        for (i, a) in fn_args.iter().enumerate() {
            let inferred_arg_type = function_call.args[i].infer_type(ctx, state);
            if !c_arg_matches(&inferred_arg_type, a) {
                error_function_arg_invalid_type(
                    &inferred_arg_type,
                    a,
                    function_call.get_nth_arg_span(i),
                    name,
                    None,
                    ctx.file_idx,
                    state.sources,
                );
            }
        }

        let first_arg = compile_reg_window(function_call.args.iter(), ctx, state, output);
        let dest_reg_id =
            if returns_null { state.alloc_reg() } else { state.alloc_reg_tgt(tgt_id) };
        output.push(Instr::CallDynamicLibFunc { fn_id: dyn_fn_id, dst: dest_reg_id, first_arg });
        state.add_to_src(ctx, output, span);
        state.free_reg_window(first_arg, function_call.args.len() as u16);
        if returns_null {
            state.free_reg(dest_reg_id);
            None
        } else {
            Some(dest_reg_id)
        }
    } else if let Some(fn_id) =
        state.scope(ctx.file_idx).find_function(namespace, name, span, ctx.file_idx, state.sources)
    {
        handle_user_function(function_call, fn_id, output, ctx, state, tgt_id)
    } else {
        error_unknown_function_in_namespace(
            name,
            state.scope(ctx.file_idx),
            namespace,
            span,
            ctx.file_idx,
            state.sources,
        );
    }
}
