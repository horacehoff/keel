use crate::Instr;
use crate::LibFunc;
use crate::check_args;
use crate::check_args_range;
use crate::errors::ErrType;
use crate::errors::throw_parser_error;
use crate::expr::Expr;
use crate::get_id;
use crate::instr::LibFuncVoid;
use crate::parser_data::Variable;
use crate::parser_data::{Ctx, State};
use crate::registers::alloc_register;
use crate::registers::free_register;
use crate::type_system::DataType;
use crate::type_system::infer_type;
use inline_colorization::*;
use smol_str::SmolStr;
use smol_str::ToSmolStr;

pub fn handle_method_calls(
    output: &mut Vec<Instr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    obj: &Expr,
    args: &[Expr],
    namespace: &[SmolStr],
    obj_markers: &(usize, usize),
    fn_markers: &(usize, usize),
    args_indexes: &[(usize, usize)],
    offset: u16,
    single_run: bool,
) {
    let src = ctx.src;
    let current_src_file = ctx.current_src_file;

    let len = namespace.len() - 1;
    let name = namespace[len].as_str();
    // not in use for now
    // let namespace = &namespace[0..len];

    let obj_type = infer_type(obj, v, state.fns, src, state.dyn_libs);
    let id = get_id(obj, v, ctx, state, output, None, false, offset, single_run);
    free_register(id, state.free_registers, v, state.const_registers);

    macro_rules! add_args {
        () => {
            for arg in args.iter().rev() {
                let arg_id = get_id(&arg, v, ctx, state, output, None, false, offset, single_run);
                output.push(Instr::StoreFuncArg(arg_id));
                *state.allocated_arg_count += 1;
                free_register(arg_id, state.free_registers, v, state.const_registers);
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
                throw_parser_error(
                    src,
                    obj_markers,
                    ErrType::Custom(
                    format_args!(
                        "Expected {color_bright_blue}{style_bold}{}{color_reset}{style_reset}, found {color_bright_red}{style_bold}{}{color_reset}{style_reset}",
                        $expected_str,
                        obj_type
                    ).to_smolstr())
                );
            }
        };
    }

    macro_rules! check {
        ($expected:pat,$expected_str:expr, $args:expr) => {
            check_type!($expected, $expected_str);
            check_args!(
                args,
                $args,
                name,
                src,
                &(args_indexes[0].0, args_indexes.last().unwrap().1)
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
                args_indexes[0].0,
                args_indexes.last().unwrap().1
            )
        };
    }

    match name {
        "uppercase" => {
            check!(DataType::String, "String", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::Uppercase,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "lowercase" => {
            check!(DataType::String, "String", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::Lowercase,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "starts_with" => {
            check!(DataType::String, "String", 1);
            add_args!();
            output.push(Instr::CallLibFunc(
                LibFunc::StartsWith,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "ends_with" => {
            check!(DataType::String, "String", 1);
            add_args!();
            output.push(Instr::CallLibFunc(
                LibFunc::EndsWith,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "replace" => {
            check!(DataType::String, "String", 2);
            add_args!();
            output.push(Instr::CallLibFunc(
                LibFunc::Replace,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "len" => {
            check!(DataType::Array(_) | DataType::String, "Array or String", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::Len,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "contains" => {
            check!(DataType::Array(_) | DataType::String, "Array or String", 1);

            let arg_type = infer_type(&args[0], v, state.fns, src, state.dyn_libs);
            if obj_type == DataType::String && arg_type != DataType::String {
                throw_parser_error(
                    src,
                    &args_indexes[0],
                    ErrType::InvalidType(DataType::String, &arg_type),
                );
            }

            add_args!();

            output.push(Instr::CallLibFunc(
                LibFunc::Contains,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "trim" => {
            check!(DataType::String, "String", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::Trim,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "trim_sequence" => {
            check!(DataType::String, "String", 1);

            let arg_type = infer_type(&args[0], v, state.fns, src, state.dyn_libs);
            if arg_type != DataType::String {
                throw_parser_error(
                    src,
                    &args_indexes[0],
                    ErrType::InvalidType(DataType::String, &arg_type),
                );
            }
            add_args!();

            output.push(Instr::CallLibFunc(
                LibFunc::TrimSequence,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "find" => {
            check!(DataType::String | DataType::Array(_), "Array or String", 1);

            let arg_type = infer_type(&args[0], v, state.fns, src, state.dyn_libs);
            if let DataType::Array(Some(array_elem_type)) = &obj_type
                && **array_elem_type != arg_type
            {
                throw_parser_error(
                    src,
                    &args_indexes[0],
                    ErrType::InvalidType(*array_elem_type.clone(), &arg_type),
                );
            } else if !matches!(obj_type, DataType::Array(None)) && arg_type != obj_type {
                throw_parser_error(
                    src,
                    &args_indexes[0],
                    ErrType::InvalidType(DataType::String, &arg_type),
                );
            }

            add_args!();

            output.push(Instr::CallLibFunc(
                LibFunc::Find,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
            state
                .instr_src
                .push((*output.last().unwrap(), *fn_markers, current_src_file))
        }
        "is_float" => {
            check!(DataType::String, "String", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::IsFloat,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "is_int" => {
            check!(DataType::String, "String", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::IsInt,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "trim_left" => {
            check!(DataType::String, "String", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::TrimLeft,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "trim_right" => {
            check!(DataType::String, "String", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::TrimRight,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "trim_sequence_left" => {
            check!(DataType::String, "String", 1);

            let arg_type = infer_type(&args[0], v, state.fns, src, state.dyn_libs);
            if arg_type != DataType::String {
                throw_parser_error(
                    src,
                    &args_indexes[0],
                    ErrType::InvalidType(DataType::String, &arg_type),
                );
            }

            add_args!();
            output.push(Instr::CallLibFunc(
                LibFunc::TrimSequenceLeft,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "trim_sequence_right" => {
            check!(DataType::String, "String", 1);

            let arg_type = infer_type(&args[0], v, state.fns, src, state.dyn_libs);
            if arg_type != DataType::String {
                throw_parser_error(
                    src,
                    &args_indexes[0],
                    ErrType::InvalidType(DataType::String, &arg_type),
                );
            }

            add_args!();
            output.push(Instr::CallLibFunc(
                LibFunc::TrimSequenceRight,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "repeat" => {
            check!(DataType::String | DataType::Array(_), "Array or String", 1);

            let arg_type = infer_type(&args[0], v, state.fns, src, state.dyn_libs);
            if arg_type != DataType::Int {
                throw_parser_error(
                    src,
                    &args_indexes[0],
                    ErrType::InvalidType(DataType::Int, &arg_type),
                );
            }

            add_args!();

            output.push(Instr::CallLibFunc(
                LibFunc::Repeat,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "push" => {
            check!(DataType::Array(_), "Array", 1);

            let arg_type = infer_type(&args[0], v, state.fns, src, state.dyn_libs);
            if let DataType::Array(Some(array_elem_type)) = &obj_type
                && **array_elem_type != arg_type
            {
                throw_parser_error(
                    src,
                    &args_indexes[0],
                    ErrType::InvalidType(*array_elem_type.clone(), &arg_type),
                );
            }

            // If the array was declared as empty, upgrade its type so downstream indexing resolves correctly
            if obj_type == DataType::Array(None)
                && let Expr::Var(var_name, _) = obj
                && let Some(var) = v.iter_mut().rfind(|var| &var.name == var_name)
            {
                var.infered_type = DataType::Array(Some(Box::new(arg_type.clone())));
            }

            let arg_id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            free_register(id, state.free_registers, v, state.const_registers);
            output.push(Instr::Push(id, arg_id));
        }
        "sqrt" => {
            check!(DataType::Float, "Float", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::SqrtFloat,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "round" => {
            check!(DataType::Float, "Float", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::Round,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "floor" => {
            check!(DataType::Float, "Float", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::Floor,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "abs" => {
            check!(DataType::Float | DataType::Int, "Int or Float", 0);
            output.push(Instr::CallLibFunc(
                LibFunc::Abs,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "reverse" => {
            check!(DataType::Array(_) | DataType::String, "Array or String", 0);
            if obj_type == DataType::String {
                output.push(Instr::CallLibFunc(
                    LibFunc::Reverse,
                    id,
                    alloc_register(state.registers, state.free_registers),
                ));
            } else {
                output.push(Instr::CallLibFuncVoid(LibFuncVoid::Reverse, id, 0));
            }
        }
        "split" => {
            check!(DataType::String, "String", 1);

            let arg_type = infer_type(&args[0], v, state.fns, src, state.dyn_libs);
            if obj_type != arg_type {
                throw_parser_error(
                    src,
                    &args_indexes[0],
                    ErrType::InvalidType(DataType::String, &arg_type),
                );
            }
            add_args!();
            output.push(Instr::CallLibFunc(
                LibFunc::Split,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "partition" => {
            check!(DataType::Array(_), "Array", 1);

            let arg_type = infer_type(&args[0], v, state.fns, src, state.dyn_libs);
            if let DataType::Array(Some(array_elem_type)) = obj_type
                && *array_elem_type != arg_type
            {
                throw_parser_error(
                    src,
                    &args_indexes[0],
                    ErrType::InvalidType(*array_elem_type, &arg_type),
                );
            }
            add_args!();
            output.push(Instr::CallLibFunc(
                LibFunc::Split,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
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
                throw_parser_error(src, fn_markers, ErrType::InvalidType(expected, &obj_type));
            }
            check_args_range!(args, 0, 1, "join", src, fn_markers);
            if !args.is_empty() {
                let arg_type = infer_type(&args[0], v, state.fns, src, state.dyn_libs);
                if arg_type != DataType::String {
                    throw_parser_error(
                        src,
                        &args_indexes[0],
                        ErrType::InvalidType(DataType::String, &arg_type),
                    );
                }
                add_args!();
            }
            output.push(Instr::CallLibFunc(
                LibFunc::JoinStringArray,
                id,
                alloc_register(state.registers, state.free_registers),
            ));
        }
        "remove" => {
            check!(DataType::Array(_), "Array", 1);

            let arg_type = infer_type(&args[0], v, state.fns, src, state.dyn_libs);
            if arg_type != DataType::Int {
                throw_parser_error(
                    src,
                    &args_indexes[0],
                    ErrType::InvalidType(DataType::Int, &arg_type),
                );
            }
            let arg_id = get_id(
                &args[0], v, ctx, state, output, None, false, offset, single_run,
            );
            free_register(arg_id, state.free_registers, v, state.const_registers);
            output.push(Instr::Remove(id, arg_id));
            state
                .instr_src
                .push((*output.last().unwrap(), *fn_markers, current_src_file));
        }
        "sort" => {
            check!(DataType::Array(_), "Array", 0);
            output.push(Instr::CallLibFuncVoid(LibFuncVoid::Sort, id, 0));
        }
        name => {
            throw_parser_error(src, fn_markers, ErrType::UnknownFunction(name));
        }
    }
}
