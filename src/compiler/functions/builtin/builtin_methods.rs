use super::super::expr::Expr;
use super::super::expr::Span;
use super::super::type_system::DataType;
use crate::compiler::UnwrapId;
use crate::compiler::compiler_data::Ctx;
use crate::compiler::compiler_data::State;
use crate::compiler::compiler_data::Variable;
use crate::compiler::compiler_errors::check_args;
use crate::compiler::compiler_errors::check_args_range;
use crate::compiler::compiler_errors::error_unknown_function;
use crate::errors::ErrType;
use crate::errors::throw_compiler_error;
use crate::instr::Instr;
use crate::instr::LibFunc;
use crate::instr::LibFuncVoid;

pub fn builtin_methods(
    name: &str,
    id: u16,
    obj_type: DataType,
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    tgt_id: Option<u16>,
    obj: &Expr,
    args: &[Expr],
    obj_markers: Span,
    fn_span: Span,
    args_indexes: &[Span],
) -> Option<u16> {
    let src = ctx.src;

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
        ($expected:pat,$expected_str:expr) => {
            if !{
                if let DataType::Poly(polytype) = &obj_type {
                    polytype.iter().all(|x| matches!(x, $expected))
                } else {
                    matches!(obj_type, $expected)
                }
            } {
                throw_compiler_error(
                    src,
                    obj_markers,
                    ErrType::InvalidObjType($expected_str, &obj_type),
                );
            }
        };
    }

    macro_rules! check {
        ($expected:pat,$expected_str:expr, $args:expr) => {
            check_type!($expected, $expected_str);
            check_args(
                args,
                $args,
                name,
                src,
                if args_indexes.is_empty() {
                    fn_span
                } else {
                    Span {
                        start: args_indexes[0].start,
                        end: args_indexes.last().unwrap().end,
                    }
                },
                state.sources,
            )
        };
        ($expected:pat,$expected_str:expr, $args_min:expr,$args_max:expr) => {
            check_type!($expected, $expected_str);
            check_args_range!(
                args,
                $args_min,
                $args_max,
                name,
                src,
                if args_indexes.is_empty() {
                    fn_markers.start
                } else {
                    args_indexes[0].start
                },
                if args_indexes.is_empty() {
                    fn_markers.end
                } else {
                    args_indexes.last().unwrap().end
                }
            )
        };
    }
    match name {
        "uppercase" => {
            check!(DataType::String, "String", 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Uppercase, id, output_id));
            Some(output_id)
        }
        "lowercase" => {
            check!(DataType::String, "String", 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Lowercase, id, output_id));
            Some(output_id)
        }
        "starts_with" => {
            check!(DataType::String, "String", 1);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::StartsWith, id, output_id));
            Some(output_id)
        }
        "ends_with" => {
            check!(DataType::String, "String", 1);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::EndsWith, id, output_id));
            Some(output_id)
        }
        "replace" => {
            check!(DataType::String, "String", 2);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Replace, id, output_id));
            Some(output_id)
        }
        "len" => {
            check!(DataType::Array(_) | DataType::String, "Array or String", 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Len, id, output_id));
            Some(output_id)
        }
        "contains" => {
            check!(DataType::Array(_) | DataType::String, "Array or String", 1);

            let arg_type = args[0].infer_type(v, ctx, state);
            if obj_type == DataType::String {
                arg_type.expect(&DataType::String, src, args_indexes[0]);
            }

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Contains, id, output_id));
            Some(output_id)
        }
        "trim" => {
            check!(DataType::String, "String", 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Trim, id, output_id));
            Some(output_id)
        }
        "trim_sequence" => {
            check!(DataType::String, "String", 1);

            let arg_type = args[0].infer_type(v, ctx, state);
            arg_type.expect(&DataType::String, src, args_indexes[0]);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TrimSequence, id, output_id));
            Some(output_id)
        }
        "find" => {
            check!(DataType::String | DataType::Array(_), "Array or String", 1);

            let arg_type = args[0].infer_type(v, ctx, state);
            if let DataType::Array(Some(array_elem_type)) = &obj_type {
                arg_type.expect(array_elem_type, src, args_indexes[0]);
            } else if obj_type == DataType::String {
                arg_type.expect(&DataType::String, src, args_indexes[0]);
            }

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Find, id, output_id));
            state.add_to_src(ctx, output, fn_span);
            Some(output_id)
        }
        "is_float" => {
            check!(DataType::String, "String", 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::IsFloat, id, output_id));
            Some(output_id)
        }
        "is_int" => {
            check!(DataType::String, "String", 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::IsInt, id, output_id));
            Some(output_id)
        }
        "trim_left" => {
            check!(DataType::String, "String", 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TrimLeft, id, output_id));
            Some(output_id)
        }
        "trim_right" => {
            check!(DataType::String, "String", 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TrimRight, id, output_id));
            Some(output_id)
        }
        "trim_sequence_left" => {
            check!(DataType::String, "String", 1);

            let arg_type = args[0].infer_type(v, ctx, state);
            arg_type.expect(&DataType::String, src, args_indexes[0]);

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::TrimSequenceLeft, id, output_id));
            Some(output_id)
        }
        "trim_sequence_right" => {
            check!(DataType::String, "String", 1);

            let arg_type = args[0].infer_type(v, ctx, state);
            arg_type.expect(&DataType::String, src, args_indexes[0]);

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
            check!(DataType::String | DataType::Array(_), "Array or String", 1);

            let arg_type = args[0].infer_type(v, ctx, state);
            arg_type.expect(&DataType::Int, src, args_indexes[0]);

            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Repeat, id, output_id));
            Some(output_id)
        }
        "push" => {
            check!(DataType::Array(_), "Array", 1);

            let arg_type = args[0].infer_type(v, ctx, state);
            if let DataType::Array(Some(array_elem_type)) = &obj_type {
                arg_type.expect(array_elem_type, src, args_indexes[0]);
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
            check!(DataType::Float, "Float", 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::SqrtFloat, id, output_id));
            Some(output_id)
        }
        "round" => {
            check!(DataType::Float, "Float", 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Round, id, output_id));
            Some(output_id)
        }
        "floor" => {
            check!(DataType::Float, "Float", 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Floor, id, output_id));
            Some(output_id)
        }
        "abs" => {
            check!(DataType::Float | DataType::Int, "Int or Float", 0);
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Abs, id, output_id));
            Some(output_id)
        }
        "reverse" => {
            check!(DataType::Array(_) | DataType::String, "Array or String", 0);
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
            check!(DataType::String, "String", 1);
            let arg_type = args[0].infer_type(v, ctx, state);
            arg_type.expect(&obj_type, src, args_indexes[0]);
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Split, id, output_id));
            Some(output_id)
        }
        "partition" => {
            check!(DataType::Array(_), "Array", 1);

            let arg_type = args[0].infer_type(v, ctx, state);
            if let DataType::Array(Some(array_elem_type)) = obj_type {
                arg_type.expect(&array_elem_type, src, args_indexes[0]);
            }
            add_args!();
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::Split, id, output_id));
            Some(output_id)
        }
        "join" => {
            let expected = DataType::Array(Some(Box::from(DataType::String)));
            if !{
                if let DataType::Poly(polytype) = &obj_type {
                    polytype.iter().all(|x| x == &expected)
                } else {
                    obj_type == expected
                }
            } {
                throw_compiler_error(src, fn_span, ErrType::InvalidType(&expected, &obj_type));
            }
            check_args_range(args, 0, 1, "join", src, fn_span);
            if !args.is_empty() {
                let arg_type = args[0].infer_type(v, ctx, state);
                arg_type.expect(&DataType::String, src, args_indexes[0]);
                add_args!();
            }
            let output_id = state.alloc_reg_tgt(tgt_id);
            output.push(Instr::CallLibFunc(LibFunc::JoinStringArray, id, output_id));
            Some(output_id)
        }
        "remove" => {
            check!(DataType::Array(_), "Array", 1);

            let arg_type = args[0].infer_type(v, ctx, state);
            arg_type.expect(&DataType::Int, src, args_indexes[0]);
            let arg_id = args[0]
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(arg_id, v);
            output.push(Instr::Remove(id, arg_id));
            state.add_to_src(ctx, output, fn_span);
            None
        }
        "sort" => {
            check!(DataType::Array(_), "Array", 0);
            output.push(Instr::CallLibFuncVoid(LibFuncVoid::Sort, id, 0));
            None
        }
        "get" => {
            check!(DataType::Map(_), "Map", 1);

            let arg_type = args[0].infer_type(v, ctx, state);
            if let DataType::Map(t) = obj_type
                && let Some(key_type) = t.0
            {
                arg_type.expect(&key_type, src, args_indexes[0]);
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
            check!(DataType::Map(_), "Map", 2);
            let key_type = args[0].infer_type(v, ctx, state);
            let val_type = args[1].infer_type(v, ctx, state);
            if let DataType::Map(m) = obj_type {
                if let Some(t) = m.0 {
                    key_type.expect(&t, src, args_indexes[0]);
                }
                if let Some(t) = m.1 {
                    val_type.expect(&t, src, args_indexes[1]);
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
        fn_name => error_unknown_function(fn_name, fn_span, std::iter::empty(), src, state.sources),
    }
}
