use crate::compiler::get_id;
use crate::compiler_data::Ctx;
use crate::compiler_data::State;
use crate::compiler_data::Variable;
use crate::errors::ErrType;
use crate::errors::throw_compiler_error;
use crate::expr::Expr;
use crate::expr::Span;
use crate::instr::Instr;
use crate::instr::LibFunc;
use crate::instr::LibFuncVoid;
use crate::type_system::DataType;
use crate::type_system::infer_type;
use crate::util::check_args;
use crate::util::check_args_range;

pub fn std_lib_methods(
    name: &str,
    id: u16,
    obj_type: DataType,
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    obj: &Expr,
    args: &[Expr],
    obj_markers: Span,
    fn_markers: Span,
    args_indexes: &[Span],
    offset: u16,
    single_run: bool,
) {
    let src = ctx.src;
    let current_src_file = ctx.current_src_file;

    macro_rules! add_args {
        () => {
            for arg in args.iter().rev() {
                let arg_id = get_id(&arg, v, ctx, state, output, None, false, offset, single_run);
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
                    fn_markers
                } else {
                    Span {
                        start: args_indexes[0].start,
                        end: args_indexes.last().unwrap().end,
                    }
                },
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
            output.push(Instr::CallLibFunc(
                LibFunc::Uppercase,
                id,
                state.alloc_reg(),
            ));
        }
        "lowercase" => {
            check!(DataType::String, "String", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::Lowercase,
                id,
                state.alloc_reg(),
            ));
        }
        "starts_with" => {
            check!(DataType::String, "String", 1);
            add_args!();
            output.push(Instr::CallLibFunc(
                LibFunc::StartsWith,
                id,
                state.alloc_reg(),
            ));
        }
        "ends_with" => {
            check!(DataType::String, "String", 1);
            add_args!();
            output.push(Instr::CallLibFunc(LibFunc::EndsWith, id, state.alloc_reg()));
        }
        "replace" => {
            check!(DataType::String, "String", 2);
            add_args!();
            output.push(Instr::CallLibFunc(LibFunc::Replace, id, state.alloc_reg()));
        }
        "len" => {
            check!(DataType::Array(_) | DataType::String, "Array or String", 0);
            output.push(Instr::CallLibFunc(LibFunc::Len, id, state.alloc_reg()));
        }
        "contains" => {
            check!(DataType::Array(_) | DataType::String, "Array or String", 1);

            let arg_type = infer_type(&args[0], v, ctx, state);
            if obj_type == DataType::String && arg_type != DataType::String {
                throw_compiler_error(
                    src,
                    args_indexes[0],
                    ErrType::InvalidType(&DataType::String, &arg_type),
                );
            }

            add_args!();

            output.push(Instr::CallLibFunc(LibFunc::Contains, id, state.alloc_reg()));
        }
        "trim" => {
            check!(DataType::String, "String", 0);
            output.push(Instr::CallLibFunc(LibFunc::Trim, id, state.alloc_reg()));
        }
        "trim_sequence" => {
            check!(DataType::String, "String", 1);

            let arg_type = infer_type(&args[0], v, ctx, state);
            if arg_type != DataType::String {
                throw_compiler_error(
                    src,
                    args_indexes[0],
                    ErrType::InvalidType(&DataType::String, &arg_type),
                );
            }
            add_args!();

            output.push(Instr::CallLibFunc(
                LibFunc::TrimSequence,
                id,
                state.alloc_reg(),
            ));
        }
        "find" => {
            check!(DataType::String | DataType::Array(_), "Array or String", 1);

            let arg_type = infer_type(&args[0], v, ctx, state);
            if let DataType::Array(Some(array_elem_type)) = &obj_type {
                if **array_elem_type != arg_type {
                    throw_compiler_error(
                        src,
                        args_indexes[0],
                        ErrType::InvalidType(array_elem_type, &arg_type),
                    );
                }
            } else if obj_type == DataType::String && arg_type != DataType::String {
                throw_compiler_error(
                    src,
                    args_indexes[0],
                    ErrType::InvalidType(&DataType::String, &arg_type),
                );
            }

            add_args!();

            output.push(Instr::CallLibFunc(LibFunc::Find, id, state.alloc_reg()));
            state
                .instr_src
                .push((*output.last().unwrap(), fn_markers, current_src_file));
        }
        "is_float" => {
            check!(DataType::String, "String", 0);
            output.push(Instr::CallLibFunc(LibFunc::IsFloat, id, state.alloc_reg()));
        }
        "is_int" => {
            check!(DataType::String, "String", 0);
            output.push(Instr::CallLibFunc(LibFunc::IsInt, id, state.alloc_reg()));
        }
        "trim_left" => {
            check!(DataType::String, "String", 0);
            output.push(Instr::CallLibFunc(LibFunc::TrimLeft, id, state.alloc_reg()));
        }
        "trim_right" => {
            check!(DataType::String, "String", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::TrimRight,
                id,
                state.alloc_reg(),
            ));
        }
        "trim_sequence_left" => {
            check!(DataType::String, "String", 1);

            let arg_type = infer_type(&args[0], v, ctx, state);
            if arg_type != DataType::String {
                throw_compiler_error(
                    src,
                    args_indexes[0],
                    ErrType::InvalidType(&DataType::String, &arg_type),
                );
            }

            add_args!();
            output.push(Instr::CallLibFunc(
                LibFunc::TrimSequenceLeft,
                id,
                state.alloc_reg(),
            ));
        }
        "trim_sequence_right" => {
            check!(DataType::String, "String", 1);

            let arg_type = infer_type(&args[0], v, ctx, state);
            if arg_type != DataType::String {
                throw_compiler_error(
                    src,
                    args_indexes[0],
                    ErrType::InvalidType(&DataType::String, &arg_type),
                );
            }

            add_args!();
            output.push(Instr::CallLibFunc(
                LibFunc::TrimSequenceRight,
                id,
                state.alloc_reg(),
            ));
        }
        "repeat" => {
            check!(DataType::String | DataType::Array(_), "Array or String", 1);

            let arg_type = infer_type(&args[0], v, ctx, state);
            if arg_type != DataType::Int {
                throw_compiler_error(
                    src,
                    args_indexes[0],
                    ErrType::InvalidType(&DataType::Int, &arg_type),
                );
            }

            add_args!();

            output.push(Instr::CallLibFunc(LibFunc::Repeat, id, state.alloc_reg()));
        }
        "push" => {
            check!(DataType::Array(_), "Array", 1);

            let arg_type = infer_type(&args[0], v, ctx, state);
            if let DataType::Array(Some(array_elem_type)) = &obj_type
                && **array_elem_type != arg_type
            {
                throw_compiler_error(
                    src,
                    args_indexes[0],
                    ErrType::InvalidType(array_elem_type, &arg_type),
                );
            }

            // If the array was declared as empty, upgrade its type so downstream indexing resolves correctly
            if obj_type == DataType::Array(None)
                && let Expr::Var(var_name, _) = obj
                && let Some(var) = v.iter_mut().rfind(|var| &var.name == var_name)
            {
                var.var_type = DataType::Array(Some(Box::new(arg_type)));
            }

            let arg_id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            state.free_reg(id, v);
            output.push(Instr::Push(id, arg_id));
        }
        "sqrt" => {
            check!(DataType::Float, "Float", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::SqrtFloat,
                id,
                state.alloc_reg(),
            ));
        }
        "round" => {
            check!(DataType::Float, "Float", 0);
            output.push(Instr::CallLibFunc(LibFunc::Round, id, state.alloc_reg()));
        }
        "floor" => {
            check!(DataType::Float, "Float", 0);
            output.push(Instr::CallLibFunc(LibFunc::Floor, id, state.alloc_reg()));
        }
        "abs" => {
            check!(DataType::Float | DataType::Int, "Int or Float", 0);
            output.push(Instr::CallLibFunc(LibFunc::Abs, id, state.alloc_reg()));
        }
        "reverse" => {
            check!(DataType::Array(_) | DataType::String, "Array or String", 0);
            if obj_type == DataType::String {
                output.push(Instr::CallLibFunc(LibFunc::Reverse, id, state.alloc_reg()));
            } else {
                output.push(Instr::CallLibFuncVoid(LibFuncVoid::Reverse, id, 0));
            }
        }
        "split" => {
            check!(DataType::String, "String", 1);

            let arg_type = infer_type(&args[0], v, ctx, state);
            if obj_type != arg_type {
                throw_compiler_error(
                    src,
                    args_indexes[0],
                    ErrType::InvalidType(&DataType::String, &arg_type),
                );
            }
            add_args!();
            output.push(Instr::CallLibFunc(LibFunc::Split, id, state.alloc_reg()));
        }
        "partition" => {
            check!(DataType::Array(_), "Array", 1);

            let arg_type = infer_type(&args[0], v, ctx, state);
            if let DataType::Array(Some(array_elem_type)) = obj_type
                && *array_elem_type != arg_type
            {
                throw_compiler_error(
                    src,
                    args_indexes[0],
                    ErrType::InvalidType(&array_elem_type, &arg_type),
                );
            }
            add_args!();
            output.push(Instr::CallLibFunc(LibFunc::Split, id, state.alloc_reg()));
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
                throw_compiler_error(src, fn_markers, ErrType::InvalidType(&expected, &obj_type));
            }
            check_args_range(args, 0, 1, "join", src, fn_markers);
            if !args.is_empty() {
                let arg_type = infer_type(&args[0], v, ctx, state);
                if arg_type != DataType::String {
                    throw_compiler_error(
                        src,
                        args_indexes[0],
                        ErrType::InvalidType(&DataType::String, &arg_type),
                    );
                }
                add_args!();
            }
            output.push(Instr::CallLibFunc(
                LibFunc::JoinStringArray,
                id,
                state.alloc_reg(),
            ));
        }
        "remove" => {
            check!(DataType::Array(_), "Array", 1);

            let arg_type = infer_type(&args[0], v, ctx, state);
            if arg_type != DataType::Int {
                throw_compiler_error(
                    src,
                    args_indexes[0],
                    ErrType::InvalidType(&DataType::Int, &arg_type),
                );
            }
            let arg_id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            state.free_reg(arg_id, v);
            output.push(Instr::Remove(id, arg_id));
            state
                .instr_src
                .push((*output.last().unwrap(), fn_markers, current_src_file));
        }
        "sort" => {
            check!(DataType::Array(_), "Array", 0);
            output.push(Instr::CallLibFuncVoid(LibFuncVoid::Sort, id, 0));
        }
        "get" => {
            check!(DataType::Map(_), "Map", 1);

            let arg_type = infer_type(&args[0], v, ctx, state);
            if let DataType::Map(t) = obj_type
                && let Some(key_type) = t.0
                && key_type != arg_type
            {
                throw_compiler_error(
                    src,
                    args_indexes[0],
                    ErrType::InvalidType(&key_type, &arg_type),
                );
            }
            let arg_id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            output.push(Instr::MapGet(id, arg_id, state.alloc_reg()));
            state
                .instr_src
                .push((*output.last().unwrap(), args_indexes[0], current_src_file));
        }
        "insert" => {
            check!(DataType::Map(_), "Map", 2);
            let key_type = infer_type(&args[0], v, ctx, state);
            let val_type = infer_type(&args[1], v, ctx, state);
            if let DataType::Map(m) = obj_type {
                if let Some(t) = m.0
                    && t != key_type
                {
                    throw_compiler_error(src, args_indexes[0], ErrType::InvalidType(&t, &key_type));
                }
                if let Some(t) = m.1
                    && t != val_type
                {
                    throw_compiler_error(src, args_indexes[1], ErrType::InvalidType(&t, &val_type));
                }
            }
            let key_id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            let val_id = get_id(
                &args[1], v, ctx, state, output, None, false, offset, single_run,
            );
            output.push(Instr::MapInsertReg(id, key_id, val_id));
        }
        name => {
            throw_compiler_error(src, fn_markers, ErrType::UnknownFunction(name));
        }
    }
}
