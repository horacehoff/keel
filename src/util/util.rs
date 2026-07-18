use crate::compiler::compiler_data::Source;
use crate::compiler::compiler_data::State;
use crate::compiler::expr::Expr;
use crate::compiler::expr::Span;
use crate::errors::ErrType;
use crate::errors::blue;
use crate::errors::bold;
use crate::errors::throw_compiler_error;
use crate::errors::throw_compiler_error_exp;
use ariadne::Label;
use ariadne::Report;
use smol_strc::SmolStr;
use std::hint::cold_path;
use std::rc::Rc;

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

pub const KEEL_LOGO: &str = "
  \x1b[34m// /\x1b[0m
 \x1b[34m// /\x1b[0m  keel
\x1b[34m// /\x1b[0m

by Horace Hoff.";
