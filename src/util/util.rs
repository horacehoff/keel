use crate::compiler_data::Struct;
use crate::errors::ErrType;
use crate::errors::throw_compiler_error;
use crate::expr::Expr;
use crate::expr::Span;
use crate::type_system::DataType;
use smol_strc::SmolStr;
use std::hint::cold_path;

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

pub fn str_to_keel_type(s: &str, structs: &[Struct], span: Span, src: (&str, &str)) -> DataType {
    let b = s.as_bytes();
    if b[b.len() - 1] == b']' && b[b.len() - 2] == b'[' {
        DataType::Array(if b.len() == 2 {
            None
        } else {
            Some(Box::from(str_to_keel_type(
                &s[..s.len() - 2],
                structs,
                span,
                src,
            )))
        })
    } else if s == "int" {
        DataType::Int
    } else if s == "float" {
        DataType::Float
    } else if s == "bool" {
        DataType::Bool
    } else if s == "string" {
        DataType::String
    } else if s == "null" {
        DataType::Null
    } else if let Some(s) = structs.iter().rposition(|candidate| candidate.name == s) {
        DataType::Struct(s as u16)
    } else {
        throw_compiler_error(src, span, ErrType::UnknownType(s));
    }
}

pub fn check_args(
    args: &[Expr],
    expected_args_len: usize,
    fn_name: &str,
    src: (&str, &str),
    span: Span,
) {
    if args.len() != expected_args_len {
        cold_path();
        throw_compiler_error(
            src,
            span,
            ErrType::IncorrectArgCount(fn_name, expected_args_len as u16, args.len() as u16),
        )
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
