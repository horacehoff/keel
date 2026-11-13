use crate::ast::Expr;
use crate::ast::symbol_of_expr;
use crate::compiler::Ctx;
use crate::compiler::DataType;
use crate::compiler::Span;
use crate::compiler::State;
use crate::compiler::compiler_errors::error_expected_function;
use crate::compiler::compiler_errors::error_op;
use crate::compiler::compiler_errors::error_struct_unknown_field;
use crate::compiler::compiler_errors::error_type_not_indexable;
use crate::compiler::compiler_errors::error_unknown_function;
use crate::compiler::compiler_errors::error_unknown_function_in_namespace;
use crate::compiler::compiler_errors::error_unknown_struct;
use crate::compiler::error_invalid_type;
use crate::compiler::program::FnSignature;
use crate::compiler::program::Function;
use crate::compiler::scope::Scope;
use crate::compiler::types::check_if_returns_void;
use crate::compiler::types::collect_direct_fn_calls;
use crate::compiler::types::error_function_needs_args_typed;
use crate::compiler::types::error_unknown_variable;
use crate::compiler::types::resolve_function_return_type;
use core::hint::unreachable_unchecked;

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

impl<'arena> Expr<'arena> {
    pub fn infer_type(&'arena self, ctx: Ctx, state: &mut State<'arena, '_>) -> DataType {
        match self {
            Self::Var(name, span) => {
                if let Some(var) = state.find_var(name.get_name()) {
                    var.var_type.clone()
                } else {
                    infer_symbol_type(name.get_namespace(), name.get_name(), *span, ctx, state)
                }
            }
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
                                    }
                                    error_unknown_function_in_namespace(
                                        function_name,
                                        state.scope(ctx.file_idx),
                                        qualified_name.get_namespace(),
                                        function_call.get_call_span(),
                                        ctx.file_idx,
                                        state.sources,
                                    );
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
                    "remove" if matches!(obj.infer_type(ctx, state), DataType::Map(_)) => {
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
                    fn_name => error_unknown_function(
                        fn_name,
                        function_call.spans[0],
                        &Scope::default(),
                        ctx.file_idx,
                        state.sources,
                    ),
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
                let fn_name = bumpalo::format!(in state.bump,"anonymous@f{}s{}e{}", ctx.file_idx, span.start, span.end);
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
                    returns_null,
                    src_file_idx: ctx.file_idx,
                    return_type_cache: Vec::new(),
                    direct_calls: state.bump.alloc_slice_copy(&callees),
                    name_span: *span,
                });
                DataType::Fn(id)
            }
            _ => unsafe { unreachable_unchecked() },
        }
    }
}
