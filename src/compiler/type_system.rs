use super::expr::Expr;
use super::expr::Span;
use super::expr::symbol_of_expr;
use crate::compiler::Scope;
use crate::compiler::compiler_data::Ctx;
use crate::compiler::compiler_data::FnSignature;
use crate::compiler::compiler_data::Function;
use crate::compiler::compiler_data::Source;
use crate::compiler::compiler_data::State;
use crate::compiler::compiler_errors::error_expected_function;
use crate::compiler::compiler_errors::error_function_needs_args_typed;
use crate::compiler::compiler_errors::error_invalid_c_type;
use crate::compiler::compiler_errors::error_invalid_type;
use crate::compiler::compiler_errors::error_op;
use crate::compiler::compiler_errors::error_struct_unknown_field;
use crate::compiler::compiler_errors::error_type_not_indexable;
use crate::compiler::compiler_errors::error_unknown_function;
use crate::compiler::compiler_errors::error_unknown_function_in_namespace;
use crate::compiler::compiler_errors::error_unknown_struct;
use crate::compiler::compiler_errors::error_unknown_type;
use crate::compiler::compiler_errors::error_unknown_type_with_namespace;
use crate::compiler::compiler_errors::error_unknown_variable;
use crate::compiler::expr::FunctionCallExpr;
use crate::compiler::expr::IfBlockExpr;
use crate::compiler::expr::QualifiedName;
use crate::compiler::expr::VariableDeclarationExpr;
use rustc_hash::FxHashSet;
use std::cell::RefCell;
use std::hint::cold_path;
use std::hint::unreachable_unchecked;

#[cfg(not(target_arch = "wasm32"))]
use crate::compiler::compiler_data::Struct;

#[cfg(not(target_arch = "wasm32"))]
use libffi::middle::Type;

// Tracks which user-defined functions are currently being analysed for their
// return type. Used to break mutual-recursion cycles in type inference
thread_local! {
    static RETURN_TYPE_INFERRING: RefCell<FxHashSet<usize>> =
        RefCell::new(FxHashSet::default());
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TypeExpr<'arena> {
    Identifier(&'arena str, Span),
    NamespacedIdentifier(QualifiedName<'arena>, Span),
    Array(&'arena Self),
    Map(&'arena Self, &'arena Self),
    Union(&'arena [Self]),
    Function(&'arena [Self]),
    Null,
}

impl TypeExpr<'_> {
    pub fn to_datatype(self, file_idx: u16, scope: &Scope, sources: &[Source]) -> DataType {
        match self {
            Self::Null => DataType::Null,
            Self::Identifier(s, span) => match s {
                "int" => DataType::Int,
                "float" => DataType::Float,
                "bool" => DataType::Bool,
                "string" => DataType::String,
                "null" => DataType::Null,
                struct_name => {
                    if let Some(struct_id) =
                        scope.find_struct(&[], struct_name, span, file_idx, sources)
                    {
                        DataType::Struct(struct_id as u16)
                    } else {
                        error_unknown_type(span, file_idx, struct_name, sources, scope);
                    }
                }
            },
            Self::NamespacedIdentifier(s, span) => {
                if let Some(struct_id) =
                    scope.find_struct(s.get_namespace(), s.get_name(), span, file_idx, sources)
                {
                    DataType::Struct(struct_id as u16)
                } else {
                    cold_path();
                    error_unknown_type_with_namespace(
                        span,
                        file_idx,
                        s.get_name(),
                        sources,
                        scope,
                        s.get_namespace(),
                    )
                }
            }
            Self::Array(inner_t) => {
                DataType::Array(Some(Box::new(inner_t.to_datatype(file_idx, scope, sources))))
            }
            Self::Map(k_t, v_t) => DataType::Map(Box::from((
                Some(k_t.to_datatype(file_idx, scope, sources)),
                Some(v_t.to_datatype(file_idx, scope, sources)),
            ))),
            Self::Union(poly) => DataType::Union(
                poly.iter().map(|t| t.to_datatype(file_idx, scope, sources)).collect(),
            )
            .check_poly(),
            Self::Function(parts) => DataType::FnSignature(
                parts.iter().map(|t| t.to_datatype(file_idx, scope, sources)).collect(),
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DataType {
    /// Array(None) = Unknown[]
    Array(Option<Box<Self>>),
    Float,
    Int,
    Bool,
    String,
    Null,
    Unknown,
    Union(Box<[Self]>),
    Fn(u16),
    /// Arg types followed by the return type as the last element
    FnSignature(Box<[Self]>),
    Struct(u16),
    Map(Box<(Option<Self>, Option<Self>)>),
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Float => write!(f, "float"),
            Self::Int => write!(f, "int"),
            Self::Bool => write!(f, "bool"),
            Self::String => write!(f, "string"),
            Self::Array(array_type) => match array_type {
                Some(array_type) => write!(f, "{array_type}[]"),
                None => write!(f, "T[]"),
            },
            Self::Null => write!(f, "null"),
            Self::Unknown => write!(f, "Unknown"),
            Self::Union(types) => write!(
                f,
                "{}",
                types.into_iter().map(|x| format!("{x}")).collect::<Vec<_>>().join("|")
            ),
            Self::Struct(_) => write!(f, "struct"),
            Self::Map(m) => write!(
                f,
                "{{{}: {}}}",
                m.0.as_ref().unwrap_or(&Self::Unknown),
                m.1.as_ref().unwrap_or(&Self::Unknown)
            ),
            Self::Fn(_) => write!(f, "function"),
            Self::FnSignature(sig) => {
                let (args, ret) = sig.split_at(sig.len() - 1);
                write!(
                    f,
                    "fn({}) -> {}",
                    args.iter().map(|t| format!("{t}")).collect::<Vec<_>>().join(", "),
                    ret[0]
                )
            }
        }
    }
}

impl DataType {
    pub fn format_detailed(&self, state: &State<'_, '_>) -> String {
        match self {
            Self::Float => String::from("float"),
            Self::Int => String::from("int"),
            Self::Bool => String::from("bool"),
            Self::String => String::from("string"),
            Self::Array(array_type) => match array_type {
                Some(array_type) => {
                    format!("{}[]", array_type.format_detailed(state))
                }
                None => String::from("T[]"),
            },
            Self::Null => String::from("null"),
            Self::Unknown => String::from("Unknown"),
            Self::Union(types) => {
                types.into_iter().map(|x| x.format_detailed(state)).collect::<Vec<_>>().join("|")
            }
            Self::Struct(s) => {
                let s = &state.structs[*s as usize];
                format!(
                    "{} {{{}}}",
                    s.name,
                    s.fields
                        .iter()
                        .map(|field| format!(
                            "{}: {}",
                            field.name,
                            field.field_type.format_detailed(state)
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Self::Map(m) => format!(
                "{{{}: {}}}",
                m.0.as_ref().unwrap_or(&Self::Unknown),
                m.1.as_ref().unwrap_or(&Self::Unknown)
            ),
            Self::Fn(id) => {
                let f = &state.functions[*id as usize];
                format!("fn ({})", f.args.iter().map(|(a, _)| *a).collect::<Vec<&str>>().join(", "))
            }
            Self::FnSignature(sig) => {
                let (args, ret) = sig.split_at(sig.len() - 1);
                format!(
                    "fn({}) -> {}",
                    args.iter().map(|t| t.format_detailed(state)).collect::<Vec<_>>().join(", "),
                    ret[0].format_detailed(state)
                )
            }
        }
    }
    #[inline(always)]
    pub const fn is_indexable(&self) -> bool {
        matches!(self, Self::String | Self::Array(_) | Self::Unknown)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn to_c_type(
        &self,
        is_return_type: bool,
        span: Span,
        structs: &[Struct],
        file_idx: u16,
        sources: &[Source],
    ) -> Type {
        match self {
            Self::Int => libffi::middle::Type::i32(),
            Self::Float => libffi::middle::Type::f64(),
            Self::String => libffi::middle::Type::pointer(),
            Self::Array(elem_type) => {
                if let Some(elem_type) = elem_type {
                    elem_type.to_c_type(false, span, structs, file_idx, sources);
                }
                libffi::middle::Type::pointer()
            }
            Self::Bool => libffi::middle::Type::u8(),
            Self::Null if is_return_type => libffi::middle::Type::void(),
            Self::Struct(id) => {
                libffi::middle::Type::structure(structs[*id as usize].fields.iter().map(|field| {
                    field.field_type.to_c_type(
                        is_return_type,
                        field.span,
                        structs,
                        file_idx,
                        sources,
                    )
                }))
            }
            invalid_type => error_invalid_c_type(invalid_type, span, file_idx, sources),
        }
    }
}

impl PartialEq for DataType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // Array(None) is compatible with any array type
            (Self::Float, Self::Float)
            | (Self::Int, Self::Int)
            | (Self::Bool, Self::Bool)
            | (Self::String, Self::String)
            | (Self::Null, Self::Null)
            | (Self::Unknown, Self::Unknown)
            | (Self::Array(_), Self::Array(None))
            | (Self::Array(None), Self::Array(_)) => true,
            (Self::Array(Some(a)), Self::Array(Some(b))) => a == b,
            #[allow(clippy::suspicious_operation_groupings)]
            (Self::Union(a), Self::Union(b)) => {
                a == b || (a.iter().all(|x| b.contains(x)) && b.iter().all(|x| a.contains(x)))
            }
            (Self::FnSignature(a), Self::FnSignature(b)) => a == b,
            (Self::Struct(a), Self::Struct(b)) => a == b,
            (Self::Fn(fn_id), Self::Fn(fn_id_2)) => fn_id == fn_id_2,
            (Self::Map(a), Self::Map(b)) => {
                (a.0.is_none() || b.0.is_none() || a.0 == b.0)
                    && (a.1.is_none() || b.1.is_none() || a.1 == b.1)
            }
            (t, Self::Union(p)) | (Self::Union(p), t) => p.contains(t),
            _ => false,
        }
    }
}

impl std::hash::Hash for DataType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // All Array variants hash identically, which is required because Array(None) == Array(Some(_))
        match self {
            Self::Array(_) => 0u8.hash(state),
            Self::Float => 1u8.hash(state),
            Self::Int => 2u8.hash(state),
            Self::Bool => 3u8.hash(state),
            Self::String => 4u8.hash(state),
            Self::Null => 6u8.hash(state),
            Self::Unknown => 7u8.hash(state),
            Self::Union(p) => {
                8u8.hash(state);
                p.hash(state);
            }
            Self::Fn(f) => {
                9u8.hash(state);
                f.hash(state);
            }
            Self::Struct(s) => {
                10u8.hash(state);
                s.hash(state);
            }
            Self::Map(m) => {
                11u8.hash(state);
                m.hash(state);
            }
            Self::FnSignature(sig) => {
                12u8.hash(state);
                sig.hash(state);
            }
        }
    }
}

#[inline(always)]
pub fn struct_field_type_matches(expected: &DataType, received: &DataType) -> bool {
    received == &DataType::Null || expected == received
}

/// Collect all the function calls in the given code
pub fn collect_direct_fn_calls<'arena>(
    content: &'arena [Expr],
    calls: &mut Vec<QualifiedName<'arena>>,
) {
    let mut expr_stack: Vec<&Expr> = content.iter().collect();
    while let Some(expression) = expr_stack.pop() {
        match expression {
            Expr::FunctionCall(FunctionCallExpr { qualified_name, args, .. })
            | Expr::ObjFunctionCall(FunctionCallExpr { qualified_name, args, .. }) => {
                calls.push(*qualified_name);
                expr_stack.extend(*args);
            }
            Expr::IfBlock(if_block) => {
                expr_stack.push(if_block.condition);
                expr_stack.extend(if_block.then);
                expr_stack.extend(if_block.otherwise);
            }
            Expr::WhileBlock(condition, code) => {
                expr_stack.push(condition);
                expr_stack.extend(*code);
            }
            Expr::EvalBlock(x) | Expr::LoopBlock(x) => {
                expr_stack.extend(*x);
            }
            Expr::ReturnVal(code) => {
                if let Some(code) = code.as_ref() {
                    expr_stack.push(code);
                }
            }
            Expr::FunctionDecl(function_declaration) => {
                expr_stack.extend(function_declaration.code.iter());
            }
            Expr::ArrayGetSlice(x, y, z, _) => {
                expr_stack.push(x);
                expr_stack.push(y);
                expr_stack.push(z);
            }
            Expr::VarDeclare(VariableDeclarationExpr { value: x, .. })
            | Expr::VarAssign(_, x, _) => expr_stack.push(x),
            Expr::Neg(x, _, _) | Expr::BoolNeg(x, _, _) => {
                expr_stack.push(x);
            }
            Expr::ForLoop(_, _, code, _) => expr_stack.extend(*code),
            Expr::IntForLoop(int_for_loop) => {
                expr_stack.push(int_for_loop.get_lower_bound());
                expr_stack.push(int_for_loop.get_upper_bound());
                expr_stack.extend(int_for_loop.get_loop_code());
            }
            Expr::ArrayModify(array, index, value, _, _) => {
                expr_stack.push(array);
                expr_stack.push(index);
                expr_stack.push(value);
            }
            Expr::Array(elems, _) => expr_stack.extend(*elems),
            Expr::Struct(_, fields, _) => {
                expr_stack.extend(fields.iter().map(|field| &field.value));
            }
            Expr::GetStructField(expr, _, _, _) => expr_stack.push(expr),
            Expr::SetStructField(struct_field_assignment) => {
                expr_stack.push(struct_field_assignment.struct_expr);
                expr_stack.push(struct_field_assignment.field_value);
            }
            Expr::TryCatchBlock(try_code, _, catch_code) => {
                expr_stack.extend(*try_code);
                expr_stack.extend(*catch_code);
            }
            Expr::ArrayGetIndex(x, y, _)
            | Expr::Mul(x, y, _, _)
            | Expr::Div(x, y, _, _)
            | Expr::Add(x, y, _, _)
            | Expr::Sub(x, y, _, _)
            | Expr::Mod(x, y, _, _)
            | Expr::Pow(x, y, _, _)
            | Expr::Eq(x, y)
            | Expr::NotEq(x, y)
            | Expr::Sup(x, y, _, _)
            | Expr::SupEq(x, y, _, _)
            | Expr::Inf(x, y, _, _)
            | Expr::InfEq(x, y, _, _)
            | Expr::BoolAnd(x, y, _, _)
            | Expr::BoolOr(x, y, _, _) => {
                expr_stack.push(x);
                expr_stack.push(y);
            }
            _ => {}
        }
    }
}

pub fn c_arg_matches(inferred: &DataType, declared: &DataType) -> bool {
    match (inferred, declared) {
        (DataType::Union(_), _) => false,
        (DataType::Array(None), DataType::Array(_)) => true,
        (DataType::Array(Some(a)), DataType::Array(Some(b))) => c_arg_matches(a, b),
        _ => inferred == declared,
    }
}

/// Check if the function `src_fn_idx` can call `target_fn_idx` (index is in `fns`)
pub fn can_reach(
    src_fn_idx: usize,
    target_fn_idx: usize,
    fns: &[Function],
    visited: &mut FxHashSet<usize>,
    file_scopes: &[Scope],
) -> bool {
    let function = &fns[src_fn_idx];
    for callee in function.direct_calls {
        if let Some(callee_fn_idx) = file_scopes[function.src_file_idx as usize]
            .find_function_fallible(callee.get_namespace(), callee.get_name())
            && ((callee_fn_idx == target_fn_idx)
                || (visited.insert(callee_fn_idx)
                    && can_reach(callee_fn_idx, target_fn_idx, fns, visited, file_scopes)))
        {
            return true;
        }
    }
    false
}

pub fn check_if_returns_void(content: &[Expr]) -> bool {
    for content in content {
        match content {
            Expr::IfBlock(if_block) => {
                if !check_if_returns_void(if_block.then)
                    || !check_if_returns_void(if_block.otherwise)
                {
                    return false;
                }
            }
            Expr::WhileBlock(_, code)
            | Expr::ForLoop(_, _, code, _)
            | Expr::EvalBlock(code)
            | Expr::LoopBlock(code) => {
                if !check_if_returns_void(code) {
                    return false;
                }
            }
            Expr::IntForLoop(int_for_loop) => {
                if !check_if_returns_void(int_for_loop.get_loop_code()) {
                    return false;
                }
            }
            Expr::ReturnVal(return_val) if return_val.is_some() => {
                return false;
            }
            _ => {}
        }
    }
    true
}

macro_rules! add_return_type {
    ($return_types: expr, $return_type: expr) => {
        if $return_type != DataType::Unknown && !($return_types).contains(&($return_type)) {
            ($return_types).push($return_type);
        }
    };
}

macro_rules! extend_return_types {
    ($return_types: expr, $new_types: expr) => {
        for return_type in $new_types {
            add_return_type!($return_types, return_type);
        }
    };
}

pub fn track_returns<'arena>(
    content: &'arena [Expr],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    fn_name: &str,
) -> Vec<DataType> {
    let mut flow = track_return_flow(content, ctx, state, fn_name);
    if !flow.always_returns && !flow.types.is_empty() {
        add_return_type!(&mut flow.types, DataType::Null);
    }
    flow.types
}

/// Resolves the return type of the function of index `fn_id` in `state.fns`.
/// It only computes it once per argument types.
pub fn resolve_function_return_type(
    fn_id: usize,
    infered_arg_types: &[DataType],
    fn_name: &str,
    ctx: Ctx,
    state: &mut State<'_, '_>,
) -> DataType {
    if let Some((_, ret)) = state.functions[fn_id]
        .return_type_cache
        .iter()
        .find(|(args, _)| **args == *infered_arg_types)
    {
        return ret.clone();
    }

    let fn_args = state.functions[fn_id].args.clone();
    let fn_code = state.functions[fn_id].code;
    let fn_src_file = state.functions[fn_id].src_file_idx;

    let v_len_before_args = state.v.len();
    for (i, infered_type) in infered_arg_types.iter().cloned().enumerate() {
        // 0 => placeholder id, it's never used
        state.new_var(fn_args[i].0, 0, infered_type);
    }

    // Mutual-recursion cycle guard -> if we are already in the
    // middle of inferring this function's return type, return Unknown to break the cycle
    let already_inferring = RETURN_TYPE_INFERRING.with(|s| s.borrow().contains(&fn_id));
    if already_inferring {
        state.v.truncate(v_len_before_args);
        return DataType::Unknown;
    }

    RETURN_TYPE_INFERRING.with(|s| s.borrow_mut().insert(fn_id));

    let fn_ctx = ctx.with_file_idx(fn_src_file);
    let fn_type = track_returns(fn_code, fn_ctx, state, fn_name);

    RETURN_TYPE_INFERRING.with(|s| s.borrow_mut().remove(&fn_id));

    let to_return = if fn_type.is_empty() {
        // If function doesn't return anything, return nothing
        DataType::Null
    } else {
        // If function returns anything, check if it returns the same thing each time
        DataType::Union(Box::from(fn_type)).check_poly()
    };

    state.v.truncate(v_len_before_args);

    // Cache the result
    state.functions[fn_id]
        .return_type_cache
        .push((Box::from(infered_arg_types), to_return.clone()));

    to_return
}

pub fn fn_args_match(fn_id: usize, expected_args: &[DataType], state: &State<'_, '_>) -> bool {
    state.functions[fn_id].args.len() == expected_args.len()
        && !state.functions[fn_id]
            .args
            .iter()
            .zip(expected_args)
            .any(|((_, t), expected)| t.as_ref().is_some_and(|t| t != expected))
}

/// Checks whether the function wth index `fn_id` is compatible with the signature `expected_sig`.
pub fn fn_matches_signature(
    fn_id: usize,
    expected_sig: &[DataType],
    ctx: Ctx,
    state: &mut State<'_, '_>,
) -> bool {
    let expected_arg_types = &expected_sig[..expected_sig.len() - 1];
    let expected_return_type = &expected_sig[expected_sig.len() - 1];
    if !fn_args_match(fn_id, expected_arg_types, state) {
        return false;
    }
    let fn_name = state.functions[fn_id].name;
    resolve_function_return_type(fn_id, expected_arg_types, fn_name, ctx, state)
        == *expected_return_type
}

struct FnReturnFlow {
    types: Vec<DataType>,
    always_returns: bool,
}

fn track_scoped_returns<'arena>(
    code: &'arena [Expr],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    fn_name: &str,
) -> FnReturnFlow {
    let v_len = state.v.len();
    let flow = track_return_flow(code, ctx, state, fn_name);
    state.v.truncate(v_len);
    flow
}

fn track_condition_returns<'arena>(
    if_block: &'arena IfBlockExpr,
    fn_name: &str,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
) -> FnReturnFlow {
    let mut return_types = Vec::new();
    let then_flow = track_scoped_returns(if_block.then, ctx, state, fn_name);
    extend_return_types!(&mut return_types, then_flow.types);

    if if_block.otherwise.is_empty() {
        FnReturnFlow { types: return_types, always_returns: false }
    } else {
        let otherwise_flow = track_scoped_returns(if_block.otherwise, ctx, state, fn_name);
        extend_return_types!(&mut return_types, otherwise_flow.types);

        FnReturnFlow {
            types: return_types,
            always_returns: then_flow.always_returns && otherwise_flow.always_returns,
        }
    }
}

fn track_return_flow<'arena>(
    content: &'arena [Expr<'arena>],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    fn_name: &str,
) -> FnReturnFlow {
    let mut return_types: Vec<DataType> = Vec::new();
    for expr in content {
        match expr {
            Expr::IfBlock(if_block) => {
                let flow = track_condition_returns(if_block, fn_name, ctx, state);
                extend_return_types!(&mut return_types, flow.types);
                if flow.always_returns {
                    return FnReturnFlow { types: return_types, always_returns: true };
                }
            }
            Expr::EvalBlock(code) | Expr::LoopBlock(code) => {
                let flow = track_scoped_returns(code, ctx, state, fn_name);
                extend_return_types!(&mut return_types, flow.types);
                if flow.always_returns {
                    return FnReturnFlow { types: return_types, always_returns: true };
                }
            }
            Expr::VarDeclare(VariableDeclarationExpr { name, value, var_type: _, span: _ }) => {
                let var_type = value.infer_type(ctx, state);
                state.new_var(name, 0, var_type);
            }
            Expr::VarAssign(name, expr, _) => {
                let var_type = expr.infer_type(ctx, state);
                if let Some(var) = state.find_var_mut(name) {
                    var.var_type = var_type;
                }
            }
            Expr::WhileBlock(_, code) => {
                let flow = track_scoped_returns(code, ctx, state, fn_name);
                extend_return_types!(&mut return_types, flow.types);
            }
            Expr::IntForLoop(int_for_loop) => {
                let v_len = state.v.len();
                state.new_var(int_for_loop.var_name, 0, DataType::Int);
                let flow = track_return_flow(int_for_loop.get_loop_code(), ctx, state, fn_name);
                extend_return_types!(&mut return_types, flow.types);
                state.v.truncate(v_len);
            }
            Expr::ForLoop(var_name, array_expr, array_code, _) => {
                let inferred_collection_type = array_expr.infer_type(ctx, state);
                let elem_type = match inferred_collection_type {
                    DataType::Array(inner) => inner.map_or(DataType::Unknown, |t| *t),
                    DataType::String => DataType::String,
                    DataType::Unknown => DataType::Unknown,
                    _ => unsafe { unreachable_unchecked() },
                };
                let v_len = state.v.len();
                if *var_name != "_" {
                    state.new_var(var_name, 0, elem_type);
                }
                let flow = track_return_flow(array_code, ctx, state, fn_name);
                extend_return_types!(&mut return_types, flow.types);
                state.v.truncate(v_len);
            }
            Expr::ObjFunctionCall(function_call)
                if function_call.qualified_name.get_name() == "push" =>
            {
                if let Expr::Var(var_name, _) = &function_call.args[0]
                    && state
                        .v
                        .iter()
                        .rfind(|var| &var.name == var_name)
                        .is_some_and(|var| var.var_type == DataType::Array(None))
                {
                    let arg_type = function_call.args[1].infer_type(ctx, state);
                    if let Some(var) = state.find_var_mut(var_name) {
                        var.var_type = DataType::Array(Some(Box::new(arg_type)));
                    }
                }
            }
            Expr::ReturnVal(return_val) => {
                if let Some(val) = return_val.as_ref() {
                    let infered = val.infer_type(ctx, state);
                    add_return_type!(&mut return_types, infered);
                } else {
                    add_return_type!(&mut return_types, DataType::Null);
                }
                return FnReturnFlow { types: return_types, always_returns: true };
            }
            _ => {}
        }
    }
    FnReturnFlow { types: return_types, always_returns: false }
}

fn infer_symbol_type(
    namespace: &[&str],
    name: &str,
    span: Span,
    ctx: Ctx,
    state: &State<'_, '_>,
) -> DataType {
    if let Some(idx) =
        state.scope(ctx.file_idx).find_global(namespace, name, span, ctx.file_idx, state.sources)
    {
        state.globals[idx].var_type.clone()
    } else if let Some(fn_id) =
        state.scope(ctx.file_idx).find_function(namespace, name, span, ctx.file_idx, state.sources)
    {
        // When a function is referenced by name, there's no call site to infer argument types from
        // As such, functions refereced by name need to have all of their arguments typed
        if state.functions[fn_id].args.iter().any(|(_, t)| t.is_none()) {
            error_function_needs_args_typed(
                name,
                span,
                (state.functions[fn_id].name_span, state.functions[fn_id].src_file_idx),
                ctx.file_idx,
                state.sources,
            );
        }
        DataType::Fn(fn_id as u16)
    } else {
        error_unknown_variable(name, span, state.v, ctx.file_idx, state.sources);
    }
}

pub fn var_type_is_compatible(declared: &DataType, candidate: &DataType) -> bool {
    match (declared, candidate) {
        (DataType::Unknown, _) => true,
        (_, DataType::Union(types)) => types.iter().all(|t| var_type_is_compatible(declared, t)),
        (DataType::Union(types), _) => types.iter().any(|t| var_type_is_compatible(t, candidate)),
        _ => declared == candidate,
    }
}

impl<'arena> Expr<'arena> {
    pub fn infer_type(&'arena self, ctx: Ctx, state: &mut State<'arena, '_>) -> DataType {
        match self {
            Self::Var(name, span) => {
                if let Some(var) = state.find_var(name) {
                    var.var_type.clone()
                } else {
                    infer_symbol_type(&[], name, *span, ctx, state)
                }
            }
            Self::NamespacedVar(qualified_name, span) => infer_symbol_type(
                qualified_name.get_namespace(),
                qualified_name.get_name(),
                *span,
                ctx,
                state,
            ),
            Self::Float(_) => DataType::Float,
            Self::Int(_) => DataType::Int,
            Self::String(_) => DataType::String,
            Self::Bool(_) | Self::Eq(_, _) | Self::NotEq(_, _) | Self::TypeEq(_, _, _) => {
                DataType::Bool
            }
            Self::Null => DataType::Null,
            Self::Array(x, _) => DataType::Array(if x.is_empty() {
                None
            } else {
                let elem_type = x
                    .iter()
                    .map(|elem| elem.infer_type(ctx, state))
                    .find(|elem_type| *elem_type != DataType::Unknown)
                    .unwrap_or(DataType::Unknown);
                Some(Box::from(elem_type))
            }),
            Self::Map(kv_pairs, _) => {
                if kv_pairs.is_empty() {
                    DataType::Map(Box::from((Some(DataType::Unknown), Some(DataType::Unknown))))
                } else {
                    let kv_type = kv_pairs
                        .iter()
                        .map(|(key, _, value, _)| {
                            (key.infer_type(ctx, state), value.infer_type(ctx, state))
                        })
                        .find(|(key_t, val_t)| {
                            key_t != &DataType::Unknown || val_t != &DataType::Unknown
                        })
                        .map_or(
                            (Some(DataType::Unknown), Some(DataType::Unknown)),
                            |(key_t, val_t)| (Some(key_t), Some(val_t)),
                        );
                    DataType::Map(Box::from(kv_type))
                }
            }
            Self::Add(x, y, span_l, span_r) => {
                match (x.infer_type(ctx, state), y.infer_type(ctx, state)) {
                    (DataType::Unknown, t) | (t, DataType::Unknown) => t,
                    (DataType::Float, DataType::Float) => DataType::Float,
                    (DataType::Int, DataType::Int) => DataType::Int,
                    (DataType::String, DataType::String) => DataType::String,
                    (DataType::Array(t1), DataType::Array(t2)) => DataType::Array(t1.or(t2)),
                    (l, r) => {
                        error_op(&l, &r, "+", *span_l, *span_r, ctx.file_idx, state.sources);
                    }
                }
            }
            Self::Mul(x, y, span_l, span_r)
            | Self::Div(x, y, span_l, span_r)
            | Self::Sub(x, y, span_l, span_r)
            | Self::Mod(x, y, span_l, span_r)
            | Self::Pow(x, y, span_l, span_r) => {
                match (x.infer_type(ctx, state), y.infer_type(ctx, state)) {
                    (DataType::Unknown, t) | (t, DataType::Unknown)
                        if matches!(t, DataType::Float | DataType::Int | DataType::Unknown) =>
                    {
                        t
                    }
                    (DataType::Float, DataType::Float) => DataType::Float,
                    (DataType::Int, DataType::Int) => DataType::Int,
                    (l, r) => {
                        error_op(
                            &l,
                            &r,
                            symbol_of_expr(self),
                            *span_l,
                            *span_r,
                            ctx.file_idx,
                            state.sources,
                        );
                    }
                }
            }
            Self::Sup(x, y, span_l, span_r)
            | Self::SupEq(x, y, span_l, span_r)
            | Self::Inf(x, y, span_l, span_r)
            | Self::InfEq(x, y, span_l, span_r) => {
                match (x.infer_type(ctx, state), y.infer_type(ctx, state)) {
                    (DataType::Unknown, DataType::Float | DataType::Int)
                    | (DataType::Float | DataType::Int, DataType::Unknown)
                    | (DataType::Float, DataType::Float)
                    | (DataType::Int, DataType::Int) => DataType::Bool,
                    (l, r) => error_op(
                        &l,
                        &r,
                        symbol_of_expr(self),
                        *span_l,
                        *span_r,
                        ctx.file_idx,
                        state.sources,
                    ),
                }
            }
            Self::BoolAnd(x, y, span_l, span_r) | Self::BoolOr(x, y, span_l, span_r) => {
                match (x.infer_type(ctx, state), y.infer_type(ctx, state)) {
                    (DataType::Unknown | DataType::Bool, DataType::Bool)
                    | (DataType::Bool, DataType::Unknown) => DataType::Bool,
                    (l, r) => {
                        error_op(&l, &r, "&&", *span_l, *span_r, ctx.file_idx, state.sources);
                    }
                }
            }
            Self::Neg(e, span_l, span_r) => match e.infer_type(ctx, state) {
                DataType::Float => DataType::Float,
                DataType::Int => DataType::Int,
                DataType::Unknown => DataType::Unknown,
                operand_type => error_op(
                    &DataType::Null,
                    &operand_type,
                    "-",
                    *span_l,
                    *span_r,
                    ctx.file_idx,
                    state.sources,
                ),
            },
            Self::BoolNeg(e, span_l, span_r) => match e.infer_type(ctx, state) {
                DataType::Bool => DataType::Bool,
                operand_type => error_op(
                    &DataType::Null,
                    &operand_type,
                    "!",
                    *span_l,
                    *span_r,
                    ctx.file_idx,
                    state.sources,
                ),
            },
            Self::ArrayGetIndex(array, _, span) => match array.infer_type(ctx, state) {
                DataType::Array(array_type) => array_type.map_or(DataType::Null, |t| *t),
                DataType::String => DataType::String,
                DataType::Unknown => DataType::Unknown,
                t => error_type_not_indexable(&t, *span, false, ctx.file_idx, state.sources),
            },
            Self::GetStructField(s, field_name, struct_span, field_span) => {
                let s = s.infer_type(ctx, state);
                if let DataType::Struct(s_id) = s {
                    state.structs[s_id as usize]
                        .fields
                        .iter()
                        .find(|field| &field.name == field_name)
                        .unwrap_or_else(|| {
                            let s = &state.structs[s_id as usize];
                            error_struct_unknown_field(
                                ctx.file_idx,
                                *field_span,
                                field_name,
                                s.name,
                                &s.fields,
                                state.sources,
                            )
                        })
                        .field_type
                        .clone()
                } else {
                    error_invalid_type(
                        &DataType::Struct(0),
                        &s,
                        *struct_span,
                        None,
                        None,
                        ctx.file_idx,
                        state.sources,
                    );
                }
            }
            Self::ArrayGetSlice(array, _, _, _) => match array.infer_type(ctx, state) {
                DataType::Array(array_type) => DataType::Array(array_type),
                DataType::String => DataType::String,
                DataType::Unknown => DataType::Unknown,
                _ => unsafe { unreachable_unchecked() },
            },
            Self::FunctionCall(function_call) => {
                let qualified_name = &function_call.qualified_name;
                let args = &function_call.args;
                match qualified_name.get_name() {
                    "print" | "write" | "append" | "delete" | "delete_dir" => DataType::Null,
                    "type" | "string" | "input" | "read" => DataType::String,
                    "float" => DataType::Float,
                    "int" | "the_answer" => DataType::Int,
                    "bool" | "exists" => DataType::Bool,
                    "range" => DataType::Array(Some(Box::from(DataType::Int))),
                    "argv" => DataType::Array(Some(Box::from(DataType::String))),
                    function_name => {
                        if !qualified_name.is_namespace_empty()
                            && let Some(lib) = state
                                .dylibs
                                .iter()
                                .find(|l| &l.name == qualified_name.get_namespace().last().unwrap())
                            && let Some(FnSignature {
                                name: _,
                                args: _,
                                return_type: fn_return_type,
                                id: _,
                            }) = lib.fns.iter().find(|x| x.name == function_name)
                        {
                            return fn_return_type.clone();
                        }
                        let infered_arg_types = args
                            .iter()
                            .map(|x| x.infer_type(ctx, state))
                            .collect::<Vec<DataType>>();

                        let fn_id = if qualified_name.is_namespace_empty()
                            && let Some(var) = state.v.iter().rfind(|var| var.name == function_name)
                        {
                            if let DataType::Fn(id) = var.var_type {
                                id as usize
                            } else {
                                error_expected_function(
                                    &var.var_type,
                                    function_call.get_call_span(),
                                    ctx.file_idx,
                                    state.sources,
                                )
                            }
                        } else {
                            state.file_scopes[ctx.file_idx as usize]
                                .find_function(
                                    qualified_name.get_namespace(),
                                    function_name,
                                    function_call.spans[0],
                                    ctx.file_idx,
                                    state.sources,
                                )
                                .unwrap_or_else(|| {
                                    if qualified_name.is_namespace_empty() {
                                        error_unknown_function(
                                            function_name,
                                            function_call.get_call_span(),
                                            state.scope(ctx.file_idx),
                                            ctx.file_idx,
                                            state.sources,
                                        );
                                    } else {
                                        error_unknown_function_in_namespace(
                                            function_name,
                                            state.scope(ctx.file_idx),
                                            qualified_name.get_namespace(),
                                            function_call.get_call_span(),
                                            ctx.file_idx,
                                            state.sources,
                                        );
                                    }
                                })
                        };

                        resolve_function_return_type(
                            fn_id,
                            &infered_arg_types,
                            function_name,
                            ctx,
                            state,
                        )
                    }
                }
            }
            Self::ObjFunctionCall(function_call) => {
                let obj = &function_call.args[0];
                match function_call.qualified_name.get_name() {
                    "uppercase"
                    | "lowercase"
                    | "replace"
                    | "trim"
                    | "trim_sequence"
                    | "trim_left"
                    | "trim_right"
                    | "trim_sequence_left"
                    | "trim_sequence_right"
                    | "join" => DataType::String,
                    "starts_with" | "ends_with" | "contains" | "is_float" | "is_int" => {
                        DataType::Bool
                    }
                    "len" | "find" => DataType::Int,
                    "repeat" | "reverse" => {
                        let obj_type = obj.infer_type(ctx, state);
                        if obj_type == DataType::String {
                            DataType::String
                        } else if let DataType::Array(array_type) = obj_type {
                            DataType::Array(array_type)
                        } else {
                            unsafe { unreachable_unchecked() }
                        }
                    }
                    "push" | "sort" | "remove" | "insert" => DataType::Null,
                    "sqrt" | "round" | "floor" => DataType::Float,
                    "abs" => {
                        let obj_type = obj.infer_type(ctx, state);
                        if obj_type == DataType::Float {
                            DataType::Float
                        } else if obj_type == DataType::Int {
                            DataType::Int
                        } else {
                            unsafe { unreachable_unchecked() }
                        }
                    }
                    "split" => DataType::Array(Some(Box::from(DataType::String))),
                    "partition" => {
                        let obj_type = obj.infer_type(ctx, state);
                        if let DataType::Array(array_type) = obj_type {
                            DataType::Array(Some(Box::from(DataType::Array(array_type))))
                        } else {
                            unsafe { unreachable_unchecked() }
                        }
                    }
                    "get" => {
                        let obj_type = obj.infer_type(ctx, state);
                        if let DataType::Map(m) = obj_type {
                            m.1.unwrap_or(DataType::Unknown)
                        } else {
                            unsafe { unreachable_unchecked() }
                        }
                    }
                    "map" => {
                        let obj_type = obj.infer_type(ctx, state);
                        let fn_type = function_call.args[1].infer_type(ctx, state);
                        let DataType::Fn(fn_id) = fn_type else {
                            error_expected_function(
                                &fn_type,
                                function_call.get_nth_arg_span(0),
                                ctx.file_idx,
                                state.sources,
                            );
                        };
                        if obj_type == DataType::String {
                            DataType::String
                        } else if let DataType::Array(elem_type) = obj_type {
                            let elem_type = match elem_type {
                                Some(t) => *t,
                                None => state.functions[fn_id as usize].args[0]
                                    .1
                                    .clone()
                                    .unwrap_or(DataType::Unknown),
                            };
                            DataType::Array(Some(Box::new(resolve_function_return_type(
                                fn_id as usize,
                                &[elem_type],
                                "map",
                                ctx,
                                state,
                            ))))
                        } else {
                            unsafe { unreachable_unchecked() }
                        }
                    }
                    "filter" => {
                        let fn_type = function_call.args[1].infer_type(ctx, state);
                        if !matches!(fn_type, DataType::Fn(_)) {
                            error_expected_function(
                                &fn_type,
                                function_call.get_nth_arg_span(0),
                                ctx.file_idx,
                                state.sources,
                            );
                        }
                        obj.infer_type(ctx, state)
                    }
                    _ => unsafe { unreachable_unchecked() },
                }
            }
            Self::IfBlock(if_block) => {
                let mut types: Vec<DataType> = Vec::with_capacity(2);
                if let Some(then) = if_block.then.last() {
                    types.push(then.infer_type(ctx, state));
                }
                if let Some(otherwise) = if_block.otherwise.last() {
                    match otherwise.infer_type(ctx, state) {
                        DataType::Union(t) => types.extend(t),
                        t => {
                            if !types.contains(&t) {
                                types.push(t);
                            }
                        }
                    }
                }
                DataType::Union(Box::from(types)).check_poly()
            }
            Self::Struct(name, _, span) => {
                let struct_name = name.get_name();
                let namespace = name.get_namespace();
                DataType::Struct(
                    state
                        .scope(ctx.file_idx)
                        .find_struct(namespace, struct_name, *span, ctx.file_idx, state.sources)
                        .unwrap_or_else(|| {
                            error_unknown_struct(struct_name, *span, state.sources, ctx.file_idx);
                        }) as u16,
                )
            }
            Self::AnonymousFunction(args, code, span) => {
                let fn_name =
                    bumpalo::format!(in state.bump,"{}{}{}", ctx.file_idx, span.start, span.end);
                let returns_null = check_if_returns_void(code);
                let mut callees = Vec::new();
                collect_direct_fn_calls(code, &mut callees);
                let id = state.functions.len() as u16;
                state.functions.push(Function {
                    name: fn_name.into_bump_str(),
                    args: args
                        .iter()
                        .map(|(name, t)| {
                            (
                                *name,
                                t.as_ref().map(|t| {
                                    t.to_datatype(
                                        ctx.file_idx,
                                        state.scope(ctx.file_idx),
                                        state.sources,
                                    )
                                }),
                            )
                        })
                        .collect::<Vec<(&str, Option<DataType>)>>()
                        .into_boxed_slice(),
                    code,
                    impls: Vec::new(),
                    is_recursive: None,
                    returns_null,
                    src_file_idx: ctx.file_idx,
                    return_type_cache: Vec::new(),
                    direct_calls: state.bump.alloc_slice_copy(&callees),
                    name_span: *span,
                });
                state.fn_registers.push(Vec::new());
                DataType::Fn(id)
            }
            _ => unsafe { unreachable_unchecked() },
        }
    }
}

impl DataType {
    pub fn check_poly(self) -> Self {
        if let Self::Union(ref elems) = self {
            if let Some(new) = reduce_null_struct(elems) {
                return new;
            }
            let mut concrete = elems.iter().filter(|elem_type| **elem_type != Self::Unknown);
            if let Some(first_type) = concrete.next() {
                if concrete.all(|x| x == first_type) { first_type.clone() } else { self }
            } else if !elems.is_empty() {
                Self::Unknown
            } else {
                unsafe { unreachable_unchecked() }
            }
        } else {
            unsafe { unreachable_unchecked() }
        }
    }
}

fn reduce_null_struct(types: &[DataType]) -> Option<DataType> {
    let mut struct_type = None;
    for t in types {
        match t {
            DataType::Null | DataType::Unknown => {}
            DataType::Struct(_) => {
                if let Some(struct_type) = &struct_type {
                    if struct_type != t {
                        return None;
                    }
                } else {
                    struct_type = Some(t.clone());
                }
            }
            _ => return None,
        }
    }
    struct_type
}
