use crate::type_system::DataType;
use smol_str::SmolStr;
use smol_str::ToSmolStr;

/// Strips the surrounding quotes from a raw string token and processes
/// backslash escape sequences: \n \t \r \\ \\" \0
pub fn unescape_string(raw: &str) -> SmolStr {
    let inner = &raw[1..raw.len() - 1]; // strip surrounding "

    // Fast path -> no backslash
    let Some(first_bs) = inner.find('\\') else {
        return SmolStr::from(inner);
    };

    let mut out = String::with_capacity(inner.len());

    // Since find returns the first result, we know that inner[..first_bs] does not contain any backslash
    // This allows us to only search the rest of the string
    out.push_str(&inner[..first_bs]);
    let mut rest = &inner[first_bs..];

    loop {
        match rest.find('\\') {
            None => {
                out.push_str(rest); // there are no more escapes
                break;
            }
            Some(bs) => {
                out.push_str(&rest[..bs]); // copy chunk before the backslash
                let after = &rest[bs + 1..];
                if after.is_empty() {
                    out.push('\\'); // trailing lone backslash
                    break;
                }
                // All recognised escape codes are ASCII, so index by byte
                match after.as_bytes()[0] {
                    b'n' => {
                        out.push('\n');
                        rest = &after[1..];
                    }
                    b't' => {
                        out.push('\t');
                        rest = &after[1..];
                    }
                    b'r' => {
                        out.push('\r');
                        rest = &after[1..];
                    }
                    b'\\' => {
                        out.push('\\');
                        rest = &after[1..];
                    }
                    b'"' => {
                        out.push('"');
                        rest = &after[1..];
                    }
                    b'0' => {
                        out.push('\0');
                        rest = &after[1..];
                    }
                    _ => {
                        // Unknown escape -> keep backslash + character verbatim
                        // Use chars() only here to correctly handle multi-byte UTF-8.
                        let c = after.chars().next().unwrap();
                        out.push('\\');
                        out.push(c);
                        rest = &after[c.len_utf8()..];
                    }
                }
            }
        }
    }
    SmolStr::from(out)
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
