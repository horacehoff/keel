use crate::{
    ast::{Expr, FunctionCallExpr, IfBlockExpr, QualifiedName, VariableDeclarationExpr},
    compiler::{
        codegen::patterns::{declare_pattern_bindings, type_after_mismatch},
        state::{Ctx, State},
        types::DataType,
    },
};
use core::{cell::RefCell, hint::unreachable_unchecked};
use rustc_hash::FxHashSet;

// Tracks which user-defined functions are currently being analysed for their
// return type. Used to break mutual-recursion cycles in type inference
thread_local! {
    static RETURN_TYPE_INFERRING: RefCell<FxHashSet<usize>> =
        RefCell::new(FxHashSet::default());
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
            Expr::Match(match_expr) => {
                expr_stack.push(match_expr.obj);
                for arm in match_expr.arms {
                    if let Some(guard) = arm.guard {
                        expr_stack.push(guard);
                    }
                    expr_stack.extend(arm.code);
                }
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
            Expr::Match(match_expr) => {
                if match_expr.arms.iter().any(|arm| !check_if_returns_void(arm.code)) {
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

fn track_returns<'arena>(
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
pub(super) fn resolve_function_return_type(
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
    let hidden_symbols = state.enter_function_scope(fn_src_file, fn_id);
    let fn_type = track_returns(fn_code, fn_ctx, state, fn_name);
    state.exit_function_scope(fn_src_file, hidden_symbols);

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
            Expr::Match(match_expr) => {
                let mut candidate = match_expr.obj.infer_type(ctx, state);
                let mut arms_always_return = true;
                let mut has_irrefutable_arm = false;
                for arm in match_expr.arms {
                    let v_len = state.v.len();
                    let Some(is_irrefutable) =
                        declare_pattern_bindings(&arm.pattern, &candidate, ctx, state)
                    else {
                        continue;
                    };
                    let arm_flow = track_return_flow(arm.code, ctx, state, fn_name);
                    state.v.truncate(v_len);
                    extend_return_types!(&mut return_types, arm_flow.types);
                    arms_always_return &= arm_flow.always_returns;
                    if is_irrefutable && arm.guard.is_none() {
                        has_irrefutable_arm = true;
                        break;
                    }
                    candidate =
                        type_after_mismatch(&arm.pattern, arm.guard, &candidate, ctx, state);
                }
                if arms_always_return && has_irrefutable_arm {
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
                if let Some(var) = state.find_var_mut(name.get_name()) {
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
                        .rfind(|var| var.name == var_name.get_name())
                        .is_some_and(|var| var.var_type == DataType::Array(None))
                {
                    let arg_type = function_call.args[1].infer_type(ctx, state);
                    if let Some(var) = state.find_var_mut(var_name.get_name()) {
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
