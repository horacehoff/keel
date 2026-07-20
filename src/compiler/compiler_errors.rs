use super::DataType;
use super::Expr;
use super::Function;
use super::Namespace;
use super::Source;
use super::Span;
use super::State;
use super::Variable;
use crate::compiler::SymbolKind;
use crate::errors::ErrType;
use crate::errors::blue;
use crate::errors::bold;
use crate::errors::green;
use crate::errors::red;
use crate::errors::throw_compiler_error;
use crate::errors::throw_compiler_error_exp;
use ariadne::Label;
use ariadne::Report;
use smol_strc::SmolStr;
use smol_strc::ToSmolStr;
use std::hint::cold_path;
use std::rc::Rc;

#[inline(never)]
#[cold]
pub fn error_array_diff_types(
    src: Source,
    sources: &[(SmolStr, Rc<String>)],
    array_span: Span,
    array_elem_type: &DataType,
    failing_elem_span: Span,
    failing_elem_type: &DataType,
) -> ! {
    throw_compiler_error_exp(
        || {
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename, array_span.into()),
            )
            .with_message("Invalid array types")
            .with_label(
                Label::new((src.filename, array_span.into()))
                    .with_message(format_args!(
                        "This expression is of type {}",
                        blue(array_elem_type)
                    ))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename, failing_elem_span.into()))
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
    );
}

#[inline(never)]
#[cold]
pub fn error_unknown_struct(
    struct_name: &SmolStr,
    struct_span: Span,
    sources: &[(SmolStr, Rc<String>)],
    src: Source,
) -> ! {
    throw_compiler_error_exp(
        || {
            Report::build(
                ariadne::ReportKind::Error,
                (src.filename, struct_span.into()),
            )
            .with_message("Unknown struct")
            .with_label(
                Label::new((src.filename, struct_span.into()))
                    .with_message(format_args!("Unknown struct {}", red(struct_name)))
                    .with_color(ariadne::Color::Red),
            )
            .finish()
        },
        sources,
    );
}

pub fn error_struct_no_such_field(
    src: Source,
    struct_name: &SmolStr,
    struct_span: Span,
    struct_field_span: Span,
    struct_field_name: &SmolStr,
    sources: &[(SmolStr, Rc<String>)],
) -> ! {
    throw_compiler_error_exp(
        || {
            let report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename, struct_field_span.into()),
            )
            .with_message("Unknown struct field")
            .with_label(
                Label::new((src.filename, struct_span.into()))
                    .with_message(format_args!("Struct defined here"))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename, struct_field_span.into()))
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
    );
}

pub fn error_struct_missing_fields(
    src: Source,
    struct_span: Span,
    struct_literal_span: Span,
    sources: &[(SmolStr, Rc<String>)],
    missing_fields: &[&SmolStr],
) -> ! {
    throw_compiler_error_exp(
        || {
            let report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename, struct_literal_span.into()),
            )
            .with_message("Missing struct fields")
            .with_label(
                Label::new((src.filename, struct_span.into()))
                    .with_message(format_args!("Struct defined here"))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename, struct_literal_span.into()))
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
    );
}

pub fn check_args(
    args: &[Expr],
    expected_args_len: usize,
    fn_name: &str,
    src: Source,
    span: Span,
    sources: &[(SmolStr, Rc<String>)],
) {
    if args.len() != expected_args_len {
        throw_compiler_error_exp(
            || {
                let report = Report::build(ariadne::ReportKind::Error, (src.filename, span.into()))
                    .with_message("Invalid argument count")
                    .with_label(
                        Label::new((src.filename, span.into()))
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
        );
    }
}

pub fn check_args_user_fn(
    args: &[Expr],
    expected_args_len: usize,
    fn_name: &str,
    src: Source,
    span: Span,
    fn_decl_span: (Span, u16),
    compiler_state: &State,
    args_indexes: &[Span],
) {
    let args_len = args.len();
    if args_len != expected_args_len {
        throw_compiler_error_exp(
            || {
                let fn_src = &compiler_state.sources[fn_decl_span.1 as usize];
                let mut report = Report::build(
                    ariadne::ReportKind::Error,
                    (src.filename, span.into()),
                )
                .with_message("Invalid argument count")
                .with_label(
                    Label::new((fn_src.0.as_str(), fn_decl_span.0.into()))
                        .with_message(format_args!(
                            "The function {} is defined here",
                            blue(fn_name)
                        ))
                        .with_color(ariadne::Color::Blue),
                )
                .with_label(
                    Label::new((src.filename, span.into()))
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
                        Label::new((src.filename, span.into()))
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
            compiler_state.sources,
        );
    }
}

pub fn check_args_range(
    args: &[Expr],
    min_args_len: usize,
    max_args_len: usize,
    fn_name: &str,
    src: Source,
    span: Span,
) {
    if args.len() < min_args_len || args.len() > max_args_len {
        cold_path();
        throw_compiler_error(
            src,
            span,
            ErrType::IncorrectArgCountVariable(
                fn_name,
                min_args_len as u16,
                max_args_len as u16,
                args.len() as u16,
            ),
        )
    }
}

#[inline(never)]
#[cold]
pub fn error_struct_unknown_field(
    src: Source,
    field_span: Span,
    field: &SmolStr,
    struct_name: &SmolStr,
    fields: &[(SmolStr, DataType, Span)],
    sources: &[(SmolStr, Rc<String>)],
) -> ! {
    throw_compiler_error_exp(
        || {
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename, field_span.into()),
            )
            .with_message("Unknown field")
            .with_label(
                Label::new((src.filename, field_span.into()))
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
    );
}

#[cold]
#[inline(never)]
pub fn error_struct_field_invalid_type(
    src: Source,
    struct_name: &SmolStr,
    struct_field_span: Span,
    struct_field_name: &SmolStr,
    struct_field_type: &DataType,
    value_span: Span,
    value_type: &DataType,
    sources: &[(SmolStr, Rc<String>)],
) -> ! {
    throw_compiler_error_exp(
        || {
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename, struct_field_span.into()),
            )
            .with_message("Incompatible types")
            .with_label(
                Label::new((src.filename, struct_field_span.into()))
                    .with_message(format_args!(
                        "Field {} in struct {} expects type {}",
                        blue(struct_field_name),
                        blue(struct_name),
                        blue(struct_field_type)
                    ))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename, value_span.into()))
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
    src: Source,
    sources: &[(SmolStr, Rc<String>)],
) -> ! {
    throw_compiler_error_exp(
        || {
            let mut report = Report::build(ariadne::ReportKind::Error, (src.filename, span.into()))
                .with_message("Unknown variable")
                .with_label(
                    Label::new((src.filename, span.into()))
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
    )
}

#[cold]
#[inline(never)]
pub fn error_unknown_function<'a>(
    fn_name: &'a str,
    span: Span,
    fns: impl Iterator<Item = &'a str>,
    src: Source,
    sources: &[(SmolStr, Rc<String>)],
) -> ! {
    let similar_fn = find_closest_str(fn_name, fns);
    throw_compiler_error_exp(
        || {
            let mut report = Report::build(ariadne::ReportKind::Error, (src.filename, span.into()))
                .with_message("Unknown function")
                .with_label(
                    Label::new((src.filename, span.into()))
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
    )
}

#[cold]
#[inline(never)]
pub fn error_unknown_namespace(
    namespace: &[SmolStr],
    span: Span,
    src: Source,
    sources: &[(SmolStr, Rc<String>)],
) -> ! {
    throw_compiler_error_exp(
        || {
            let report = Report::build(ariadne::ReportKind::Error, (src.filename, span.into()))
                .with_message("Unknown namespace")
                .with_label(
                    Label::new((src.filename, span.into()))
                        .with_message(format_args!(
                            "{} is not a valid namespace",
                            red(namespace.join("::")),
                        ))
                        .with_color(ariadne::Color::Red),
                );

            report.finish()
        },
        sources,
    )
}

#[cold]
#[inline(never)]
pub fn error_unknown_function_in_namespace(
    fn_name: &str,
    namespace_root: &Namespace,
    namespace: &[SmolStr],
    span: Span,
    src: Source,
    sources: &[(SmolStr, Rc<String>)],
) -> ! {
    let mut current = namespace_root;
    for sub in namespace {
        current = if let Some(c) = current.children.iter().find(|n| n.name == *sub) {
            c
        } else {
            error_unknown_namespace(namespace, span, src, sources);
        };
    }
    let namespace_str = namespace.join("::");

    let similar_fn = find_closest_str(
        fn_name,
        current.symbols.iter().filter_map(|(name, symbol_kind)| {
            if matches!(symbol_kind, SymbolKind::Fn(_)) {
                Some(name.as_str())
            } else {
                None
            }
        }),
    );
    throw_compiler_error_exp(
        || {
            let mut report = Report::build(ariadne::ReportKind::Error, (src.filename, span.into()))
                .with_message("Unknown function in namespace")
                .with_label(
                    Label::new((src.filename, span.into()))
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
    )
}

#[cold]
#[inline(never)]
pub fn error_function_already_defined(
    func: &Function,
    redeclaration_span: Span,
    src: Source,
    sources: &[(SmolStr, Rc<String>)],
) -> ! {
    throw_compiler_error_exp(
        || {
            let fn_src = &sources[func.src_file as usize];
            let report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename, redeclaration_span.into()),
            )
            .with_message(format_args!("Function already exists"))
            .with_label(
                Label::new((fn_src.0.as_str(), func.name_span.into()))
                    .with_message(format_args!("Already defined here"))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.filename, redeclaration_span.into()))
                    .with_message(format_args!(
                        "Function {} is already defined",
                        blue(&func.name)
                    ))
                    .with_color(ariadne::Color::Red),
            );

            report.finish()
        },
        sources,
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
    src: Source,
    sources: &[(SmolStr, Rc<String>)],
) -> ! {
    throw_compiler_error_exp(
        || {
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.filename, span_l.extend(span_r).into()),
            );

            if (op == "-" && l == &DataType::Null) || op == "!" {
                report = report
                    .with_message(format_args!(
                        "Cannot perform operation {} {}",
                        red(op),
                        blue(r)
                    ))
                    .with_label(
                        Label::new((src.filename, span_r.into()))
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
                        Label::new((src.filename, span_l.into()))
                            .with_message(format_args!("This expression is of type {}", blue(l)))
                            .with_color(ariadne::Color::Red),
                    )
                    .with_label(
                        Label::new((src.filename, span_r.into()))
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
    );
}

#[cold]
#[inline(never)]
pub fn error_unknown_type<'a>(
    span: Span,
    src: Source,
    t: &'a str,
    sources: &[(SmolStr, Rc<String>)],
    structs: impl Iterator<Item = &'a str>,
) -> ! {
    let closest_struct = find_closest_str(t, structs);
    throw_compiler_error_exp(
        || {
            let mut report = Report::build(ariadne::ReportKind::Error, (src.filename, span.into()))
                .with_message(format_args!("Unknown type {}", red(t)))
                .with_label(
                    Label::new((src.filename, span.into()))
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
    )
}

#[cold]
#[inline(never)]
pub fn error_unknown_type_with_namespace<'a>(
    span: Span,
    src: Source,
    t: &'a str,
    sources: &[(SmolStr, Rc<String>)],
    namespace_root: &Namespace,
    namespace: &[SmolStr],
) -> ! {
    let mut current = namespace_root;
    for sub in namespace {
        current = if let Some(c) = current.children.iter().find(|n| n.name == *sub) {
            c
        } else {
            error_unknown_namespace(namespace, span, src, sources);
        };
    }
    let namespace_str = namespace.join("::");

    let closest_struct = find_closest_str(
        t,
        current.symbols.iter().filter_map(|(name, symbol_kind)| {
            if matches!(symbol_kind, SymbolKind::Struct(_)) {
                Some(name.as_str())
            } else {
                None
            }
        }),
    );
    throw_compiler_error_exp(
        || {
            let mut report = Report::build(ariadne::ReportKind::Error, (src.filename, span.into()))
                .with_message(format_args!("Unknown type {}", red(t)))
                .with_label(
                    Label::new((src.filename, span.into()))
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
    )
}
