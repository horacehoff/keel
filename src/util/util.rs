use crate::type_system::DataType;
use smol_str::SmolStr;
use smol_str::ToSmolStr;

/// Strips the surrounding quotes & processes escape sequences \n \t \r \\ \" \0
pub fn parse_string(s: &str) -> SmolStr {
    let inner = &s[1..s.len() - 1]; // Strip the surrounding quotes

    // Return the stripped string directly if it doesn't contain any escape sequences
    let Some(first_escape) = inner.find('\\') else {
        return SmolStr::from(inner);
    };
    let mut processed = String::with_capacity(inner.len());

    // Find returns the first occurence, so we know that inner[..first_escape] does not contain any escape sequence
    processed.push_str(&inner[..first_escape]);
    let mut to_process = &inner[first_escape..];
    loop {
        match to_process.find('\\') {
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

pub fn str_to_type(s: &str) -> DataType {
    if s == "int" {
        DataType::Int
    } else if s == "float" {
        DataType::Float
    } else if s == "bool" {
        DataType::Bool
    } else if s == "string" {
        DataType::String
    } else {
        unreachable!()
    }
}

#[macro_export]
macro_rules! span {
    ($start:expr,$end:expr) => {
        Span {
            start: $start as u32,
            end: $end as u32,
        }
    };
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataType::Float => write!(f, "Float"),
            DataType::Int => write!(f, "Integer"),
            DataType::Bool => write!(f, "Boolean"),
            DataType::String => write!(f, "String"),
            DataType::Array(array_type) => match array_type {
                Some(t) => write!(f, "Array<{}>", t),
                None => write!(f, "Array<?>"),
            },
            DataType::Null => write!(f, "Null"),
            DataType::Unknown => write!(f, "Unknown"),
            DataType::File => write!(f, "File"),
            DataType::Poly(types) => write!(
                f,
                "{}",
                types
                    .into_iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>()
                    .join("|")
            ),
            DataType::Fn(t) => {
                write!(
                    f,
                    "({}) -> {}",
                    t[..1]
                        .iter()
                        .map(|x| x.to_smolstr())
                        .collect::<Vec<_>>()
                        .join(", "),
                    {
                        let x = t.last().unwrap();
                        if x == &DataType::Null {
                            SmolStr::new_static("")
                        } else {
                            x.to_smolstr()
                        }
                    }
                )
            }
        }
    }
}

/// check_args(args, expected_args_len, fn_name, src, markers)
#[macro_export]
macro_rules! check_args {
    ($args:expr, $expected_args_len:expr, $fn_name:expr, $src:expr,$markers:expr) => {
        if $args.len() != $expected_args_len {
            throw_parser_error(
                $src,
                $markers,
                ErrType::IncorrectFuncArgCount(
                    $fn_name,
                    $expected_args_len as u16,
                    $args.len() as u16,
                ),
            );
        }
    };
}

/// check_args_range(args, min_args_len, max_args_len, fn_name, src, markers)
#[macro_export]
macro_rules! check_args_range {
    ($args:expr, $min_args_len:expr,$max_args_len:expr, $fn_name:expr, $src:expr,$markers:expr) => {
        #[allow(unused_comparisons)]
        if $args.len() < $min_args_len || $args.len() > $max_args_len {
            throw_parser_error(
                $src,
                $markers,
                ErrType::IncorrectFuncArgCountVariable(
                    $fn_name.into(),
                    $min_args_len as u16,
                    $max_args_len as u16,
                    $args.len() as u16,
                ),
            );
        }
    };
}

pub const KEEL_LOGO: &str = "
  \x1b[34m// /\x1b[0m
 \x1b[34m// /\x1b[0m  keel
\x1b[34m// /\x1b[0m

by Horace Hoff.";
