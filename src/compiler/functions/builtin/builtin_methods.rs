use super::super::expr::Expr;
use super::super::expr::Span;
use super::super::type_system::DataType;
use super::super::type_system::fn_args_match;
use super::super::type_system::fn_matches_signature;
use crate::compiler::Namespace;
use crate::compiler::UnwrapId;
use crate::compiler::compiler_data::Ctx;
use crate::compiler::compiler_data::State;
use crate::compiler::compiler_data::Variable;
use crate::compiler::compiler_errors::check_args;
use crate::compiler::compiler_errors::check_args_range;
use crate::compiler::compiler_errors::error_expected_function;
use crate::compiler::compiler_errors::error_function_arg_invalid_type;
use crate::compiler::compiler_errors::error_invalid_obj_type;
use crate::compiler::compiler_errors::error_unknown_function;
use crate::compiler::functions::check_arg_type;
use crate::compiler::functions::user_functions::compile_function_impl;
use crate::data::Data;
use crate::instr::Instr;
use crate::instr::LibFunc;
use crate::instr::LibFuncVoid;
use std::hint::unreachable_unchecked;

pub fn builtin_methods(
    name: &str,
    id: u16,
    obj_type: DataType,
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx,
    state: &mut State<'_>,
    tgt_id: Option<u16>,
    obj: &Expr,
    args: &[Expr],
    obj_span: Span,
    fn_span: Span,
    args_indexes: &[Span],
) -> Option<u16> {
    macro_rules! add_args {
        () => {
            for arg in args.iter().rev() {
                let arg_id = arg
                    .compile(v, ctx, state, output, None, false, true)
                    .unwrap_id();
                output.push(Instr::StoreFuncArg(arg_id));
                *state.allocated_arg_count += 1;
                state.free_reg(arg_id, v);
            }
        };
    }

    macro_rules! check_type {
        ($expected:pat,$expected_list:expr,$name:expr) => {
            if !{
                if let DataType::Union(polytype) = &obj_type {
                    polytype.iter().all(|x| matches!(x, $expected))
                } else {
                    matches!(obj_type, $expected)
                }
            } {
                error_invalid_obj_type(
                    $expected_list,
                    &obj_type,
                    $name,
                    obj_span,
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
                if args_indexes.is_empty() {
                    fn_span
                } else {
                    Span {
                        start: args_indexes[0].start,
                        end: args_indexes.last().unwrap().end,
                    }
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
            output.push(Instr::CallLibFunc(LibFunc::Uppercase, id, output_id));
            Some(output_id)
        }
        "lowercase" => {
            check!(DataType::String, &[DataType::String], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Lowercase, id, output_id));
            Some(output_id)
        }
        "starts_with" => {
            check!(DataType::String, &[DataType::String], name, 1);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::StartsWith, id, output_id));
            Some(output_id)
        }
        "ends_with" => {
            check!(DataType::String, &[DataType::String], name, 1);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::EndsWith, id, output_id));
            Some(output_id)
        }
        "replace" => {
            check!(DataType::String, &[DataType::String], name, 2);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Replace, id, output_id));
            Some(output_id)
        }
        "len" => {
            check!(
                DataType::Array(_) | DataType::String,
                &[DataType::String, DataType::Array(None)],
                name,
                0
            );
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Len, id, output_id));
            Some(output_id)
        }
        "contains" => {
            check!(
                DataType::Array(_) | DataType::String,
                &[DataType::String, DataType::Array(None)],
                name,
                1
            );

            if obj_type == DataType::String {
                check_arg_type(
                    name,
                    v,
                    ctx,
                    state,
                    args,
                    args_indexes,
                    0,
                    &[DataType::String],
                );
            }

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Contains, id, output_id));
            Some(output_id)
        }
        "trim" => {
            check!(DataType::String, &[DataType::String], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Trim, id, output_id));
            Some(output_id)
        }
        "trim_sequence" => {
            check!(DataType::String, &[DataType::String], name, 1);

            check_arg_type(
                name,
                v,
                ctx,
                state,
                args,
                args_indexes,
                0,
                &[DataType::String],
            );
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TrimSequence, id, output_id));
            Some(output_id)
        }
        "find" => {
            check!(
                DataType::String | DataType::Array(_),
                &[DataType::String, DataType::Array(None)],
                name,
                1
            );

            if let DataType::Array(Some(array_elem_type)) = &obj_type {
                check_arg_type(
                    name,
                    v,
                    ctx,
                    state,
                    args,
                    args_indexes,
                    0,
                    std::slice::from_ref(array_elem_type),
                );
            } else if obj_type == DataType::String {
                check_arg_type(
                    name,
                    v,
                    ctx,
                    state,
                    args,
                    args_indexes,
                    0,
                    &[DataType::String],
                );
            }

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Find, id, output_id));
            state.add_to_src(ctx, output, fn_span);
            Some(output_id)
        }
        "is_float" => {
            check!(DataType::String, &[DataType::String], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::IsFloat, id, output_id));
            Some(output_id)
        }
        "is_int" => {
            check!(DataType::String, &[DataType::String], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::IsInt, id, output_id));
            Some(output_id)
        }
        "trim_left" => {
            check!(DataType::String, &[DataType::String], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TrimLeft, id, output_id));
            Some(output_id)
        }
        "trim_right" => {
            check!(DataType::String, &[DataType::String], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TrimRight, id, output_id));
            Some(output_id)
        }
        "trim_sequence_left" => {
            check!(DataType::String, &[DataType::String], name, 1);

            check_arg_type(
                name,
                v,
                ctx,
                state,
                args,
                args_indexes,
                0,
                &[DataType::String],
            );

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TrimSequenceLeft, id, output_id));
            Some(output_id)
        }
        "trim_sequence_right" => {
            check!(DataType::String, &[DataType::String], name, 1);

            check_arg_type(
                name,
                v,
                ctx,
                state,
                args,
                args_indexes,
                0,
                &[DataType::String],
            );

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(
                LibFunc::TrimSequenceRight,
                id,
                output_id,
            ));
            Some(output_id)
        }
        "repeat" => {
            check!(
                DataType::String | DataType::Array(_),
                &[DataType::String, DataType::Array(None)],
                name,
                1
            );

            check_arg_type(name, v, ctx, state, args, args_indexes, 0, &[DataType::Int]);

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Repeat, id, output_id));
            Some(output_id)
        }
        "push" => {
            check!(DataType::Array(_), &[DataType::Array(None)], name, 1);

            let arg_type = args[0].infer_type(v, ctx, state);
            if let DataType::Array(Some(array_elem_type)) = &obj_type {
                check_arg_type(
                    name,
                    v,
                    ctx,
                    state,
                    args,
                    args_indexes,
                    0,
                    std::slice::from_ref(array_elem_type),
                );
            }

            // If the array was declared as empty, upgrade its type so downstream indexing resolves correctly
            if obj_type == DataType::Array(None)
                && let Expr::Var(var_name, _) = obj
                && let Some(var) = v.iter_mut().rfind(|var| &var.name == var_name)
            {
                var.var_type = DataType::Array(Some(Box::new(arg_type)));
            }

            let arg_id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(id, v);
            output.push(Instr::Push(id, arg_id));
            None
        }
        "sqrt" => {
            check!(DataType::Float, &[DataType::Float], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::SqrtFloat, id, output_id));
            Some(output_id)
        }
        "round" => {
            check!(DataType::Float, &[DataType::Float], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Round, id, output_id));
            Some(output_id)
        }
        "floor" => {
            check!(DataType::Float, &[DataType::Float], name, 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Floor, id, output_id));
            Some(output_id)
        }
        "abs" => {
            check!(
                DataType::Float | DataType::Int,
                &[DataType::Int, DataType::Float],
                name,
                0
            );
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Abs, id, output_id));
            Some(output_id)
        }
        "reverse" => {
            check!(
                DataType::Array(_) | DataType::String,
                &[DataType::String, DataType::Array(None)],
                name,
                0
            );
            if obj_type == DataType::String {
                let output_id = state.alloc_reg_tgt(tgt_id);
                output.push(Instr::CallLibFunc(LibFunc::Reverse, id, output_id));
                Some(output_id)
            } else {
                output.push(Instr::CallLibFuncVoid(LibFuncVoid::Reverse, id, 0));
                None
            }
        }
        "split" => {
            check!(DataType::String, &[DataType::String], name, 1);
            check_arg_type(name, v, ctx, state, args, args_indexes, 0, &[obj_type]);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Split, id, output_id));
            Some(output_id)
        }
        "partition" => {
            check!(DataType::Array(_), &[DataType::Array(None)], name, 1);

            if let DataType::Array(Some(array_elem_type)) = obj_type {
                check_arg_type(
                    name,
                    v,
                    ctx,
                    state,
                    args,
                    args_indexes,
                    0,
                    std::slice::from_ref(&array_elem_type),
                );
            }
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Split, id, output_id));
            Some(output_id)
        }
        "join" => {
            let expected = DataType::Array(Some(Box::from(DataType::String)));
            if !{
                if let DataType::Union(polytype) = &obj_type {
                    polytype.iter().all(|x| x == &expected)
                } else {
                    obj_type == expected
                }
            } {
                error_invalid_obj_type(
                    &[expected],
                    &obj_type,
                    name,
                    obj_span,
                    state.sources,
                    ctx.file_idx,
                );
            }
            check_args_range(
                args,
                0,
                1,
                "join",
                args_indexes,
                ctx.file_idx,
                state.sources,
                fn_span,
            );
            if !args.is_empty() {
                check_arg_type(
                    name,
                    v,
                    ctx,
                    state,
                    args,
                    args_indexes,
                    0,
                    &[DataType::String],
                );
                add_args!();
            }
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::JoinStringArray, id, output_id));
            Some(output_id)
        }
        "remove" => {
            check!(DataType::Array(_), &[DataType::Array(None)], name, 1);
            check_arg_type(name, v, ctx, state, args, args_indexes, 0, &[DataType::Int]);
            let arg_id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(arg_id, v);
            output.push(Instr::Remove(id, arg_id));
            state.add_to_src(ctx, output, fn_span);
            None
        }
        "sort" => {
            check!(DataType::Array(_), &[DataType::Array(None)], name, 0);
            output.push(Instr::CallLibFuncVoid(LibFuncVoid::Sort, id, 0));
            None
        }
        "get" => {
            check!(
                DataType::Map(_),
                &[DataType::Map(Box::from((None, None)))],
                name,
                1
            );

            if let DataType::Map(t) = obj_type
                && let Some(key_type) = t.0
            {
                check_arg_type(name, v, ctx, state, args, args_indexes, 0, &[key_type]);
            }
            let arg_id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::MapGet(id, arg_id, output_id));
            state.add_to_src(ctx, output, args_indexes[0]);
            Some(output_id)
        }
        "insert" => {
            check!(
                DataType::Map(_),
                &[DataType::Map(Box::from((None, None)))],
                name,
                2
            );
            if let DataType::Map(m) = obj_type {
                if let Some(t) = m.0 {
                    check_arg_type(name, v, ctx, state, args, args_indexes, 0, &[t]);
                }
                if let Some(t) = m.1 {
                    check_arg_type(name, v, ctx, state, args, args_indexes, 1, &[t]);
                }
            }
            let key_id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            let val_id = args[1]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            output.push(Instr::MapInsertReg(id, key_id, val_id));
            None
        }
        "map" => {
            check!(
                DataType::Array(_) | DataType::String,
                &[DataType::String, DataType::Array(None)],
                name,
                1
            );

            let fn_type = args[0].infer_type(v, ctx, state);
            let fn_id = if let DataType::Fn(id) = fn_type {
                id as usize
            } else {
                error_expected_function(&fn_type, args_indexes[0], ctx.file_idx, state.sources);
            };

            let is_str = obj_type == DataType::String;

            let elem_type = if is_str {
                DataType::String
            } else {
                match &obj_type {
                    DataType::Array(Some(t)) => (**t).clone(),
                    DataType::Array(None) => state.fns[fn_id].args[0]
                        .1
                        .clone()
                        .unwrap_or(DataType::Unknown),
                    _ => unsafe { unreachable_unchecked() },
                }
            };

            if !{
                if is_str {
                    fn_matches_signature(
                        fn_id,
                        &[DataType::String, DataType::String],
                        v,
                        ctx,
                        state,
                    )
                } else {
                    fn_args_match(fn_id, std::slice::from_ref(&elem_type), state)
                }
            } {
                let expected_type = DataType::FnSignature(Box::from([
                    elem_type.clone(),
                    if is_str {
                        DataType::String
                    } else {
                        DataType::Unknown
                    },
                ]));
                error_function_arg_invalid_type(
                    &fn_type,
                    &expected_type,
                    args_indexes[0],
                    name,
                    None,
                    ctx.file_idx,
                    state.sources,
                );
            }

            let fn_impl_idx = compile_function_impl(
                output,
                v,
                ctx,
                state,
                fn_id,
                std::slice::from_ref(&elem_type),
            );
            let loc = state.fns[fn_id].impls[fn_impl_idx].loc;
            let arg_reg = state.fns[fn_id].impls[fn_impl_idx].args_loc[0];

            let fn_value_reg = state.registers.len() as u16;
            state.registers.push(Data::function(loc));

            let result_id = state.alloc_reg_tgt(tgt_id);
            if is_str {
                let empty_str_id = state.registers.len() as u16;
                state
                    .registers
                    .push(Data::p_str("", &mut state.pools.strings));
                output.push(Instr::Mov(empty_str_id, result_id));
            } else {
                output.push(Instr::EmptyArray(result_id));
            }

            let len_id = state.alloc_reg();
            output.push(Instr::CallLibFunc(LibFunc::Len, id, len_id));

            let index_id = if ctx.single_run {
                state.registers.push(0.into());
                (state.registers.len() - 1) as u16
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
                output.push(Instr::GetIndexString(id, index_id, arg_reg));
            } else {
                output.push(Instr::GetIndexArray(id, index_id, arg_reg));
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
                state.free_reg(len_id, v);
                state.free_reg(index_id, v);
                state.free_reg(fn_return_val_id, v);
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

            let fn_type = args[0].infer_type(v, ctx, state);
            let fn_id = if let DataType::Fn(id) = fn_type {
                id as usize
            } else {
                error_expected_function(&fn_type, args_indexes[0], ctx.file_idx, state.sources);
            };

            let is_str = obj_type == DataType::String;

            let elem_type = if is_str {
                DataType::String
            } else {
                match &obj_type {
                    DataType::Array(Some(t)) => (**t).clone(),
                    DataType::Array(None) => state.fns[fn_id].args[0]
                        .1
                        .clone()
                        .unwrap_or(DataType::Unknown),
                    _ => unsafe { unreachable_unchecked() },
                }
            };

            if !fn_matches_signature(fn_id, &[elem_type.clone(), DataType::Bool], v, ctx, state) {
                let expected_type =
                    DataType::FnSignature(Box::from([elem_type.clone(), DataType::Bool]));
                error_function_arg_invalid_type(
                    &fn_type,
                    &expected_type,
                    args_indexes[0],
                    name,
                    None,
                    ctx.file_idx,
                    state.sources,
                );
            }

            let fn_impl_idx = compile_function_impl(
                output,
                v,
                ctx,
                state,
                fn_id,
                std::slice::from_ref(&elem_type),
            );
            let loc = state.fns[fn_id].impls[fn_impl_idx].loc;
            let arg_reg = state.fns[fn_id].impls[fn_impl_idx].args_loc[0];

            let fn_value_reg = state.registers.len() as u16;
            state.registers.push(Data::function(loc));

            let result_id = state.alloc_reg_tgt(tgt_id);
            if is_str {
                let empty_str_id = state.registers.len() as u16;
                state
                    .registers
                    .push(Data::p_str("", &mut state.pools.strings));
                output.push(Instr::Mov(empty_str_id, result_id));
            } else {
                output.push(Instr::EmptyArray(result_id));
            }

            let len_id = state.alloc_reg();
            output.push(Instr::CallLibFunc(LibFunc::Len, id, len_id));

            let index_id = if ctx.single_run {
                state.registers.push(0.into());
                (state.registers.len() - 1) as u16
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
                output.push(Instr::GetIndexString(id, index_id, arg_reg));
            } else {
                output.push(Instr::GetIndexArray(id, index_id, arg_reg));
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
                state.free_reg(len_id, v);
                state.free_reg(index_id, v);
                state.free_reg(fn_return_val_id, v);
            }

            Some(result_id)
        }
        fn_name => error_unknown_function(
            fn_name,
            fn_span,
            &Namespace::default(),
            ctx.file_idx,
            state.sources,
        ),
    }
}
