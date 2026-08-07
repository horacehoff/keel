use super::super::expr::Expr;
use super::super::expr::Span;
use super::super::type_system::DataType;
use super::super::type_system::fn_args_match;
use super::super::type_system::fn_matches_signature;
use crate::compiler::Scope;
use crate::compiler::UnwrapId;
use crate::compiler::compiler_data::Ctx;
use crate::compiler::compiler_data::State;
use crate::compiler::compiler_errors::check_args;
use crate::compiler::compiler_errors::check_args_range;
use crate::compiler::compiler_errors::error_expected_function;
use crate::compiler::compiler_errors::error_function_arg_invalid_type;
use crate::compiler::compiler_errors::error_invalid_obj_type;
use crate::compiler::compiler_errors::error_unknown_function;
use crate::compiler::expr::FunctionCallExpr;
use crate::compiler::functions::check_arg_type;
use crate::compiler::functions::user_functions::compile_function_impl;
use crate::data::Data;
use crate::instr::Instr;
use crate::instr::LibFunc;
use crate::instr::LibFuncVoid;
use std::hint::unreachable_unchecked;

pub fn builtin_methods(
    receiver_id: u16,
    receiver_type: DataType,
    output: &mut Vec<Instr>,
    ctx: Ctx,
    state: &mut State<'_>,
    tgt_id: Option<u16>,
    function_call: &FunctionCallExpr,
) -> Option<u16> {
    let args = &function_call.args[1..];
    let name = function_call.qualified_name.get_name().as_str();
    let arg_spans = &function_call.arg_spans;
    let receiver_span = function_call.arg_spans[0];
    let span = function_call.span;
    let receiver = &function_call.args[0];
    macro_rules! add_args {
        () => {
            *state.allocated_arg_count = (*state.allocated_arg_count).max(args.len());
            for arg in args.iter().rev() {
                let arg_id = arg.compile(ctx, state, output, None, false, true).unwrap_id();
                output.push(Instr::StoreFuncArg(arg_id));
                state.free_reg(arg_id);
            }
        };
    }

    macro_rules! check_type {
        ($expected:pat,$expected_list:expr,$name:expr) => {
            if !{
                if let DataType::Union(polytype) = &receiver_type {
                    polytype.iter().all(|x| matches!(x, $expected))
                } else {
                    matches!(receiver_type, $expected)
                }
            } {
                error_invalid_obj_type(
                    $expected_list,
                    &receiver_type,
                    $name,
                    receiver_span,
                    state.sources,
                    ctx.file_idx,
                );
            }
        };
    }

    macro_rules! check {
        ($expected:pat,$expected_str:expr,$name:expr,$args:expr) => {
            check_type!($expected, $expected_str, $name);
            check_args(
                args,
                $args,
                name,
                if arg_spans.is_empty() {
                    span
                } else {
                    Span { start: arg_spans[0].start, end: arg_spans.last().unwrap().end }
                },
                state.sources,
                ctx.file_idx,
            )
        };
        ($expected:pat,$expected_str:expr,$name:expr, $args_min:expr,$args_max:expr) => {
            check_type!($expected, $expected_str, $name);
            check_args_range(
                args,
                $args_min,
                $args_max,
                name,
                args_indexes,
                ctx.file_idx,
                state.sources,
                fn_span,
            )
        };
    }
    match name {
        "uppercase" => {
            check!(DataType::String, &[DataType::String], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Uppercase, receiver_id, output_id));
            Some(output_id)
        }
        "lowercase" => {
            check!(DataType::String, &[DataType::String], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Lowercase, receiver_id, output_id));
            Some(output_id)
        }
        "starts_with" => {
            check!(DataType::String, &[DataType::String], name, 1);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::StartsWith, receiver_id, output_id));
            Some(output_id)
        }
        "ends_with" => {
            check!(DataType::String, &[DataType::String], name, 1);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::EndsWith, receiver_id, output_id));
            Some(output_id)
        }
        "replace" => {
            check!(DataType::String, &[DataType::String], name, 2);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Replace, receiver_id, output_id));
            Some(output_id)
        }
        "len" => {
            check!(
                DataType::Array(_) | DataType::String | DataType::Map(_),
                &[DataType::String, DataType::Array(None), DataType::Map(Box::from((None, None)))],
                name,
                0
            );
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Len, receiver_id, output_id));
            Some(output_id)
        }
        "contains" => {
            check!(
                DataType::Array(_) | DataType::String,
                &[DataType::String, DataType::Array(None)],
                name,
                1
            );

            if receiver_type == DataType::String {
                check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::String]);
            }

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Contains, receiver_id, output_id));
            Some(output_id)
        }
        "trim" => {
            check!(DataType::String, &[DataType::String], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Trim, receiver_id, output_id));
            Some(output_id)
        }
        "trim_sequence" => {
            check!(DataType::String, &[DataType::String], name, 1);

            check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::String]);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TrimSequence, receiver_id, output_id));
            Some(output_id)
        }
        "find" => {
            check!(
                DataType::String | DataType::Array(_),
                &[DataType::String, DataType::Array(None)],
                name,
                1
            );

            if let DataType::Array(Some(array_elem_type)) = &receiver_type {
                check_arg_type(
                    name,
                    ctx,
                    state,
                    args,
                    arg_spans,
                    0,
                    std::slice::from_ref(array_elem_type),
                );
            } else if receiver_type == DataType::String {
                check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::String]);
            }

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Find, receiver_id, output_id));
            state.add_to_src(ctx, output, span);
            Some(output_id)
        }
        "is_float" => {
            check!(DataType::String, &[DataType::String], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::IsFloat, receiver_id, output_id));
            Some(output_id)
        }
        "is_int" => {
            check!(DataType::String, &[DataType::String], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::IsInt, receiver_id, output_id));
            Some(output_id)
        }
        "trim_left" => {
            check!(DataType::String, &[DataType::String], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TrimLeft, receiver_id, output_id));
            Some(output_id)
        }
        "trim_right" => {
            check!(DataType::String, &[DataType::String], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TrimRight, receiver_id, output_id));
            Some(output_id)
        }
        "trim_sequence_left" => {
            check!(DataType::String, &[DataType::String], name, 1);

            check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::String]);

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TrimSequenceLeft, receiver_id, output_id));
            Some(output_id)
        }
        "trim_sequence_right" => {
            check!(DataType::String, &[DataType::String], name, 1);

            check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::String]);

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TrimSequenceRight, receiver_id, output_id));
            Some(output_id)
        }
        "repeat" => {
            check!(
                DataType::String | DataType::Array(_),
                &[DataType::String, DataType::Array(None)],
                name,
                1
            );

            check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::Int]);

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Repeat, receiver_id, output_id));
            Some(output_id)
        }
        "push" => {
            check!(DataType::Array(_), &[DataType::Array(None)], name, 1);

            let arg_type = args[0].infer_type(ctx, state);
            if let DataType::Array(Some(array_elem_type)) = &receiver_type {
                check_arg_type(
                    name,
                    ctx,
                    state,
                    args,
                    arg_spans,
                    0,
                    std::slice::from_ref(array_elem_type),
                );
            }

            // If the array was declared as empty, upgrade its type so downstream indexing resolves correctly
            if receiver_type == DataType::Array(None)
                && let Expr::Var(var_name, _) = receiver
                && let Some(var) = state.find_var_mut(var_name)
            {
                var.var_type = DataType::Array(Some(Box::new(arg_type)));
            }

            let arg_id = args[0].compile(ctx, state, output, None, false, true).unwrap_id();
            state.free_reg(receiver_id);
            output.push(Instr::Push(receiver_id, arg_id));
            None
        }
        "sqrt" => {
            check!(DataType::Float, &[DataType::Float], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::SqrtFloat, receiver_id, output_id));
            Some(output_id)
        }
        "round" => {
            check!(DataType::Float, &[DataType::Float], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Round, receiver_id, output_id));
            Some(output_id)
        }
        "floor" => {
            check!(DataType::Float, &[DataType::Float], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Floor, receiver_id, output_id));
            Some(output_id)
        }
        "abs" => {
            check!(DataType::Float | DataType::Int, &[DataType::Int, DataType::Float], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Abs, receiver_id, output_id));
            Some(output_id)
        }
        "reverse" => {
            check!(
                DataType::Array(_) | DataType::String,
                &[DataType::String, DataType::Array(None)],
                name,
                0
            );
            if receiver_type == DataType::String {
                let output_id = state.alloc_reg_tgt(tgt_id);
                output.push(Instr::CallLibFunc(LibFunc::Reverse, receiver_id, output_id));
                Some(output_id)
            } else {
                output.push(Instr::CallLibFuncVoid(LibFuncVoid::Reverse, receiver_id, 0));
                None
            }
        }
        "split" => {
            check!(DataType::String, &[DataType::String], name, 1);
            check_arg_type(name, ctx, state, args, arg_spans, 0, &[receiver_type]);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Split, receiver_id, output_id));
            Some(output_id)
        }
        "partition" => {
            check!(DataType::Array(_), &[DataType::Array(None)], name, 1);

            if let DataType::Array(Some(array_elem_type)) = receiver_type {
                check_arg_type(
                    name,
                    ctx,
                    state,
                    args,
                    arg_spans,
                    0,
                    std::slice::from_ref(&array_elem_type),
                );
            }
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Split, receiver_id, output_id));
            Some(output_id)
        }
        "join" => {
            let expected = DataType::Array(Some(Box::from(DataType::String)));
            if !{
                if let DataType::Union(polytype) = &receiver_type {
                    polytype.iter().all(|x| x == &expected)
                } else {
                    receiver_type == expected
                }
            } {
                error_invalid_obj_type(
                    &[expected],
                    &receiver_type,
                    name,
                    receiver_span,
                    state.sources,
                    ctx.file_idx,
                );
            }
            check_args_range(args, 0, 1, "join", arg_spans, ctx.file_idx, state.sources, span);
            if !args.is_empty() {
                check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::String]);
                add_args!();
            }
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::JoinStringArray, receiver_id, output_id));
            Some(output_id)
        }
        "remove" => {
            check!(DataType::Array(_), &[DataType::Array(None)], name, 1);
            check_arg_type(name, ctx, state, args, arg_spans, 0, &[DataType::Int]);
            let arg_id = args[0].compile(ctx, state, output, None, false, true).unwrap_id();
            state.free_reg(arg_id);
            output.push(Instr::Remove(receiver_id, arg_id));
            state.add_to_src(ctx, output, span);
            None
        }
        "sort" => {
            check!(DataType::Array(_), &[DataType::Array(None)], name, 0);
            output.push(Instr::CallLibFuncVoid(LibFuncVoid::Sort, receiver_id, 0));
            None
        }
        "get" => {
            check!(DataType::Map(_), &[DataType::Map(Box::from((None, None)))], name, 1);

            if let DataType::Map(t) = receiver_type
                && let Some(key_type) = t.0
            {
                check_arg_type(name, ctx, state, args, arg_spans, 0, &[key_type]);
            }
            let arg_id = args[0].compile(ctx, state, output, None, false, true).unwrap_id();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::MapGet(receiver_id, arg_id, output_id));
            state.add_to_src(ctx, output, arg_spans[0]);
            Some(output_id)
        }
        "insert" => {
            check!(DataType::Map(_), &[DataType::Map(Box::from((None, None)))], name, 2);
            if let DataType::Map(m) = receiver_type {
                if let Some(t) = m.0 {
                    check_arg_type(name, ctx, state, args, arg_spans, 0, &[t]);
                }
                if let Some(t) = m.1 {
                    check_arg_type(name, ctx, state, args, arg_spans, 1, &[t]);
                }
            }
            let key_id = args[0].compile(ctx, state, output, None, false, true).unwrap_id();
            let val_id = args[1].compile(ctx, state, output, None, false, true).unwrap_id();
            output.push(Instr::MapInsertReg(receiver_id, key_id, val_id));
            None
        }
        "map" => {
            check!(
                DataType::Array(_) | DataType::String,
                &[DataType::String, DataType::Array(None)],
                name,
                1
            );

            let fn_type = args[0].infer_type(ctx, state);
            let fn_id = if let DataType::Fn(id) = fn_type {
                id as usize
            } else {
                error_expected_function(&fn_type, arg_spans[0], ctx.file_idx, state.sources);
            };

            let is_str = receiver_type == DataType::String;

            let elem_type = if is_str {
                DataType::String
            } else {
                match &receiver_type {
                    DataType::Array(Some(t)) => (**t).clone(),
                    DataType::Array(None) => {
                        state.functions[fn_id].args[0].1.clone().unwrap_or(DataType::Unknown)
                    }
                    _ => unsafe { unreachable_unchecked() },
                }
            };

            if !{
                if is_str {
                    fn_matches_signature(fn_id, &[DataType::String, DataType::String], ctx, state)
                } else {
                    fn_args_match(fn_id, std::slice::from_ref(&elem_type), state)
                }
            } {
                let expected_type = DataType::FnSignature(Box::from([
                    elem_type.clone(),
                    if is_str { DataType::String } else { DataType::Unknown },
                ]));
                error_function_arg_invalid_type(
                    &fn_type,
                    &expected_type,
                    arg_spans[0],
                    name,
                    None,
                    ctx.file_idx,
                    state.sources,
                );
            }

            let fn_impl_idx =
                compile_function_impl(output, ctx, state, fn_id, std::slice::from_ref(&elem_type));
            let loc = state.functions[fn_id].impls[fn_impl_idx].loc;
            let arg_reg = state.functions[fn_id].impls[fn_impl_idx].args_loc[0];

            let fn_value_reg = state.new_reg(Data::function(loc));

            let result_id = state.alloc_reg_tgt(tgt_id);
            if is_str {
                let data = Data::p_str("", &mut state.pools.str_pool);
                let empty_str_id = state.new_reg(data);
                output.push(Instr::Mov(empty_str_id, result_id));
            } else {
                output.push(Instr::EmptyArray(result_id));
            }

            let len_id = state.alloc_reg();
            output.push(Instr::CallLibFunc(LibFunc::Len, receiver_id, len_id));

            let index_id = if ctx.single_run {
                state.new_reg(Data::int(0))
            } else {
                let r = state.alloc_reg();
                output.push(Instr::SetInt(r, 0));
                r
            };

            // Basically the same thing as compile_for_loop
            let jmp_start_idx = output.len();
            output.push(Instr::SupEqIntJmp(index_id, len_id, 0));

            let body_start = output.len();
            if is_str {
                output.push(Instr::GetIndexString(receiver_id, index_id, arg_reg));
            } else {
                output.push(Instr::GetIndexArray(receiver_id, index_id, arg_reg));
            }
            let fn_return_val_id = state.alloc_reg();
            output.push(Instr::CallFuncDynamic(fn_value_reg, fn_return_val_id));
            if is_str {
                output.push(Instr::AddStr(result_id, fn_return_val_id, result_id));
            } else {
                output.push(Instr::Push(result_id, fn_return_val_id));
            }
            let jump_size = (output.len() - body_start + 1) as u16;

            output.push(Instr::IncInt(index_id));
            output.push(Instr::InfIntJmpBack(index_id, len_id, jump_size));

            let exit_size = (output.len() - jmp_start_idx) as u16;
            output[jmp_start_idx] = Instr::SupEqIntJmp(index_id, len_id, exit_size);

            if ctx.single_run {
                state.free_reg(len_id);
                state.free_reg(index_id);
                state.free_reg(fn_return_val_id);
            }

            Some(result_id)
        }
        "filter" => {
            check!(
                DataType::Array(_) | DataType::String,
                &[DataType::String, DataType::Array(None)],
                name,
                1
            );

            let fn_type = args[0].infer_type(ctx, state);
            let fn_id = if let DataType::Fn(id) = fn_type {
                id as usize
            } else {
                error_expected_function(&fn_type, arg_spans[0], ctx.file_idx, state.sources);
            };

            let is_str = receiver_type == DataType::String;

            let elem_type = if is_str {
                DataType::String
            } else {
                match &receiver_type {
                    DataType::Array(Some(t)) => (**t).clone(),
                    DataType::Array(None) => {
                        state.functions[fn_id].args[0].1.clone().unwrap_or(DataType::Unknown)
                    }
                    _ => unsafe { unreachable_unchecked() },
                }
            };

            if !fn_matches_signature(fn_id, &[elem_type.clone(), DataType::Bool], ctx, state) {
                let expected_type =
                    DataType::FnSignature(Box::from([elem_type.clone(), DataType::Bool]));
                error_function_arg_invalid_type(
                    &fn_type,
                    &expected_type,
                    arg_spans[0],
                    name,
                    None,
                    ctx.file_idx,
                    state.sources,
                );
            }

            let fn_impl_idx =
                compile_function_impl(output, ctx, state, fn_id, std::slice::from_ref(&elem_type));
            let loc = state.functions[fn_id].impls[fn_impl_idx].loc;
            let arg_reg = state.functions[fn_id].impls[fn_impl_idx].args_loc[0];

            let fn_value_reg = state.new_reg(Data::function(loc));

            let result_id = state.alloc_reg_tgt(tgt_id);
            if is_str {
                let data = Data::p_str("", &mut state.pools.str_pool);
                let empty_str_id = state.new_reg(data);
                output.push(Instr::Mov(empty_str_id, result_id));
            } else {
                output.push(Instr::EmptyArray(result_id));
            }

            let len_id = state.alloc_reg();
            output.push(Instr::CallLibFunc(LibFunc::Len, receiver_id, len_id));

            let index_id = if ctx.single_run {
                state.new_reg(Data::int(0))
            } else {
                let r = state.alloc_reg();
                output.push(Instr::SetInt(r, 0));
                r
            };

            // Basically the same thing as compile_for_loop
            let jmp_start_idx = output.len();
            output.push(Instr::SupEqIntJmp(index_id, len_id, 0));

            let body_start = output.len();
            if is_str {
                output.push(Instr::GetIndexString(receiver_id, index_id, arg_reg));
            } else {
                output.push(Instr::GetIndexArray(receiver_id, index_id, arg_reg));
            }

            let fn_return_val_id = state.alloc_reg();
            output.push(Instr::CallFuncDynamic(fn_value_reg, fn_return_val_id));

            output.push(Instr::IsFalseJmp(fn_return_val_id, 2));
            if is_str {
                output.push(Instr::AddStr(result_id, arg_reg, result_id));
            } else {
                output.push(Instr::Push(result_id, arg_reg));
            }
            let jump_size = (output.len() - body_start + 1) as u16;

            output.push(Instr::IncInt(index_id));
            output.push(Instr::InfIntJmpBack(index_id, len_id, jump_size));

            let exit_size = (output.len() - jmp_start_idx) as u16;
            output[jmp_start_idx] = Instr::SupEqIntJmp(index_id, len_id, exit_size);

            if ctx.single_run {
                state.free_reg(len_id);
                state.free_reg(index_id);
                state.free_reg(fn_return_val_id);
            }

            Some(result_id)
        }
        fn_name => {
            error_unknown_function(fn_name, span, &Scope::default(), ctx.file_idx, state.sources)
        }
    }
}
