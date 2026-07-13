use crate::compiler_data::State;
use crate::compiler_data::Struct;
use crate::errors::ErrType;
use crate::errors::blue;
use crate::errors::bold;
use crate::errors::throw_compiler_error;
use crate::errors::throw_compiler_error_exp;
use crate::expr::Expr;
use crate::expr::Span;
use crate::parser::TypeExpr;
use crate::type_system::DataType;
use ariadne::Label;
use ariadne::Report;
use smol_strc::SmolStr;
use std::hint::cold_path;
use std::rc::Rc;

/// Strips the surrounding quotes & processes escape sequences \n \t \r \\ \" \0
pub fn parse_string(s: &str) -> SmolStr {
    let inner = &s[1..s.len() - 1]; // Strip the surrounding quotes

    // Return the stripped string directly if it doesn't contain any escape sequences
    let Some(first_escape) = memchr::memchr(b'\\', inner.as_bytes()) else {
        return SmolStr::from(inner);
    };
    let mut processed = String::with_capacity(inner.len());

    // Find returns the first occurence, so we know that inner[..first_escape] does not contain any escape sequence
    processed.push_str(&inner[..first_escape]);
    let mut to_process = &inner[first_escape..];
    loop {
        match memchr::memchr(b'\\', to_process.as_bytes()) {
            None => {
                // there are no escape sequences left
                processed.push_str(to_process);
                break;
            }
            Some(escape_seq_idx) => {
                processed.push_str(&to_process[..escape_seq_idx]);
                let after = &to_process[escape_seq_idx + 1..];
                if after.is_empty() {
                    processed.push('\\');
                    break;
                }
                let escape_seq = after.as_bytes()[0];
                if escape_seq == b'n'
                    || escape_seq == b't'
                    || escape_seq == b'r'
                    || escape_seq == b'\\'
                    || escape_seq == b'"'
                    || escape_seq == b'0'
                {
                    processed.push(match escape_seq {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'\\' => '\\',
                        b'"' => '"',
                        b'0' => '\0',
                        _ => unreachable!(),
                    });
                    to_process = &after[1..];
                } else {
                    // chars() is used to correctly handle multi-byte characters
                    let c = after.chars().next().unwrap();
                    processed.push('\\');
                    processed.push(c);
                    to_process = &after[c.len_utf8()..];
                }
            }
        }
    }
    SmolStr::from(processed)
}

pub fn parse_keel_type(
    t: &TypeExpr,
    structs: &[Struct],
    span: Span,
    src: (&str, &str),
) -> DataType {
    match t {
        TypeExpr::Identifier(s) => match s.as_str() {
            "int" => DataType::Int,
            "float" => DataType::Float,
            "bool" => DataType::Bool,
            "string" => DataType::String,
            "null" => DataType::Null,
            other => {
                if let Some(id) = structs.iter().rposition(|st| st.name == other) {
                    DataType::Struct(id as u16)
                } else {
                    cold_path();
                    throw_compiler_error(src, span, ErrType::UnknownType(s))
                }
            }
        },
        TypeExpr::Array(inner_t) => {
            DataType::Array(Some(Box::new(parse_keel_type(inner_t, structs, span, src))))
        }
        TypeExpr::Map(k_t, v_t) => DataType::Map(Box::from((
            Some(parse_keel_type(k_t, structs, span, src)),
            Some(parse_keel_type(v_t, structs, span, src)),
        ))),
        TypeExpr::Union(poly) => DataType::Poly(
            poly.iter()
                .map(|t| parse_keel_type(t, structs, span, src))
                .collect(),
        )
        .check_poly(),
    }
}

pub fn check_args(
    args: &[Expr],
    expected_args_len: usize,
    fn_name: &str,
    (filename, _): (&str, &str),
    span: Span,
    sources: &[(SmolStr, Rc<String>)],
) {
    if args.len() != expected_args_len {
        throw_compiler_error_exp(
            || {
                let mut report = Report::build(ariadne::ReportKind::Error, (filename, span.into()))
                    .with_message("Invalid argument count")
                    .with_label(
                        Label::new((filename, span.into()))
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
    src: (&str, &str),
    span: Span,
    fn_decl_span: (Span, u16),
    compiler_state: &State,
) {
    if args.len() != expected_args_len {
        throw_compiler_error_exp(
            || {
                let fn_src = &compiler_state.sources[fn_decl_span.1 as usize];
                let mut report = Report::build(ariadne::ReportKind::Error, (src.0, span.into()))
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
                        Label::new((src.0, span.into()))
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
            &compiler_state.sources,
        );
    }
}

pub fn check_args_range(
    args: &[Expr],
    min_args_len: usize,
    max_args_len: usize,
    fn_name: &str,
    src: (&str, &str),
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
