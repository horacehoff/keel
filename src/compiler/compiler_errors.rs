use super::DataType;
use super::Expr;
use super::Function;
use super::Namespace;
use super::Source;
use super::Span;
use super::State;
use super::Variable;
use crate::errors::BLUE;
use crate::errors::GREEN;
use crate::errors::RESET;
use crate::errors::blue;
use crate::errors::bold;
use crate::errors::green;
use crate::errors::red;
use crate::errors::throw_compiler_error;
use ariadne::Label;
use ariadne::Report;
use smol_strc::SmolStr;
use smol_strc::ToSmolStr;

#[inline(never)]
#[cold]
pub fn error_array_diff_types(
    file_idx: u16,
    sources: &[Source],
    array_span: Span,
    array_elem_type: &DataType,
    failing_elem_span: Span,
    failing_elem_type: &DataType,
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), array_span.into()),
            )
            .with_message("Invalid array types")
            .with_label(
                Label::new((src.filename.as_str(), array_span.into()))
                    .with_message(format_args!(
                        "This expression is of type {}",
                        blue(array_elem_type)
                    ))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename.as_str(), failing_elem_span.into()))
                    .with_message(format_args!(
                        "This expression is of type {}",
                        red(failing_elem_type),
                    ))
                    .with_color(ariadne::Color::Red),
            )
            .with_note("Arrays are homogeneous and can only hold elements of a single type")
            .finish()
        },
        sources,
        file_idx,
        array_span,
        &format!("Invalid array types: this array holds elements of type {array_elem_type} but an element of type {failing_elem_type} was found"),
    );
}

#[inline(never)]
#[cold]
pub fn error_invalid_type(
    expected_type: &DataType,
    perceived_type: &DataType,
    span: Span,
    help: Option<std::fmt::Arguments>,
    note: Option<std::fmt::Arguments>,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message("Invalid type")
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!(
                        "Expected {}, but this expression's type is {}",
                        blue(expected_type),
                        red(perceived_type)
                    ))
                    .with_color(ariadne::Color::Red),
            );

            if let Some(note_msg) = note {
                report = report.with_note(note_msg);
            }
            if let Some(help_msg) = help {
                report = report.with_help(help_msg);
            }

            report.finish()
        },
        sources,
        file_idx,
        span,
        &format!("Invalid type: expected {expected_type}, but this expression's type is {perceived_type}"),
    );
}

#[inline(never)]
#[cold]
pub fn error_invalid_index_type(t: &DataType, span: Span, file_idx: u16, sources: &[Source]) {
    error_invalid_type(
        &DataType::Int,
        t,
        span,
        Some(format_args!("Try using the {} function", blue("int()"))),
        Some(format_args!(
            "The {} type is the only valid index type",
            blue(DataType::Int)
        )),
        file_idx,
        sources,
    );
}

#[inline(never)]
#[cold]
pub fn error_division_by_zero(modulo: bool, span: Span, file_idx: u16, sources: &[Source]) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message(format_args!(
                "{} by zero",
                if modulo { "Modulo" } else { "Division" }
            ))
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!(
                        "This performs {} by zero!",
                        if modulo { "modulo" } else { "division" }
                    ))
                    .with_color(ariadne::Color::Red),
            )
            .finish()
        },
        sources,
        file_idx,
        span,
        &format!("{} by zero", if modulo { "Modulo" } else { "Division" }),
    );
}

#[inline(never)]
#[cold]
pub fn error_cannot_push_type_to_array(
    array_type: &DataType,
    elem_type: &DataType,
    array_span: Span,
    span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message(format_args!(
                "Cannot insert {} in {}",
                red(elem_type),
                red(array_type)
            ))
            .with_label(
                Label::new((src.filename.as_str(), array_span.into()))
                    .with_message(format_args!("This array's type is {}", blue(array_type)))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!(
                        "But this expression's type is {}",
                        red(elem_type)
                    ))
                    .with_color(ariadne::Color::Red),
            )
            .finish()
        },
        sources,
        file_idx,
        span,
        &format!("Cannot insert {elem_type} in {array_type}"),
    );
}

#[inline(never)]
#[cold]
pub fn error_type_not_indexable(
    t: &DataType,
    span: Span,
    iterator_error: bool,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let msg = if iterator_error {
                "iterated on"
            } else {
                "indexed"
            };
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message("Invalid type")
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!(
                        "This expression's type is {}. This type cannot be {msg}.",
                        red(t),
                    ))
                    .with_color(ariadne::Color::Red),
            )
            .with_note(format_args!(
                "The following types can be {msg}:\n- {}\n- {}",
                blue(DataType::Array(None)),
                blue(DataType::String)
            ))
            .finish()
        },
        sources,
        file_idx,
        span,
        &format!("This expression's type is {t}, which cannot be {}", if iterator_error { "iterated on" } else { "indexed" }),
    );
}

#[cold]
#[inline(never)]
pub fn error_conditional_expression_without_else(
    span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message("Invalid inline conditional expression")
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!("Inline if blocks must have an else branch."))
                    .with_color(ariadne::Color::Red),
            )
            .finish()
        },
        sources,
        file_idx,
        span,
        "Inline if blocks must have an else branch",
    );
}

#[inline(never)]
#[cold]
pub fn error_cannot_read_file(span: Span, file_idx: u16, sources: &[Source]) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message("Cannot read file")
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!("This file cannot be found."))
                    .with_color(ariadne::Color::Red),
            )
            .finish()
        },
        sources,
        file_idx,
        span,
        "Cannot read file: this file cannot be found",
    );
}

#[inline(never)]
#[cold]
pub fn error_cannot_load_dynlib(span: Span, file_idx: u16, sources: &[Source]) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message("Cannot load dynamic library")
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!("This dynamic library cannot be found/loaded."))
                    .with_color(ariadne::Color::Red),
            )
            .finish()
        },
        sources,
        file_idx,
        span,
        "Cannot load dynamic library: this dynamic library cannot be found/loaded",
    );
}

#[inline(never)]
#[cold]
pub fn error_cannot_find_dynlib_symbol(
    symbol: &str,
    symbol_span: Span,
    dynlib_span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), dynlib_span.into()),
            )
            .with_message("Cannot find symbol in dynamic library")
            .with_label(
                Label::new((src.filename.as_str(), dynlib_span.into()))
                    .with_message(format_args!("This dynamic library is loaded here."))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename.as_str(), symbol_span.into()))
                    .with_message(format_args!(
                        "Cannot find symbol {} in this dynamic library",
                        red(symbol)
                    ))
                    .with_color(ariadne::Color::Red),
            )
            .finish()
        },
        sources,
        file_idx,
        symbol_span,
        &format!("Cannot find symbol {symbol} in this dynamic library"),
    );
}

#[inline(never)]
#[cold]
pub fn error_map_diff_types(
    file_idx: u16,
    sources: &[Source],
    map_span: Span,
    map_elem_type: &DataType,
    failing_elem_span: Span,
    failing_elem_type: &DataType,
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), map_span.into()),
            )
            .with_message("Invalid map types")
            .with_label(
                Label::new((src.filename.as_str(), map_span.into()))
                    .with_message(format_args!(
                        "This expression is of type {}",
                        blue(map_elem_type)
                    ))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename.as_str(), failing_elem_span.into()))
                    .with_message(format_args!(
                        "This expression is of type {}",
                        red(failing_elem_type),
                    ))
                    .with_color(ariadne::Color::Red),
            )
            .with_note("Maps are homogeneous and can only hold one key-value type")
            .finish()
        },
        sources,
        file_idx,
        map_span,
        &format!("Invalid map types: this map holds entries of type {map_elem_type} but an expression of type {failing_elem_type} was found"),
    );
}

#[inline(never)]
#[cold]
pub fn error_unknown_struct(
    struct_name: &SmolStr,
    struct_span: Span,
    sources: &[Source],
    file_idx: u16,
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), struct_span.into()),
            )
            .with_message("Unknown struct")
            .with_label(
                Label::new((src.filename.as_str(), struct_span.into()))
                    .with_message(format_args!("Unknown struct {}", red(struct_name)))
                    .with_color(ariadne::Color::Red),
            )
            .finish()
        },
        sources,
        file_idx,
        struct_span,
        &format!("Unknown struct {struct_name}"),
    );
}

pub fn error_struct_no_such_field(
    file_idx: u16,
    struct_name: &SmolStr,
    struct_span: Span,
    struct_field_span: Span,
    struct_field_name: &SmolStr,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), struct_field_span.into()),
            )
            .with_message("Unknown struct field")
            .with_label(
                Label::new((src.filename.as_str(), struct_span.into()))
                    .with_message(format_args!("Struct defined here"))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename.as_str(), struct_field_span.into()))
                    .with_message(format_args!(
                        "There is no field {} in {}",
                        red(struct_field_name),
                        blue(struct_name)
                    ))
                    .with_color(ariadne::Color::Red),
            );

            report.finish()
        },
        sources,
        file_idx,
        struct_field_span,
        &format!("There is no field {struct_field_name} in {struct_name}"),
    );
}

pub fn error_struct_missing_fields(
    file_idx: u16,
    struct_span: Span,
    struct_literal_span: Span,
    sources: &[Source],
    missing_fields: &[&SmolStr],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), struct_literal_span.into()),
            )
            .with_message("Missing struct fields")
            .with_label(
                Label::new((src.filename.as_str(), struct_span.into()))
                    .with_message(format_args!("Struct defined here"))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename.as_str(), struct_literal_span.into()))
                    .with_message(format_args!(
                        "This is missing field{} {}",
                        if missing_fields.len() > 1 { "s" } else { "" },
                        missing_fields
                            .iter()
                            .map(|f| blue(f).to_smolstr())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .with_color(ariadne::Color::Red),
            );

            report.finish()
        },
        sources,
        file_idx,
        struct_literal_span,
        &format!("Missing struct field{}: {}", if missing_fields.len() > 1 { "s" } else { "" }, missing_fields.iter().map(|f| f.as_str()).collect::<Vec<_>>().join(", ")),
    );
}

pub fn check_args(
    args: &[Expr],
    expected_args_len: usize,
    fn_name: &str,
    span: Span,
    sources: &[Source],
    file_idx: u16,
) {
    if args.len() != expected_args_len {
        throw_compiler_error(
            &|| {
                let src = &sources[file_idx as usize];
                let report = Report::build(
                    ariadne::ReportKind::Error,
                    (src.filename.as_str(), span.into()),
                )
                .with_message("Invalid argument count")
                .with_label(
                    Label::new((src.filename.as_str(), span.into()))
                        .with_message(format_args!(
                            "Function {} expects {} arguments but {} arguments were supplied",
                            blue(fn_name),
                            bold(expected_args_len),
                            bold(args.len())
                        ))
                        .with_color(ariadne::Color::Red),
                );

                report.finish()
            },
            sources,
            file_idx,
            span,
            &format!("Function {fn_name} expects {expected_args_len} arguments but {} arguments were supplied", args.len()),
        );
    }
}

#[cold]
#[inline(never)]
pub fn error_invalid_obj_type(
    expected_type: &[DataType],
    perceived_type: &DataType,
    fn_name: &str,
    span: Span,
    sources: &[Source],
    file_idx: u16,
) {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message("Invalid type")
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!(
                        "Function {} expects this expression's type to be {BLUE}{}{RESET} but here its type is {}",
                        blue(fn_name),
                        expected_type.iter().map(|s| s.to_smolstr()).collect::<Vec<SmolStr>>().join("{RESET} or {BLUE}"),
                        red(perceived_type)
                    ))
                    .with_color(ariadne::Color::Red),
            );

            report.finish()
        },
        sources,
        file_idx,
        span,
        &format!("Function {fn_name} expects this expression's type to be {} but here its type is {perceived_type}", expected_type.iter().map(|s| s.to_smolstr()).collect::<Vec<SmolStr>>().join(" or ")),
    );
}

pub fn check_args_user_fn(
    args: &[Expr],
    expected_args_len: usize,
    fn_name: &str,
    file_idx: u16,
    span: Span,
    fn_decl_span: (Span, u16),
    state: &State,
    args_indexes: &[Span],
) {
    let args_len = args.len();
    if args_len != expected_args_len {
        throw_compiler_error(
            &|| {
                let src = &state.sources[file_idx as usize];
                let fn_src = &state.sources[fn_decl_span.1 as usize];
                let mut report = Report::build(
                    ariadne::ReportKind::Error,
                    (src.filename.as_str(), span.into()),
                )
                .with_message("Invalid argument count")
                .with_label(
                    Label::new((fn_src.filename.as_str(), fn_decl_span.0.into()))
                        .with_message(format_args!(
                            "The function {} is defined here",
                            blue(fn_name)
                        ))
                        .with_color(ariadne::Color::Blue),
                )
                .with_label(
                    Label::new((src.filename.as_str(), span.into()))
                        .with_message(format_args!(
                            "Function {} expects {} arguments but {} arguments were supplied",
                            blue(fn_name),
                            bold(expected_args_len),
                            bold(args.len())
                        ))
                        .with_color(ariadne::Color::Red),
                );

                if args_len > expected_args_len {
                    let span: Span = (
                        args_indexes[expected_args_len].start,
                        args_indexes[args_indexes.len() - 1].end,
                    )
                        .into();
                    report = report.with_label(
                        Label::new((src.filename.as_str(), span.into()))
                            .with_message(format_args!(
                                "{}: Remove {}",
                                blue("Help"),
                                if args_len - expected_args_len == 1 {
                                    "this argument"
                                } else {
                                    "those arguments"
                                }
                            ))
                            .with_color(ariadne::Color::Blue)
                            .with_priority(-1),
                    );
                }

                report.finish()
            },
            state.sources,
            file_idx,
            span,
            &format!("Function {fn_name} expects {expected_args_len} arguments but {args_len} arguments were supplied"),
        );
    }
}

pub fn check_args_range(
    args: &[Expr],
    min_args_len: usize,
    max_args_len: usize,
    fn_name: &str,
    args_indexes: &[Span],
    file_idx: u16,
    sources: &[Source],
    span: Span,
) {
    if args.len() < min_args_len || args.len() > max_args_len {
        throw_compiler_error(
            &|| {
                let src = &sources[file_idx as usize];
                let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()))
            .with_message("Invalid argument count")
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!(
                        "Function {} expects at least {} and at most {} arguments but {} were supplied",
                        blue(fn_name),
                        blue(min_args_len),
                        blue(max_args_len),
                        red(args.len())
                    ))
                    .with_color(ariadne::Color::Red),
            );

                if args.len() > max_args_len {
                    let span: Span = (
                        args_indexes[max_args_len].start,
                        args_indexes[args_indexes.len() - 1].end,
                    )
                        .into();
                    report = report.with_label(
                        Label::new((src.filename.as_str(), span.into()))
                            .with_message(format_args!(
                                "{}: Remove {}",
                                blue("Help"),
                                if args.len() - max_args_len == 1 {
                                    "this argument"
                                } else {
                                    "those arguments"
                                }
                            ))
                            .with_color(ariadne::Color::Blue)
                            .with_priority(-1),
                    );
                }

                report.finish()
            },
            sources,
            file_idx,
            span,
            &format!("Function {fn_name} expects at least {min_args_len} and at most {max_args_len} arguments but {} were supplied", args.len()),
        );
    }
}

#[inline(never)]
#[cold]
pub fn error_struct_unknown_field(
    file_idx: u16,
    field_span: Span,
    field: &SmolStr,
    struct_name: &SmolStr,
    fields: &[(SmolStr, DataType, Span)],
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), field_span.into()),
            )
            .with_message("Unknown field")
            .with_label(
                Label::new((src.filename.as_str(), field_span.into()))
                    .with_message(format_args!(
                        "The field {} isn't defined in struct {}",
                        red(field),
                        blue(struct_name)
                    ))
                    .with_color(ariadne::Color::Red),
            );

            let similar_field = find_closest_str(field, fields.iter().map(|(f, _, _)| f.as_str()));
            if let Some(similar_field) = similar_field {
                report = report.with_help(format_args!(
                    "A field with a similar name exists: {}",
                    blue(similar_field)
                ));
            } else {
                report = report.with_help(format_args!(
                    "The available fields are: {}",
                    fields
                        .iter()
                        .map(|(field, _, _)| blue(field))
                        .collect::<Vec<_>>()
                        .join(", "),
                ));
            }
            report.finish()
        },
        sources,
        file_idx,
        field_span,
        &format!("The field {field} isn't defined in struct {struct_name}"),
    );
}

#[cold]
#[inline(never)]
pub fn error_struct_field_invalid_type(
    file_idx: u16,
    struct_name: &SmolStr,
    struct_field_span: Span,
    struct_field_name: &SmolStr,
    struct_field_type: &DataType,
    value_span: Span,
    value_type: &DataType,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), struct_field_span.into()),
            )
            .with_message("Incompatible types")
            .with_label(
                Label::new((src.filename.as_str(), struct_field_span.into()))
                    .with_message(format_args!(
                        "Field {} in struct {} expects type {}",
                        blue(struct_field_name),
                        blue(struct_name),
                        blue(struct_field_type)
                    ))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename.as_str(), value_span.into()))
                    .with_message(format_args!(
                        "This expression is of type {}",
                        red(value_type)
                    ))
                    .with_color(ariadne::Color::Red),
            );

            if struct_field_type == &DataType::Int
                && (value_type == &DataType::Float || value_type == &DataType::String)
            {
                report = report.with_help(format_args!("Try using the {} function", blue("int()")));
            } else if struct_field_type == &DataType::Float
                && (value_type == &DataType::Int || value_type == &DataType::String)
            {
                report =
                    report.with_help(format_args!("Try using the {} function", blue("float()")));
            } else if struct_field_type == &DataType::Bool && value_type == &DataType::String {
                report =
                    report.with_help(format_args!("Try using the {} function", blue("bool()")));
            } else if struct_field_type == &DataType::String {
                report = report.with_help(format_args!("Try using the {} function", blue("str()")));
            }

            report.finish()
        },
        sources,
        file_idx,
        value_span,
        &format!("Field {struct_field_name} in struct {struct_name} expects type {struct_field_type}, but this expression is of type {value_type}"),
    );
}

fn find_closest_str<'a>(name: &'a str, list: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut best: Option<(&str, usize)> = None;
    for candidate in list {
        let dist = levenshtein(name, candidate);
        if dist <= (name.len().max(candidate.len()) / 3).max(1)
            && best.is_none_or(|(_, d)| dist < d)
        {
            best = Some((candidate, dist));
        }
    }
    best.map(|(s, _)| s)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let m = a.len();
    let n = b.len();

    let mut v0: Vec<usize> = (0..=n).collect();
    let mut v1: Vec<usize> = vec![0; n + 1];

    for i in 0..m {
        v1[0] = i + 1;

        for j in 0..n {
            let deletion_cost = v0[j + 1] + 1;
            let insertion_cost = v1[j] + 1;
            let substitution_cost = if a.as_bytes()[i] == b.as_bytes()[j] {
                v0[j]
            } else {
                v0[j] + 1
            };
            v1[j + 1] = std::cmp::min(
                deletion_cost,
                std::cmp::min(insertion_cost, substitution_cost),
            );
        }

        std::mem::swap(&mut v0, &mut v1);
    }

    v0[n]
}

#[cold]
#[inline(never)]
pub fn error_unknown_variable(
    var_name: &SmolStr,
    span: Span,
    v: &[Variable],
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message("Unknown variable")
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!(
                        "Cannot find variable {} in this scope",
                        red(var_name),
                    ))
                    .with_color(ariadne::Color::Red),
            );

            let similar_var = find_closest_str(var_name, v.iter().map(|v| v.name.as_str()));
            if let Some(similar_var) = similar_var {
                report = report.with_help(format_args!(
                    "A variable with a similar name exists: {}",
                    blue(similar_var)
                ));
            }

            report.finish()
        },
        sources,
        file_idx,
        span,
        &format!("Cannot find variable {var_name} in this scope"),
    )
}

#[cold]
#[inline(never)]
pub fn error_unknown_function(
    fn_name: &str,
    span: Span,
    namespace: &Namespace,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    let similar_fn = find_closest_str(fn_name, namespace.fns().map(|f| f.0.as_str()));
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message("Unknown function")
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!(
                        "Cannot find function {} in this scope",
                        red(fn_name),
                    ))
                    .with_color(ariadne::Color::Red),
            );

            if let Some(similar_fn) = similar_fn {
                report = report.with_help(format_args!(
                    "A function with a similar name exists: {}",
                    blue(similar_fn)
                ));
            }

            report.finish()
        },
        sources,
        file_idx,
        span,
        &format!("Cannot find function {fn_name} in this scope"),
    )
}

#[cold]
#[inline(never)]
pub fn error_unknown_namespace(
    namespace: &[SmolStr],
    span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message("Unknown namespace")
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!(
                        "{} is not a valid namespace",
                        red(namespace.join("::")),
                    ))
                    .with_color(ariadne::Color::Red),
            );

            report.finish()
        },
        sources,
        file_idx,
        span,
        &format!("{} is not a valid namespace", namespace.join("::")),
    )
}

#[cold]
#[inline(never)]
pub fn error_unknown_function_in_namespace(
    fn_name: &str,
    namespace: &Namespace,
    path: &[SmolStr],
    span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    let namespace_str = path.join("::");
    let namespace = namespace.walk_to_namespace(path, span, file_idx, sources);
    let similar_fn = find_closest_str(fn_name, namespace.fns().map(|s| s.0.as_str()));
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message("Unknown function in namespace")
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!(
                        "Cannot find function {} in namespace {}",
                        red(fn_name),
                        blue(&namespace_str)
                    ))
                    .with_color(ariadne::Color::Red),
            );

            if let Some(similar_fn) = similar_fn {
                report = report.with_help(format_args!(
                    "A function with a similar name exists: {}",
                    blue(format_args!("{namespace_str}::{similar_fn}"))
                ));
            }

            report.finish()
        },
        sources,
        file_idx,
        span,
        &format!("Cannot find function {fn_name} in namespace {namespace_str}"),
    )
}

#[cold]
#[inline(never)]
pub fn error_function_already_defined(
    func: &Function,
    redeclaration_span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let fn_src = &sources[func.src_file as usize];
            let report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), redeclaration_span.into()),
            )
            .with_message(format_args!("Function already exists"))
            .with_label(
                Label::new((fn_src.filename.as_str(), func.name_span.into()))
                    .with_message(format_args!("Already defined here"))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename.as_str(), redeclaration_span.into()))
                    .with_message(format_args!(
                        "Function {} is already defined",
                        blue(&func.name)
                    ))
                    .with_color(ariadne::Color::Red),
            );

            report.finish()
        },
        sources,
        file_idx,
        redeclaration_span,
        &format!("Function {} is already defined", func.name),
    )
}

#[cold]
#[inline(never)]
pub fn error_op(
    l: &DataType,
    r: &DataType,
    op: &str,
    span_l: Span,
    span_r: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span_l.extend(span_r).into()),
            );

            if (op == "-" && l == &DataType::Null) || op == "!" {
                report = report
                    .with_message(format_args!(
                        "Cannot perform operation {} {}",
                        red(op),
                        blue(r)
                    ))
                    .with_label(
                        Label::new((src.filename.as_str(), span_r.into()))
                            .with_message(format_args!("This expression is of type {}", blue(r)))
                            .with_color(ariadne::Color::Red),
                    );
            } else {
                report = report
                    .with_message(format_args!(
                        "Cannot perform operation {} {} {}",
                        blue(l),
                        red(op),
                        green(r)
                    ))
                    .with_label(
                        Label::new((src.filename.as_str(), span_l.into()))
                            .with_message(format_args!("This expression is of type {}", blue(l)))
                            .with_color(ariadne::Color::Red),
                    )
                    .with_label(
                        Label::new((src.filename.as_str(), span_r.into()))
                            .with_message(format_args!("This expression is of type {}", green(r)))
                            .with_color(ariadne::Color::Red),
                    );
            }

            if op == "+" {
                report = report.with_note(format_args!(
                    "The supported types are:\n- {} {} {}\n- {} {} {}\n- {} {} {}\n- {} {} {}",
                    blue(DataType::Int),
                    red(op),
                    green(DataType::Int),
                    blue(DataType::Float),
                    red(op),
                    green(DataType::Float),
                    blue(DataType::String),
                    red(op),
                    green(DataType::String),
                    blue("T[]"),
                    red(op),
                    green("T[]")
                ));
            } else if op == "-" && l == &DataType::Null {
                report = report.with_note(format_args!(
                    "The supported types are:\n- {} {}\n- {} {}",
                    red(op),
                    blue(DataType::Int),
                    red(op),
                    green(DataType::Float),
                ));
            } else if op == "*"
                || op == "/"
                || op == "-"
                || op == "%"
                || op == "^"
                || op == ">"
                || op == ">="
                || op == "<"
                || op == "<="
            {
                report = report.with_note(format_args!(
                    "The supported types are:\n- {} {} {}\n- {} {} {}",
                    blue(DataType::Int),
                    red(op),
                    green(DataType::Int),
                    blue(DataType::Float),
                    red(op),
                    green(DataType::Float),
                ));
            } else if op == "&&" || op == "||" {
                report = report.with_note(format_args!(
                    "The supported types are:\n- {} {} {}",
                    blue(DataType::Bool),
                    red(op),
                    green(DataType::Bool),
                ));
            } else if op == "!" {
                report = report.with_note(format_args!(
                    "The supported types are:\n- {} {}",
                    red(op),
                    blue(DataType::Bool),
                ));
            }

            report.finish()
        },
        sources,
        file_idx,
        span_l.extend(span_r),
        &if (op == "-" && l == &DataType::Null) || op == "!" {
            format!("Cannot perform operation {op} {r}")
        } else {
            format!("Cannot perform operation {l} {op} {r}")
        },
    );
}

#[cold]
#[inline(never)]
pub fn error_unknown_type(
    span: Span,
    file_idx: u16,
    t: &str,
    sources: &[Source],
    namespace: &Namespace,
) -> ! {
    let closest_struct = find_closest_str(t, namespace.structs().map(|s| s.0.as_str()));
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message(format_args!("Unknown type {}", red(t)))
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!("This isn't a valid type"))
                    .with_color(ariadne::Color::Red),
            );

            if let Some(s) = closest_struct {
                report = report.with_help(format_args!(
                    "A struct with a similar name exists: {}",
                    blue(s)
                ));
            }

            report.finish()
        },
        sources,
        file_idx,
        span,
        &format!("Unknown type {t}"),
    )
}

#[cold]
#[inline(never)]
pub fn error_unknown_type_with_namespace(
    span: Span,
    file_idx: u16,
    t: &str,
    sources: &[Source],
    namespace: &Namespace,
    path: &[SmolStr],
) -> ! {
    let namespace_str = path.join("::");
    let namespace = namespace.walk_to_namespace(path, span, file_idx, sources);
    let closest_struct = find_closest_str(t, namespace.structs().map(|s| s.0.as_str()));
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message(format_args!("Unknown type {}", red(t)))
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!("This isn't a valid type"))
                    .with_color(ariadne::Color::Red),
            );

            if let Some(s) = closest_struct {
                report = report.with_help(format_args!(
                    "A struct with a similar name exists: {}::{}",
                    blue(&namespace_str),
                    blue(s)
                ));
            }

            report.finish()
        },
        sources,
        file_idx,
        span,
        &format!("Unknown type {t}"),
    )
}

#[cold]
#[inline(never)]
pub fn error_duplicate_map_key(
    key_first_span: Span,
    key_repeat_span: Span,
    span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message(format_args!("Key is defined more than once in map"))
            .with_label(
                Label::new((src.filename.as_str(), key_first_span.into()))
                    .with_message(format_args!("This key is first defined here"))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename.as_str(), key_repeat_span.into()))
                    .with_message(format_args!("It's then redefined here"))
                    .with_color(ariadne::Color::Red),
            )
            .finish()
        },
        sources,
        file_idx,
        key_repeat_span,
        "Key is defined more than once in map",
    );
}

#[cold]
#[inline(never)]
pub fn error_not_literal_map_key(
    key_span: Span,
    map_span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), map_span.into()),
            )
            .with_message(format_args!("Non-literal map key"))
            .with_label(
                Label::new((src.filename.as_str(), key_span.into()))
                    .with_message(format_args!("This map key is not a literal."))
                    .with_color(ariadne::Color::Blue),
            )
            .with_note("Map keys must be literals. However, fret not, this requirement™ will soon be removed!")
            .finish()
        },
        sources,
        file_idx,
        key_span,
        "Map keys must be literals",
    );
}

#[cold]
#[inline(never)]
pub fn error_function_arg_invalid_type(
    perceived_type: &DataType,
    expected_type: &DataType,
    arg_span: Span,
    fn_name: &str,
    fn_decl_span: Option<(Span, u16)>,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), arg_span.into()),
            )
            .with_message(format_args!("Invalid argument type"));

            if let Some((fn_span, fn_file_idx)) = fn_decl_span {
                let fn_src = &sources[fn_file_idx as usize];
                report = report.with_label(
                    Label::new((fn_src.filename.as_str(), fn_span.into()))
                        .with_message("Function is defined here")
                        .with_color(ariadne::Color::Blue),
                );
            }

            report = report.with_label(
                Label::new((src.filename.as_str(), arg_span.into()))
                    .with_message(format_args!("Function {} expects this argument's type to be {}, but this expression's type is {}", blue(fn_name), green(expected_type), red(perceived_type)))
                    .with_color(ariadne::Color::Red),
            );

            report.finish()
        },
        sources,
        file_idx,
        arg_span,
        &format!("Function {fn_name} expects this argument's type to be {expected_type}, but this expression's type is {perceived_type}"),
    );
}

#[cold]
#[inline(never)]
pub fn error_function_arg_invalid_type_multiple(
    perceived_type: &DataType,
    expected_type: &[DataType],
    arg_span: Span,
    fn_name: &str,
    fn_decl_span: Option<(Span, u16)>,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), arg_span.into()),
            )
            .with_message(format_args!("Invalid argument type"));

            if let Some((fn_span, fn_file_idx)) = fn_decl_span {
                let fn_src = &sources[fn_file_idx as usize];
                report = report.with_label(
                    Label::new((fn_src.filename.as_str(), fn_span.into()))
                        .with_message("Function is defined here")
                        .with_color(ariadne::Color::Blue),
                );
            }

            report = report.with_label(
                Label::new((src.filename.as_str(), arg_span.into()))
                    .with_message(format_args!("Function {} expects this argument to be of type {GREEN}{}{RESET}, but this expression's type is {}", blue(fn_name), expected_type.iter().map(|s| s.to_smolstr()).collect::<Vec<SmolStr>>().join("{RESET} or {GREEN}"), red(perceived_type)))
                    .with_color(ariadne::Color::Red),
            );

            report.finish()
        },
        sources,
        file_idx,
        arg_span,
        &format!("Function {fn_name} expects this argument to be of type {}, but this expression's type is {perceived_type}", expected_type.iter().map(|s| s.to_smolstr()).collect::<Vec<SmolStr>>().join(" or ")),
    );
}

pub fn error_range_invalid_type(
    span: Span,
    perceived_type: &DataType,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_compiler_error(
        &|| {
            let src = &sources[file_idx as usize];
            let report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename.as_str(), span.into()),
            )
            .with_message(format_args!("Invalid type in range"))
            .with_label(
                Label::new((src.filename.as_str(), span.into()))
                    .with_message(format_args!(
                        "Expected {}, but this expression's type is {}",
                        blue(DataType::Int),
                        red(perceived_type)
                    ))
                    .with_color(ariadne::Color::Red),
            )
            .with_note(format_args!(
                "A range has the following syntax: {}..{},\nwhere both {} and {} are of type {}",
                blue("start"),
                blue("end"),
                blue("start"),
                blue("end"),
                blue(DataType::Int)
            ))
            .with_help(format_args!("Try using the {} function.", blue("int()")));

            report.finish()
        },
        sources,
        file_idx,
        span,
        &format!("Invalid type in range: expected int, but this expression's type is {perceived_type}"),
    );
}
