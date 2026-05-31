use crate::errors::ErrType;
use crate::errors::dev_error;
use crate::errors::throw_parser_error;
use crate::expr::Expr;
use crate::expr::symbol_of_expr;
use crate::parser_data::Dynamiclib;
use crate::parser_data::FnSignature;
use crate::parser_data::Function;
use crate::parser_data::Variable;
#[cfg(not(target_arch = "wasm32"))]
use libffi::middle::Type;
use smol_strc::SmolStr;
use std::cell::RefCell;
use std::collections::HashSet;

// Tracks which user-defined functions are currently being analysed for their
// return type. Used to break mutual-recursion cycles in type inference
thread_local! {
    static RETURN_TYPE_INFERRING: RefCell<HashSet<SmolStr>> =
        RefCell::new(HashSet::new());
}

#[derive(Debug, Clone)]
pub enum DataType {
    /// Array(None) = unknown element type (e.g. empty array literal [])
    Array(Option<Box<DataType>>),
    Float,
    Int,
    Bool,
    String,
    Null,
    /// Internal inference placeholder used while breaking recursive return-type cycles
    Unknown,
    Poly(Box<[DataType]>),
    /// Fn (\[arg_types ... return_type\]) => return_type is always specified
    Fn(Box<[DataType]>),
    Struct(u16),
}

impl PartialEq for DataType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // Array(None) is compatible with any array type
            (DataType::Array(None), DataType::Array(_)) => true,
            (DataType::Array(_), DataType::Array(None)) => true,
            (DataType::Array(Some(a)), DataType::Array(Some(b))) => a == b,
            (DataType::Float, DataType::Float) => true,
            (DataType::Int, DataType::Int) => true,
            (DataType::Bool, DataType::Bool) => true,
            (DataType::String, DataType::String) => true,
            (DataType::Null, DataType::Null) => true,
            (DataType::Unknown, DataType::Unknown) => true,
            (DataType::Poly(a), DataType::Poly(b)) => a == b,
            (DataType::Fn(a), DataType::Fn(b)) => a == b,
            (DataType::Struct(a), DataType::Struct(b)) => a == b,
            _ => false,
        }
    }
}

impl std::hash::Hash for DataType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // All Array variants hash identically, which is required because Array(None) == Array(Some(_))
        match self {
            DataType::Array(_) => 0u8.hash(state),
            DataType::Float => 1u8.hash(state),
            DataType::Int => 2u8.hash(state),
            DataType::Bool => 3u8.hash(state),
            DataType::String => 4u8.hash(state),
            DataType::Null => 6u8.hash(state),
            DataType::Unknown => 7u8.hash(state),
            DataType::Poly(p) => {
                8u8.hash(state);
                p.hash(state);
            }
            DataType::Fn(f) => {
                9u8.hash(state);
                f.hash(state);
            }
            DataType::Struct(s) => {
                10u8.hash(state);
                s.hash(state);
            }
        }
    }
}

pub fn is_type_indexable(x: &DataType) -> bool {
    matches!(x, DataType::String | DataType::Array(_) | DataType::Unknown)
}

/// Collect all the function calls in the given code
pub fn collect_direct_fn_calls(content: &[Expr], calls: &mut Vec<SmolStr>) {
    let mut expr_stack: Vec<&Expr> = content.iter().collect();
    while let Some(expression) = expr_stack.pop() {
        match expression {
            Expr::FunctionCall(_, namespace, _, _) => {
                calls.push(namespace.last().unwrap().clone());
            }
            Expr::Condition(x, y, _) | Expr::InlineCondition(x, y, _) => {
                expr_stack.push(x);
                expr_stack.extend(y.iter());
            }
            Expr::ElseIfBlock(x, y) => {
                expr_stack.push(x);
                expr_stack.extend(y.iter());
            }
            Expr::ElseBlock(x) | Expr::EvalBlock(x) | Expr::LoopBlock(x) => {
                expr_stack.extend(x.iter());
            }
            Expr::WhileBlock(x, y) => {
                expr_stack.push(x);
                expr_stack.extend(y.iter());
            }
            Expr::ObjFunctionCall(x, y, _, _, _, _) => {
                expr_stack.push(x);
                expr_stack.extend(y.iter());
            }
            Expr::ReturnVal(code) => {
                if let Some(code) = code.as_ref() {
                    expr_stack.push(code);
                }
            }
            Expr::FunctionDecl(_, x, _) => expr_stack.extend(x.iter()),
            Expr::ArrayGetIndex(x, y, _) => {
                expr_stack.push(x);
                expr_stack.push(y);
            }
            Expr::ArrayGetSlice(x, y, z, _) => {
                expr_stack.push(x);
                expr_stack.push(y);
                expr_stack.push(z);
            }
            Expr::VarDeclare(_, x) | Expr::VarAssign(_, x, _) => expr_stack.push(x),
            Expr::ForLoop(_, code, _) => expr_stack.extend(code.iter()),
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
            Expr::Mul(x, y, _)
            | Expr::Div(x, y, _)
            | Expr::Add(x, y, _)
            | Expr::Sub(x, y, _)
            | Expr::Mod(x, y, _)
            | Expr::Pow(x, y, _)
            | Expr::Eq(x, y)
            | Expr::NotEq(x, y)
            | Expr::Sup(x, y, _)
            | Expr::SupEq(x, y, _)
            | Expr::Inf(x, y, _)
            | Expr::InfEq(x, y, _)
            | Expr::BoolAnd(x, y, _)
            | Expr::BoolOr(x, y, _) => {
                expr_stack.push(x);
                expr_stack.push(y);
            }
            Expr::Neg(x, _) | Expr::BoolNeg(x, _) => expr_stack.push(x),
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
            | Expr::ForLoop(_, code, _)
            | Expr::EvalBlock(code)
            | Expr::LoopBlock(code)
            | Expr::IntForLoop(_, _, _, code, _, _) => {
                if !check_if_returns_void(code) {
                    return false;
                }
            }
            Expr::ReturnVal(return_val) => {
                if return_val.is_some() {
                    return false;
                }
            }
            _ => continue,
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
    fns: &mut [Function],
    src: (&str, &str),
    fn_name: &str,
    dyn_libs: &[Dynamiclib],
) -> Vec<DataType> {
    let mut flow = track_return_flow(content, v, fns, src, fn_name, dyn_libs);
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
    fns: &mut [Function],
    src: (&str, &str),
    fn_name: &str,
    dyn_libs: &[Dynamiclib],
) -> FnReturnFlow {
    let v_len = v.len();
    let flow = track_return_flow(code, v, fns, src, fn_name, dyn_libs);
    v.truncate(v_len);
    flow
}

fn track_condition_returns(
    code: &[Expr],
    v: &mut Vec<Variable>,
    fns: &mut [Function],
    src: (&str, &str),
    fn_name: &str,
    dyn_libs: &[Dynamiclib],
) -> FnReturnFlow {
    let mut return_types = Vec::new();
    let first_branch_end = code
        .iter()
        .position(|expr| matches!(expr, Expr::ElseIfBlock(_, _) | Expr::ElseBlock(_)))
        .unwrap_or(code.len());

    let first_flow =
        track_scoped_returns(&code[..first_branch_end], v, fns, src, fn_name, dyn_libs);
    let mut all_branches_return = first_flow.always_returns;
    let mut has_else = false;
    extend_return_types!(&mut return_types, first_flow.types);

    for expr in &code[first_branch_end..] {
        match expr {
            Expr::ElseIfBlock(_, branch_code) => {
                let flow = track_scoped_returns(branch_code, v, fns, src, fn_name, dyn_libs);
                all_branches_return &= flow.always_returns;
                extend_return_types!(&mut return_types, flow.types);
            }
            Expr::ElseBlock(branch_code) => {
                has_else = true;
                let flow = track_scoped_returns(branch_code, v, fns, src, fn_name, dyn_libs);
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
    fns: &mut [Function],
    src: (&str, &str),
    fn_name: &str,
    dyn_libs: &[Dynamiclib],
) -> FnReturnFlow {
    let mut return_types: Vec<DataType> = Vec::new();
    for expr in content {
        match expr {
            Expr::Condition(_, code, _) | Expr::InlineCondition(_, code, _) => {
                let flow = track_condition_returns(code, v, fns, src, fn_name, dyn_libs);
                extend_return_types!(&mut return_types, flow.types);
                if flow.always_returns {
                    return FnReturnFlow {
                        types: return_types,
                        always_returns: true,
                    };
                }
            }
            Expr::ElseIfBlock(_, code) | Expr::ElseBlock(code) => {
                let flow = track_scoped_returns(code, v, fns, src, fn_name, dyn_libs);
                extend_return_types!(&mut return_types, flow.types);
                if flow.always_returns {
                    return FnReturnFlow {
                        types: return_types,
                        always_returns: true,
                    };
                }
            }
            Expr::VarDeclare(name, expr) => {
                let var_type = infer_type(expr, v, fns, src, dyn_libs);
                v.push(Variable {
                    name: name.clone(),
                    register_id: 0,
                    var_type,
                });
            }
            Expr::VarAssign(name, expr, _) => {
                let var_type = infer_type(expr, v, fns, src, dyn_libs);
                if let Some(var) = v.iter_mut().rfind(|var| &var.name == name) {
                    var.var_type = var_type;
                }
            }
            Expr::EvalBlock(code) => {
                let flow = track_scoped_returns(code, v, fns, src, fn_name, dyn_libs);
                extend_return_types!(&mut return_types, flow.types);
                if flow.always_returns {
                    return FnReturnFlow {
                        types: return_types,
                        always_returns: true,
                    };
                }
            }
            Expr::WhileBlock(_, code) => {
                let flow = track_scoped_returns(code, v, fns, src, fn_name, dyn_libs);
                extend_return_types!(&mut return_types, flow.types);
            }
            Expr::LoopBlock(code) => {
                let flow = track_scoped_returns(code, v, fns, src, fn_name, dyn_libs);
                extend_return_types!(&mut return_types, flow.types);
                if flow.always_returns {
                    return FnReturnFlow {
                        types: return_types,
                        always_returns: true,
                    };
                }
            }
            Expr::IntForLoop(var_name, _, _, code, _, _) => {
                let v_len = v.len();
                v.push(Variable {
                    name: var_name.clone(),
                    register_id: 0,
                    var_type: DataType::Int,
                });
                let flow = track_return_flow(code, v, fns, src, fn_name, dyn_libs);
                extend_return_types!(&mut return_types, flow.types);
                v.truncate(v_len);
            }
            Expr::ForLoop(var_name, array_code, _) => {
                let array_expr = array_code.first().unwrap();
                let inferred_collection_type = infer_type(array_expr, v, fns, src, dyn_libs);
                let elem_type = match inferred_collection_type {
                    DataType::Array(inner) => inner.map_or(DataType::Unknown, |t| *t),
                    DataType::String => DataType::String,
                    DataType::Unknown => DataType::Unknown,
                    _ => unreachable!(),
                };
                let v_len = v.len();
                if var_name.as_str() != "_" {
                    v.push(Variable {
                        name: var_name.clone(),
                        register_id: 0,
                        var_type: elem_type,
                    });
                }
                let flow = track_return_flow(&array_code[1..], v, fns, src, fn_name, dyn_libs);
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
                    let arg_type = infer_type(&args[0], v, fns, src, dyn_libs);
                    if let Some(var) = v.iter_mut().rfind(|var| &var.name == var_name) {
                        var.var_type = DataType::Array(Some(Box::new(arg_type)));
                    }
                }
            }
            Expr::ReturnVal(return_val) => {
                if let Some(val) = return_val.as_ref() {
                    let infered = infer_type(val, v, fns, src, dyn_libs);
                    add_return_type!(&mut return_types, infered);
                } else {
                    add_return_type!(&mut return_types, DataType::Null);
                }
                return FnReturnFlow {
                    types: return_types,
                    always_returns: true,
                };
            }
            _ => continue,
        }
    }
    FnReturnFlow {
        types: return_types,
        always_returns: false,
    }
}

pub fn infer_type(
    e: &Expr,
    v: &mut Vec<Variable>,
    fns: &mut [Function],
    src: (&str, &str),
    dyn_libs: &[Dynamiclib],
) -> DataType {
    match e {
        Expr::Var(name, markers) => v
            .iter()
            .rfind(|x| &x.name == name)
            .unwrap_or_else(|| {
                throw_parser_error(src, markers, ErrType::UnknownVariable(name));
            })
            .var_type
            .clone(),
        Expr::Float(_) => DataType::Float,
        Expr::Int(_) => DataType::Int,
        Expr::String(_) => DataType::String,
        Expr::Bool(_) => DataType::Bool,
        Expr::Null => DataType::Null,
        Expr::Array(x, _) => DataType::Array(if x.is_empty() {
            None
        } else {
            let elem_type = x
                .iter()
                .map(|elem| infer_type(elem, v, fns, src, dyn_libs))
                .find(|elem_type| *elem_type != DataType::Unknown)
                .unwrap_or(DataType::Unknown);
            Some(Box::from(elem_type))
        }),
        Expr::Add(x, y, markers) => {
            match (
                infer_type(x, v, fns, src, dyn_libs),
                infer_type(y, v, fns, src, dyn_libs),
            ) {
                (DataType::Unknown, t) | (t, DataType::Unknown) => t,
                (DataType::Float, DataType::Float) => DataType::Float,
                (DataType::Int, DataType::Int) => DataType::Int,
                (DataType::String, DataType::String) => DataType::String,
                (DataType::Array(t1), DataType::Array(t2)) => DataType::Array(t1.or(t2)),
                (l, r) => throw_parser_error(src, markers, ErrType::OpError(&l, &r, "+")),
            }
        }
        Expr::Mul(x, y, markers)
        | Expr::Div(x, y, markers)
        | Expr::Sub(x, y, markers)
        | Expr::Mod(x, y, markers)
        | Expr::Pow(x, y, markers) => {
            match (
                infer_type(x, v, fns, src, dyn_libs),
                infer_type(y, v, fns, src, dyn_libs),
            ) {
                (DataType::Unknown, t) | (t, DataType::Unknown)
                    if matches!(t, DataType::Float | DataType::Int | DataType::Unknown) =>
                {
                    t
                }
                (DataType::Float, DataType::Float) => DataType::Float,
                (DataType::Int, DataType::Int) => DataType::Int,
                (l, r) => {
                    throw_parser_error(src, markers, ErrType::OpError(&l, &r, symbol_of_expr(e)))
                }
            }
        }
        Expr::Eq(_, _) => DataType::Bool,
        Expr::NotEq(_, _) => DataType::Bool,
        Expr::Sup(x, y, markers)
        | Expr::SupEq(x, y, markers)
        | Expr::Inf(x, y, markers)
        | Expr::InfEq(x, y, markers) => {
            match (
                infer_type(x, v, fns, src, dyn_libs),
                infer_type(y, v, fns, src, dyn_libs),
            ) {
                (DataType::Unknown, DataType::Float | DataType::Int | DataType::Unknown)
                | (DataType::Float | DataType::Int, DataType::Unknown) => DataType::Bool,
                (DataType::Float, DataType::Float) => DataType::Bool,
                (DataType::Int, DataType::Int) => DataType::Bool,
                (l, r) => {
                    throw_parser_error(src, markers, ErrType::OpError(&l, &r, symbol_of_expr(e)))
                }
            }
        }
        Expr::BoolAnd(x, y, markers) | Expr::BoolOr(x, y, markers) => {
            match (
                infer_type(x, v, fns, src, dyn_libs),
                infer_type(y, v, fns, src, dyn_libs),
            ) {
                (DataType::Unknown, DataType::Bool | DataType::Unknown)
                | (DataType::Bool, DataType::Unknown) => DataType::Bool,
                (DataType::Bool, DataType::Bool) => DataType::Bool,
                (l, r) => throw_parser_error(src, markers, ErrType::OpError(&l, &r, "||")),
            }
        }
        Expr::Neg(e, _) => match infer_type(e, v, fns, src, dyn_libs) {
            DataType::Float => DataType::Float,
            DataType::Int => DataType::Int,
            DataType::Unknown => DataType::Unknown,
            _ => unreachable!(),
        },
        Expr::BoolNeg(e, _) => match infer_type(e, v, fns, src, dyn_libs) {
            DataType::Bool => DataType::Bool,
            _ => unreachable!(),
        },
        Expr::ArrayGetIndex(array, _, _) => match infer_type(array, v, fns, src, dyn_libs) {
            DataType::Array(array_type) => array_type.map_or(DataType::Null, |t| *t),
            DataType::String => DataType::String,
            DataType::Unknown => DataType::Unknown,
            _ => unreachable!(),
        },
        Expr::ArrayGetSlice(array, _, _, _) => match infer_type(array, v, fns, src, dyn_libs) {
            DataType::Array(array_type) => DataType::Array(array_type),
            DataType::String => DataType::String,
            DataType::Unknown => DataType::Unknown,
            _ => unreachable!(),
        },
        Expr::FunctionCall(args, namespace, markers, _) => {
            match namespace.last().unwrap().as_str() {
                "print" => DataType::Null,
                "type" => DataType::String,
                "float" => DataType::Float,
                "int" => DataType::Int,
                "str" => DataType::String,
                "bool" => DataType::Bool,
                "input" => DataType::String,
                "range" => DataType::Array(Some(Box::from(DataType::Int))),
                "the_answer" => DataType::Int,
                "argv" => DataType::Array(Some(Box::from(DataType::String))),
                // File System
                "read" => DataType::String,
                "exists" => DataType::Bool,
                "write" => DataType::Null,
                "append" => DataType::Null,
                "delete" => DataType::Null,
                "delete_dir" => DataType::Null,
                function_name => {
                    if let Some(lib) = dyn_libs.iter().find(|l| l.name == namespace[0])
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
                        .map(|x| infer_type(x, v, fns, src, dyn_libs))
                        .collect::<Vec<DataType>>();

                    let func = fns
                        .iter()
                        .find(|func| func.name == function_name)
                        .unwrap_or_else(|| {
                            throw_parser_error(
                                src,
                                markers,
                                ErrType::UnknownFunction(function_name),
                            );
                        });

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
                            name: fn_args[i].clone(),
                            register_id: 0,
                            var_type: infered_type,
                        });
                    }

                    // Mutual-recursion cycle guard -> if we are already in the
                    // middle of inferring this function's return type, return Null to break the cycle
                    let already_inferring =
                        RETURN_TYPE_INFERRING.with(|s| s.borrow().contains(function_name));
                    if already_inferring {
                        v.truncate(v_len_before_args);
                        return DataType::Unknown;
                    }

                    RETURN_TYPE_INFERRING
                        .with(|s| s.borrow_mut().insert(SmolStr::from(function_name)));

                    let fn_type = track_returns(&fn_code, v, fns, src, function_name, dyn_libs);

                    RETURN_TYPE_INFERRING.with(|s| s.borrow_mut().remove(function_name));

                    let to_return = if !fn_type.is_empty() {
                        // If function returns anything, check if it returns the same thing each time
                        check_poly(DataType::Poly(Box::from(fn_type)))
                    } else {
                        // If function doesn't return anything, return nothing
                        DataType::Null
                    };

                    v.truncate(v_len_before_args);

                    // Cache the result
                    fns.iter_mut()
                        .find(|f| f.name == function_name)
                        .unwrap()
                        .return_type_cache
                        .push((Box::from(infered_arg_types), to_return.clone()));

                    to_return
                }
            }
        }
        Expr::ObjFunctionCall(obj, _, namespace, _, _, _) => {
            match namespace.last().unwrap().as_str() {
                "uppercase" => DataType::String,
                "lowercase" => DataType::String,
                "starts_with" => DataType::Bool,
                "ends_with" => DataType::Bool,
                "replace" => DataType::String,
                "len" => DataType::Int,
                "contains" => DataType::Bool,
                "trim" => DataType::String,
                "trim_sequence" => DataType::String,
                "find" => DataType::Int,
                "is_float" => DataType::Bool,
                "is_int" => DataType::Bool,
                "trim_left" => DataType::String,
                "trim_right" => DataType::String,
                "trim_sequence_left" => DataType::String,
                "trim_sequence_right" => DataType::String,
                "repeat" => {
                    let obj_type = infer_type(obj, v, fns, src, dyn_libs);
                    if obj_type == DataType::String {
                        DataType::String
                    } else if let DataType::Array(array_type) = obj_type {
                        DataType::Array(array_type)
                    } else {
                        unreachable!()
                    }
                }
                "push" => DataType::Null,
                "sort" => DataType::Null,
                "sqrt" => DataType::Float,
                "round" => DataType::Float,
                "floor" => DataType::Float,
                "abs" => {
                    let obj_type = infer_type(obj, v, fns, src, dyn_libs);
                    if obj_type == DataType::Float {
                        DataType::Float
                    } else if obj_type == DataType::Int {
                        DataType::Int
                    } else {
                        unreachable!()
                    }
                }
                "reverse" => {
                    let obj_type = infer_type(obj, v, fns, src, dyn_libs);
                    if obj_type == DataType::String {
                        DataType::String
                    } else if let DataType::Array(array_type) = obj_type {
                        DataType::Array(array_type)
                    } else {
                        unreachable!()
                    }
                }
                "split" => DataType::Array(Some(Box::from(DataType::String))),
                "partition" => {
                    let obj_type = infer_type(obj, v, fns, src, dyn_libs);
                    if let DataType::Array(array_type) = obj_type {
                        DataType::Array(Some(Box::from(DataType::Array(array_type))))
                    } else {
                        unreachable!()
                    }
                }
                "join" => DataType::String,
                "remove" => DataType::Null,
                _ => unreachable!(),
            }
        }
        Expr::Condition(_, _, _) => DataType::Null,
        Expr::InlineCondition(_, code, _) => {
            let mut types: Vec<DataType> = Vec::with_capacity(code.len());
            types.push(infer_type(&code[0], v, fns, src, dyn_libs));
            for t in &code[0..] {
                if let Expr::ElseIfBlock(_, code) = t {
                    let infered = infer_type(&code[0], v, fns, src, dyn_libs);
                    if !types.contains(&infered) {
                        types.push(infered);
                    }
                } else if let Expr::ElseBlock(code) = t {
                    let infered = infer_type(&code[0], v, fns, src, dyn_libs);
                    if !types.contains(&infered) {
                        types.push(infered);
                    }
                }
            }
            check_poly(DataType::Poly(Box::from(types)))
        }
        _ => unreachable!(),
    }
}

pub fn check_poly(data: DataType) -> DataType {
    if let DataType::Poly(ref elems) = data {
        let mut concrete = elems
            .iter()
            .filter(|elem_type| **elem_type != DataType::Unknown);
        if let Some(first_type) = concrete.next() {
            if concrete.all(|x| x == first_type) {
                first_type.clone()
            } else {
                data
            }
        } else if !elems.is_empty() {
            DataType::Unknown
        } else {
            dev_error(
                "type_inference.rs",
                "check_poly",
                format_args!("DataType::Poly is empty"),
            )
        }
    } else {
        dev_error(
            "type_inference.rs",
            "check_poly",
            format_args!("Received data : {data} and not data : DataType::Poly"),
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn datatype_to_c_type(x: &DataType) -> Type {
    match x {
        DataType::Int => libffi::middle::Type::i32(),
        DataType::Float => libffi::middle::Type::f64(),
        DataType::String | DataType::Array(_) => libffi::middle::Type::pointer(),
        DataType::Null => libffi::middle::Type::void(),
        _ => unreachable!(),
    }
}
