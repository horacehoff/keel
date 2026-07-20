use super::expr::Expr;
use super::expr::Span;
use super::expr::symbol_of_expr;
use crate::compiler::Namespace;
use crate::compiler::SymbolKind;
use crate::compiler::compiler_data::Ctx;
use crate::compiler::compiler_data::FnSignature;
use crate::compiler::compiler_data::Function;
use crate::compiler::compiler_data::Source;
use crate::compiler::compiler_data::State;
use crate::compiler::compiler_data::Struct;
use crate::compiler::compiler_data::Variable;
use crate::compiler::compiler_errors::error_op;
use crate::compiler::compiler_errors::error_struct_unknown_field;
use crate::compiler::compiler_errors::error_unknown_function;
use crate::compiler::compiler_errors::error_unknown_function_in_namespace;
use crate::compiler::compiler_errors::error_unknown_struct;
use crate::compiler::compiler_errors::error_unknown_type;
use crate::compiler::compiler_errors::error_unknown_type_with_namespace;
use crate::compiler::compiler_errors::error_unknown_variable;
use crate::compiler::find_struct;
use crate::compiler::walk_namespace;
use crate::errors::ErrType;
use crate::errors::throw_compiler_error;
use rustc_hash::FxHashSet;
use smol_strc::SmolStr;
use smol_strc::ToSmolStr;
use std::cell::RefCell;
use std::collections::HashSet;
use std::hint::cold_path;
use std::hint::unreachable_unchecked;
use std::rc::Rc;

#[cfg(not(target_arch = "wasm32"))]
use libffi::middle::Type;

// Tracks which user-defined functions are currently being analysed for their
// return type. Used to break mutual-recursion cycles in type inference
thread_local! {
    static RETURN_TYPE_INFERRING: RefCell<FxHashSet<usize>> =
        RefCell::new(FxHashSet::default());
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TypeExpr {
    Identifier(SmolStr, Span),
    NamespacedIdentifier(Box<[SmolStr]>, Span),
    Array(Box<Self>),
    Map(Box<Self>, Box<Self>),
    Union(Box<[Self]>),
}

impl TypeExpr {
    pub fn to_datatype(
        &self,
        structs: &[Struct],
        span: Span,
        src: Source,
        namespace: &Namespace,
        sources: &[(SmolStr, Rc<String>)],
    ) -> DataType {
        match self {
            Self::Identifier(s, span) => match s.as_str() {
                "int" => DataType::Int,
                "float" => DataType::Float,
                "bool" => DataType::Bool,
                "string" => DataType::String,
                "null" => DataType::Null,
                other => {
                    if let Some(SymbolKind::Struct(id)) = walk_namespace(namespace, &[], other) {
                        DataType::Struct(id as u16)
                    } else {
                        error_unknown_type(
                            *span,
                            src,
                            other,
                            sources,
                            namespace.symbols.iter().filter_map(|(name, kind)| {
                                if matches!(kind, SymbolKind::Struct(_)) {
                                    Some(name.as_str())
                                } else {
                                    None
                                }
                            }),
                        );
                    }
                }
            },
            Self::NamespacedIdentifier(s, span) => {
                if let Some(SymbolKind::Struct(id)) =
                    walk_namespace(namespace, &s[..s.len() - 1], unsafe {
                        s.last().unwrap_unchecked()
                    })
                {
                    DataType::Struct(id as u16)
                } else {
                    cold_path();
                    error_unknown_type_with_namespace(
                        *span,
                        src,
                        unsafe { s.last().unwrap_unchecked() },
                        sources,
                        namespace,
                        &s[..s.len() - 1],
                    )
                }
            }
            Self::Array(inner_t) => DataType::Array(Some(Box::new(
                inner_t.to_datatype(structs, span, src, namespace, sources),
            ))),
            Self::Map(k_t, v_t) => DataType::Map(Box::from((
                Some(k_t.to_datatype(structs, span, src, namespace, sources)),
                Some(v_t.to_datatype(structs, span, src, namespace, sources)),
            ))),
            Self::Union(poly) => DataType::Poly(
                poly.iter()
                    .map(|t| t.to_datatype(structs, span, src, namespace, sources))
                    .collect(),
            )
            .check_poly(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DataType {
    /// Array(None) = unknown element type (e.g. empty array literal [])
    Array(Option<Box<Self>>),
    Float,
    Int,
    Bool,
    String,
    Null,
    /// Internal inference placeholder used while breaking recursive return-type cycles
    Unknown,
    Poly(Box<[Self]>),
    /// Fn (\[arg_types ... return_type\]) => return_type is always specified
    Fn(u16),
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
                None => write!(f, "Unknown[]"),
            },
            Self::Null => write!(f, "null"),
            Self::Unknown => write!(f, "Unknown"),
            Self::Poly(types) => write!(
                f,
                "{}",
                types
                    .into_iter()
                    .map(|x| format!("{x}"))
                    .collect::<Vec<_>>()
                    .join("|")
            ),
            Self::Struct(_) => write!(f, "struct"),
            Self::Map(m) => write!(
                f,
                "{{{}: {}}}",
                m.0.as_ref().unwrap_or(&Self::Unknown),
                m.1.as_ref().unwrap_or(&Self::Unknown)
            ),
            Self::Fn(_) => write!(f, "function"),
        }
    }
}

impl DataType {
    pub fn format_detailed(&self, state: &State<'_>) -> SmolStr {
        match self {
            Self::Float => SmolStr::new_static("float"),
            Self::Int => SmolStr::new_static("int"),
            Self::Bool => SmolStr::new_static("bool"),
            Self::String => SmolStr::new_static("string"),
            Self::Array(array_type) => match array_type {
                Some(array_type) => {
                    format_args!("{}[]", array_type.format_detailed(state)).to_smolstr()
                }
                None => SmolStr::new_static("Unknown[]"),
            },
            Self::Null => SmolStr::new_static("null"),
            Self::Unknown => SmolStr::new_static("Unknown"),
            Self::Poly(types) => format_args!(
                "{}",
                types
                    .into_iter()
                    .map(|x| x.format_detailed(state))
                    .collect::<Vec<SmolStr>>()
                    .join("|")
            )
            .to_smolstr(),
            Self::Struct(s) => {
                let s = &state.structs[*s as usize];
                format_args!(
                    "{} {{{}}}",
                    s.name,
                    s.fields
                        .iter()
                        .map(
                            |(n, t, _)| format_args!("{n}: {}", t.format_detailed(state))
                                .to_smolstr()
                        )
                        .collect::<Vec<SmolStr>>()
                        .join(", ")
                )
                .to_smolstr()
            }
            Self::Map(m) => format_args!(
                "{{{}: {}}}",
                m.0.as_ref().unwrap_or(&Self::Unknown),
                m.1.as_ref().unwrap_or(&Self::Unknown)
            )
            .to_smolstr(),
            Self::Fn(id) => {
                let f = &state.fns[*id as usize];
                format_args!(
                    "fn ({})",
                    f.args
                        .iter()
                        .map(|(a, _)| a.clone())
                        .collect::<Vec<SmolStr>>()
                        .join(", ")
                )
                .to_smolstr()
            }
        }
    }
    #[inline(always)]
    pub const fn is_indexable(&self) -> bool {
        matches!(self, Self::String | Self::Array(_) | Self::Unknown)
    }
    #[inline(always)]
    pub fn expect(&self, expected: &Self, src: Source, span: Span) {
        if self != expected {
            cold_path();
            throw_compiler_error(src, span, ErrType::InvalidType(expected, self));
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
            (Self::Poly(a), Self::Poly(b)) => a == b,
            (Self::Struct(a), Self::Struct(b)) => a == b,
            (Self::Fn(_), Self::Fn(_)) => true,
            (Self::Map(a), Self::Map(b)) => {
                (a.0.is_none() || b.0.is_none() || a.0 == b.0)
                    && (a.1.is_none() || b.1.is_none() || a.1 == b.1)
            }
            (t, Self::Poly(p)) | (Self::Poly(p), t) => p.contains(t),
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
            Self::Poly(p) => {
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
        }
    }
}

#[inline(always)]
pub fn struct_field_type_matches(expected: &DataType, received: &DataType) -> bool {
    received == &DataType::Null || expected == received
}

/// Collect all the function calls in the given code
pub fn collect_direct_fn_calls(content: &[Expr], calls: &mut Vec<SmolStr>) {
    let mut expr_stack: Vec<&Expr> = content.iter().collect();
    while let Some(expression) = expr_stack.pop() {
        match expression {
            Expr::FunctionCall(args, namespace, _, _) => {
                calls.push(namespace.last().unwrap().clone());
                expr_stack.extend(args.iter());
            }
            Expr::Condition(x, y, _)
            | Expr::InlineCondition(x, y, _)
            | Expr::ElseIfBlock(x, y)
            | Expr::WhileBlock(x, y)
            | Expr::ObjFunctionCall(x, y, _, _, _, _) => {
                expr_stack.push(x);
                expr_stack.extend(y.iter());
            }
            Expr::ElseBlock(x) | Expr::EvalBlock(x) | Expr::LoopBlock(x) => {
                expr_stack.extend(x.iter());
            }
            Expr::ReturnVal(code) => {
                if let Some(code) = code.as_ref() {
                    expr_stack.push(code);
                }
            }
            Expr::FunctionDecl(_, _, x, _) => expr_stack.extend(x.iter()),
            Expr::ArrayGetSlice(x, y, z, _) => {
                expr_stack.push(x);
                expr_stack.push(y);
                expr_stack.push(z);
            }
            Expr::VarDeclare(_, x)
            | Expr::VarAssign(_, x, _)
            | Expr::Neg(x, _, _)
            | Expr::BoolNeg(x, _, _) => expr_stack.push(x),
            Expr::ForLoop(_, _, code, _) => expr_stack.extend(code.iter()),
            Expr::IntForLoop(_, start, end, code, _, _) => {
                expr_stack.push(start);
                expr_stack.push(end);
                expr_stack.extend(code.iter());
            }
            Expr::ArrayModify(array, index, value, _, _) => {
                expr_stack.push(array);
                expr_stack.push(index);
                expr_stack.push(value);
            }
            Expr::Array(elems, _) => expr_stack.extend(elems.iter()),
            Expr::Struct(_, fields, _) => {
                expr_stack.extend(fields.iter().map(|(_, expr, _, _)| expr));
            }
            Expr::GetStructField(expr, _, _, _) => expr_stack.push(expr),
            Expr::SetStructField(expr, _, value, _, _, _) => {
                expr_stack.push(expr);
                expr_stack.push(value);
            }
            Expr::TryCatchBlock(try_code, _, catch_code) => {
                expr_stack.extend(try_code.iter());
                expr_stack.extend(catch_code.iter());
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

/// Check if the function src_fn can call target_fn
pub fn can_reach(
    src_fn: &str,
    target_fn: &str,
    fns: &[Function],
    visited: &mut HashSet<SmolStr>,
) -> bool {
    if let Some(from_fn) = fns.iter().find(|f| f.name.as_str() == src_fn) {
        for callee in &from_fn.direct_calls {
            if callee == target_fn {
                return true;
            }
            if visited.insert(callee.clone()) && can_reach(callee, target_fn, fns, visited) {
                return true;
            }
        }
    }
    false
}

pub fn check_if_returns_void(content: &[Expr]) -> bool {
    for content in content {
        match content {
            Expr::ElseIfBlock(_, code)
            | Expr::ElseBlock(code)
            | Expr::Condition(_, code, _)
            | Expr::InlineCondition(_, code, _)
            | Expr::WhileBlock(_, code)
            | Expr::ForLoop(_, _, code, _)
            | Expr::EvalBlock(code)
            | Expr::LoopBlock(code)
            | Expr::IntForLoop(_, _, _, code, _, _) => {
                if !check_if_returns_void(code) {
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

pub fn track_returns(
    content: &[Expr],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    fn_name: &str,
) -> Vec<DataType> {
    let mut flow = track_return_flow(content, v, ctx, state, fn_name);
    if !flow.always_returns && !flow.types.is_empty() {
        add_return_type!(&mut flow.types, DataType::Null);
    }
    flow.types
}

struct FnReturnFlow {
    types: Vec<DataType>,
    always_returns: bool,
}

fn track_scoped_returns(
    code: &[Expr],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    fn_name: &str,
) -> FnReturnFlow {
    let v_len = v.len();
    let flow = track_return_flow(code, v, ctx, state, fn_name);
    v.truncate(v_len);
    flow
}

fn track_condition_returns(
    code: &[Expr],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    fn_name: &str,
) -> FnReturnFlow {
    let mut return_types = Vec::new();
    let first_branch_end = code
        .iter()
        .position(|expr| matches!(expr, Expr::ElseIfBlock(_, _) | Expr::ElseBlock(_)))
        .unwrap_or(code.len());

    let first_flow = track_scoped_returns(&code[..first_branch_end], v, ctx, state, fn_name);
    let mut all_branches_return = first_flow.always_returns;
    let mut has_else = false;
    extend_return_types!(&mut return_types, first_flow.types);

    for expr in &code[first_branch_end..] {
        match expr {
            Expr::ElseIfBlock(_, branch_code) => {
                let flow = track_scoped_returns(branch_code, v, ctx, state, fn_name);
                all_branches_return &= flow.always_returns;
                extend_return_types!(&mut return_types, flow.types);
            }
            Expr::ElseBlock(branch_code) => {
                has_else = true;
                let flow = track_scoped_returns(branch_code, v, ctx, state, fn_name);
                all_branches_return &= flow.always_returns;
                extend_return_types!(&mut return_types, flow.types);
            }
            _ => {}
        }
    }

    FnReturnFlow {
        types: return_types,
        always_returns: has_else && all_branches_return,
    }
}

fn track_return_flow(
    content: &[Expr],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    fn_name: &str,
) -> FnReturnFlow {
    let mut return_types: Vec<DataType> = Vec::new();
    for expr in content {
        match expr {
            Expr::Condition(_, code, _) | Expr::InlineCondition(_, code, _) => {
                let flow = track_condition_returns(code, v, ctx, state, fn_name);
                extend_return_types!(&mut return_types, flow.types);
                if flow.always_returns {
                    return FnReturnFlow {
                        types: return_types,
                        always_returns: true,
                    };
                }
            }
            Expr::ElseIfBlock(_, code)
            | Expr::ElseBlock(code)
            | Expr::EvalBlock(code)
            | Expr::LoopBlock(code) => {
                let flow = track_scoped_returns(code, v, ctx, state, fn_name);
                extend_return_types!(&mut return_types, flow.types);
                if flow.always_returns {
                    return FnReturnFlow {
                        types: return_types,
                        always_returns: true,
                    };
                }
            }
            Expr::VarDeclare(name, expr) => {
                let var_type = expr.infer_type(v, ctx, state);
                v.push(Variable {
                    name: name.clone(),
                    register_id: 0,
                    var_type,
                });
            }
            Expr::VarAssign(name, expr, _) => {
                let var_type = expr.infer_type(v, ctx, state);
                if let Some(var) = v.iter_mut().rfind(|var| &var.name == name) {
                    var.var_type = var_type;
                }
            }
            Expr::WhileBlock(_, code) => {
                let flow = track_scoped_returns(code, v, ctx, state, fn_name);
                extend_return_types!(&mut return_types, flow.types);
            }
            Expr::IntForLoop(var_name, _, _, code, _, _) => {
                let v_len = v.len();
                v.push(Variable {
                    name: var_name.clone(),
                    register_id: 0,
                    var_type: DataType::Int,
                });
                let flow = track_return_flow(code, v, ctx, state, fn_name);
                extend_return_types!(&mut return_types, flow.types);
                v.truncate(v_len);
            }
            Expr::ForLoop(var_name, array_expr, array_code, _) => {
                let inferred_collection_type = array_expr.infer_type(v, ctx, state);
                let elem_type = match inferred_collection_type {
                    DataType::Array(inner) => inner.map_or(DataType::Unknown, |t| *t),
                    DataType::String => DataType::String,
                    DataType::Unknown => DataType::Unknown,
                    _ => unsafe { unreachable_unchecked() },
                };
                let v_len = v.len();
                if var_name.as_str() != "_" {
                    v.push(Variable {
                        name: var_name.clone(),
                        register_id: 0,
                        var_type: elem_type,
                    });
                }
                let flow = track_return_flow(array_code, v, ctx, state, fn_name);
                extend_return_types!(&mut return_types, flow.types);
                v.truncate(v_len);
            }
            Expr::ObjFunctionCall(obj, args, namespace, _, _, _)
                if namespace.last().unwrap().as_str() == "push" =>
            {
                if let Expr::Var(var_name, _) = obj.as_ref()
                    && v.iter()
                        .rfind(|var| &var.name == var_name)
                        .is_some_and(|var| var.var_type == DataType::Array(None))
                {
                    let arg_type = args[0].infer_type(v, ctx, state);
                    if let Some(var) = v.iter_mut().rfind(|var| &var.name == var_name) {
                        var.var_type = DataType::Array(Some(Box::new(arg_type)));
                    }
                }
            }
            Expr::ReturnVal(return_val) => {
                if let Some(val) = return_val.as_ref() {
                    let infered = val.infer_type(v, ctx, state);
                    add_return_type!(&mut return_types, infered);
                } else {
                    add_return_type!(&mut return_types, DataType::Null);
                }
                return FnReturnFlow {
                    types: return_types,
                    always_returns: true,
                };
            }
            _ => {}
        }
    }
    FnReturnFlow {
        types: return_types,
        always_returns: false,
    }
}

impl Expr {
    pub fn infer_type(
        &self,
        v: &mut Vec<Variable>,
        ctx: Ctx<'_>,
        state: &mut State<'_>,
    ) -> DataType {
        match self {
            Self::Var(name, span) => v
                .iter()
                .rfind(|x| &x.name == name)
                .unwrap_or_else(|| {
                    error_unknown_variable(name, *span, v, ctx.src, state.sources);
                })
                .var_type
                .clone(),
            Self::Float(_) => DataType::Float,
            Self::Int(_) => DataType::Int,
            Self::String(_) => DataType::String,
            Self::Bool(_) | Self::Eq(_, _) | Self::NotEq(_, _) => DataType::Bool,
            Self::Null => DataType::Null,
            Self::Array(x, _) => DataType::Array(if x.is_empty() {
                None
            } else {
                let elem_type = x
                    .iter()
                    .map(|elem| elem.infer_type(v, ctx, state))
                    .find(|elem_type| *elem_type != DataType::Unknown)
                    .unwrap_or(DataType::Unknown);
                Some(Box::from(elem_type))
            }),
            Self::Map(kv_pairs, _) => {
                if kv_pairs.is_empty() {
                    DataType::Map(Box::from((
                        Some(DataType::Unknown),
                        Some(DataType::Unknown),
                    )))
                } else {
                    let kv_type = kv_pairs
                        .iter()
                        .map(|(key, value)| {
                            (
                                key.infer_type(v, ctx, state),
                                value.infer_type(v, ctx, state),
                            )
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
                match (x.infer_type(v, ctx, state), y.infer_type(v, ctx, state)) {
                    (DataType::Unknown, t) | (t, DataType::Unknown) => t,
                    (DataType::Float, DataType::Float) => DataType::Float,
                    (DataType::Int, DataType::Int) => DataType::Int,
                    (DataType::String, DataType::String) => DataType::String,
                    (DataType::Array(t1), DataType::Array(t2)) => DataType::Array(t1.or(t2)),
                    (l, r) => {
                        error_op(&l, &r, "+", *span_l, *span_r, ctx.src, state.sources);
                    }
                }
            }
            Self::Mul(x, y, span_l, span_r)
            | Self::Div(x, y, span_l, span_r)
            | Self::Sub(x, y, span_l, span_r)
            | Self::Mod(x, y, span_l, span_r)
            | Self::Pow(x, y, span_l, span_r) => {
                match (x.infer_type(v, ctx, state), y.infer_type(v, ctx, state)) {
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
                            ctx.src,
                            state.sources,
                        );
                    }
                }
            }
            Self::Sup(x, y, span_l, span_r)
            | Self::SupEq(x, y, span_l, span_r)
            | Self::Inf(x, y, span_l, span_r)
            | Self::InfEq(x, y, span_l, span_r) => {
                match (x.infer_type(v, ctx, state), y.infer_type(v, ctx, state)) {
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
                        ctx.src,
                        state.sources,
                    ),
                }
            }
            Self::BoolAnd(x, y, span_l, span_r) | Self::BoolOr(x, y, span_l, span_r) => {
                match (x.infer_type(v, ctx, state), y.infer_type(v, ctx, state)) {
                    (DataType::Unknown | DataType::Bool, DataType::Bool)
                    | (DataType::Bool, DataType::Unknown) => DataType::Bool,
                    (l, r) => {
                        error_op(&l, &r, "&&", *span_l, *span_r, ctx.src, state.sources);
                    }
                }
            }
            Self::Neg(e, span_l, span_r) => match e.infer_type(v, ctx, state) {
                DataType::Float => DataType::Float,
                DataType::Int => DataType::Int,
                DataType::Unknown => DataType::Unknown,
                operand_type => error_op(
                    &DataType::Null,
                    &operand_type,
                    "-",
                    *span_l,
                    *span_r,
                    ctx.src,
                    state.sources,
                ),
            },
            Self::BoolNeg(e, span_l, span_r) => match e.infer_type(v, ctx, state) {
                DataType::Bool => DataType::Bool,
                operand_type => error_op(
                    &DataType::Null,
                    &operand_type,
                    "!",
                    *span_l,
                    *span_r,
                    ctx.src,
                    state.sources,
                ),
            },
            Self::ArrayGetIndex(array, _, _) => match array.infer_type(v, ctx, state) {
                DataType::Array(array_type) => array_type.map_or(DataType::Null, |t| *t),
                DataType::String => DataType::String,
                DataType::Unknown => DataType::Unknown,
                _ => unsafe { unreachable_unchecked() },
            },
            Self::GetStructField(s, field, struct_span, field_span) => {
                let s = s.infer_type(v, ctx, state);
                if let DataType::Struct(s_id) = s {
                    state.structs[s_id as usize]
                        .fields
                        .iter()
                        .find(|x| &x.0 == field)
                        .unwrap_or_else(|| {
                            let s = &state.structs[s_id as usize];
                            error_struct_unknown_field(
                                ctx.src,
                                *field_span,
                                field,
                                &s.name,
                                &s.fields,
                                state.sources,
                            )
                        })
                        .1
                        .clone()
                } else {
                    throw_compiler_error(
                        ctx.src,
                        *struct_span,
                        ErrType::InvalidType(&DataType::Struct(0), &s),
                    );
                }
            }
            Self::ArrayGetSlice(array, _, _, _) => match array.infer_type(v, ctx, state) {
                DataType::Array(array_type) => DataType::Array(array_type),
                DataType::String => DataType::String,
                DataType::Unknown => DataType::Unknown,
                _ => unsafe { unreachable_unchecked() },
            },
            Self::FunctionCall(args, namespace, span, _) => {
                match namespace.last().unwrap().as_str() {
                    "print" | "write" | "append" | "delete" | "delete_dir" => DataType::Null,
                    "type" | "str" | "input" | "read" => DataType::String,
                    "float" => DataType::Float,
                    "int" | "the_answer" => DataType::Int,
                    "bool" | "exists" => DataType::Bool,
                    "range" => DataType::Array(Some(Box::from(DataType::Int))),
                    "argv" => DataType::Array(Some(Box::from(DataType::String))),
                    function_name => {
                        if let Some(lib) = state.dyn_libs.iter().find(|l| l.name == namespace[0])
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
                            .map(|x| x.infer_type(v, ctx, state))
                            .collect::<Vec<DataType>>();

                        let fn_id = state
                            .fns
                            .iter()
                            .rposition(|func| func.name == function_name)
                            .unwrap_or_else(|| {
                                if namespace.len() == 1 {
                                    error_unknown_function(
                                        function_name,
                                        *span,
                                        state.namespace.symbols.iter().filter_map(
                                            |(name, kind)| {
                                                if matches!(kind, SymbolKind::Fn(_)) {
                                                    Some(name.as_str())
                                                } else {
                                                    None
                                                }
                                            },
                                        ),
                                        ctx.src,
                                        state.sources,
                                    );
                                } else {
                                    error_unknown_function_in_namespace(
                                        function_name,
                                        state.namespace,
                                        &namespace[..namespace.len() - 1],
                                        *span,
                                        ctx.src,
                                        state.sources,
                                    );
                                }
                            });

                        let func = &state.fns[fn_id];
                        // Check the return type cache
                        if let Some((_, ret)) = func
                            .return_type_cache
                            .iter()
                            .find(|(args, _)| **args == *infered_arg_types)
                        {
                            return ret.clone();
                        }

                        let fn_args = func.args.clone();
                        let fn_code = func.code.clone();
                        let v_len_before_args = v.len();
                        for (i, infered_type) in infered_arg_types.iter().cloned().enumerate() {
                            // 0 => placeholder id, it's never used
                            v.push(Variable {
                                name: fn_args[i].0.clone(),
                                register_id: 0,
                                var_type: infered_type,
                            });
                        }

                        // Mutual-recursion cycle guard -> if we are already in the
                        // middle of inferring this function's return type, return Null to break the cycle
                        let already_inferring =
                            RETURN_TYPE_INFERRING.with(|s| s.borrow().contains(&fn_id));
                        if already_inferring {
                            v.truncate(v_len_before_args);
                            return DataType::Unknown;
                        }

                        RETURN_TYPE_INFERRING.with(|s| s.borrow_mut().insert(fn_id));

                        let (fn_src_name, fn_src_contents) =
                            state.sources[func.src_file as usize].clone();
                        let fn_ctx = Ctx {
                            src: Source {
                                filename: fn_src_name.as_str(),
                                contents: fn_src_contents.as_str(),
                            },
                            current_src_file: func.src_file,
                            ..ctx
                        };
                        let fn_type = track_returns(&fn_code, v, fn_ctx, state, function_name);

                        RETURN_TYPE_INFERRING.with(|s| s.borrow_mut().remove(&fn_id));

                        let to_return = if fn_type.is_empty() {
                            // If function doesn't return anything, return nothing
                            DataType::Null
                        } else {
                            // If function returns anything, check if it returns the same thing each time
                            DataType::Poly(Box::from(fn_type)).check_poly()
                        };

                        v.truncate(v_len_before_args);

                        // Cache the result
                        state
                            .fns
                            .iter_mut()
                            .find(|f| f.name == function_name)
                            .unwrap()
                            .return_type_cache
                            .push((Box::from(infered_arg_types), to_return.clone()));

                        to_return
                    }
                }
            }
            Self::ObjFunctionCall(obj, _, namespace, _, _, _) => {
                match namespace.last().unwrap().as_str() {
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
                        let obj_type = obj.infer_type(v, ctx, state);
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
                        let obj_type = obj.infer_type(v, ctx, state);
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
                        let obj_type = obj.infer_type(v, ctx, state);
                        if let DataType::Array(array_type) = obj_type {
                            DataType::Array(Some(Box::from(DataType::Array(array_type))))
                        } else {
                            unsafe { unreachable_unchecked() }
                        }
                    }
                    "get" => {
                        let obj_type = obj.infer_type(v, ctx, state);
                        if let DataType::Map(m) = obj_type {
                            m.1.unwrap_or(DataType::Unknown)
                        } else {
                            unsafe { unreachable_unchecked() }
                        }
                    }
                    _ => unsafe { unreachable_unchecked() },
                }
            }
            Self::InlineCondition(_, code, _) => {
                let mut types: Vec<DataType> = Vec::with_capacity(code.len());
                types.push(code[0].infer_type(v, ctx, state));
                for t in &code[0..] {
                    if let Self::ElseIfBlock(_, code) = t {
                        let infered = code[0].infer_type(v, ctx, state);
                        if !types.contains(&infered) {
                            types.push(infered);
                        }
                    } else if let Self::ElseBlock(code) = t {
                        let infered = code[0].infer_type(v, ctx, state);
                        if !types.contains(&infered) {
                            types.push(infered);
                        }
                    }
                }
                DataType::Poly(Box::from(types)).check_poly()
            }
            Self::Struct(namespace, _, span) => {
                let name = &namespace[namespace.len() - 1];
                let namespace = &namespace[..(namespace.len() - 1)];
                DataType::Struct(
                    find_struct(state.namespace, namespace, name).unwrap_or_else(|| {
                        error_unknown_struct(name, *span, state.sources, ctx.src);
                    }) as u16,
                )
            }
            Self::AnonymousFunction(_, _, _) => {
                todo!("Anonymous functions are WIP")
                // let fn_name =
                //     format_args!("{}{}{}", ctx.current_src_file, span.start, span.end).to_smolstr();
                // if let Some(id) = state
                //     .fns
                //     .iter()
                //     .rposition(|f| f.name == fn_name && &f.args == args)
                // {
                //     return DataType::Fn(id as u16);
                // }
                // let returns_null = check_if_returns_void(code);
                // let mut callees = Vec::new();
                // collect_direct_fn_calls(code, &mut callees);
                // let id = state.fns.len() as u16;
                // state.fns.push(Function {
                //     name: fn_name,
                //     args: args.clone(),
                //     code: Rc::from(code.clone()),
                //     impls: Vec::new(),
                //     is_recursive: None,
                //     returns_null,
                //     src_file: ctx.current_src_file,
                //     return_type_cache: Vec::new(),
                //     direct_calls: callees.into_boxed_slice(),
                //     name_span: *span,
                // });
                // state.fn_registers.push(Vec::new());
                // // state.namespace.fns.push((x.clone(), id));
                // DataType::Fn(id)
            }
            _ => unsafe { unreachable_unchecked() },
        }
    }
}

impl DataType {
    pub fn check_poly(self) -> Self {
        if let Self::Poly(ref elems) = self {
            if let Some(new) = reduce_null_struct(elems) {
                return new;
            }
            let mut concrete = elems
                .iter()
                .filter(|elem_type| **elem_type != Self::Unknown);
            if let Some(first_type) = concrete.next() {
                if concrete.all(|x| x == first_type) {
                    first_type.clone()
                } else {
                    self
                }
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

#[cfg(not(target_arch = "wasm32"))]
pub fn datatype_to_c_type(x: &DataType) -> Type {
    match x {
        DataType::Int => libffi::middle::Type::i32(),
        DataType::Float => libffi::middle::Type::f64(),
        DataType::String | DataType::Array(_) => libffi::middle::Type::pointer(),
        DataType::Null => libffi::middle::Type::void(),
        _ => unsafe { unreachable_unchecked() },
    }
}
