use crate::ast::FunctionCallExpr;
use crate::compiler::DataType;
use crate::compiler::Scope;
use crate::compiler::compiler_errors::check_args_length;
use crate::compiler::compiler_errors::check_args_range;
use crate::compiler::compiler_errors::error_function_arg_invalid_type_multiple;
use crate::compiler::compiler_errors::error_invalid_obj_type;
use crate::compiler::compiler_errors::error_unknown_function;
use crate::compiler::registers::UnwrapId;
use crate::compiler::state::Ctx;
use crate::compiler::state::State;
use crate::data::Data;
use crate::instr::Instr;
use core::hint::cold_path;
use core::hint::unreachable_unchecked;
use global::GLOBAL_FUNCTIONS;
use library::LIBRARIES;
use library::Library;
use method::METHODS;

mod global;
mod library;
mod method;

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

pub(super) struct BuiltIn {
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

#[inline(always)]
pub(super) fn find_library(namespace: &[&str]) -> Option<&'static Library> {
    let [name] = namespace else { return None };
    LIBRARIES.iter().find(|library| library.name == *name)
}

#[inline(always)]
pub(super) fn find_builtin(builtins: &'static [BuiltIn], name: &str) -> Option<&'static BuiltIn> {
    builtins.iter().find(|function| function.name == name)
}

#[inline(always)]
pub(super) fn find_builtin_global(name: &str) -> Option<&'static BuiltIn> {
    GLOBAL_FUNCTIONS.iter().find(|function| function.name == name)
}

pub(super) fn find_method<'arena>(
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

pub(super) fn check_builtin_call<'arena>(
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

pub(super) fn compile_builtin_call<'arena>(
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
