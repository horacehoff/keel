use crate::{
    ast::FunctionCallExpr,
    compiler::{
        DataType,
        functions::builtin::{BuiltIn, DefaultArg, ErrorSpan, Lowering, ReturnType, SignatureType},
        registers::UnwrapId,
        state::{Ctx, State},
    },
    data::Data,
    instr::Instr,
};

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

pub(super) static GLOBAL_FUNCTIONS: [BuiltIn; 12] = [
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
