use crate::display::token_recognition;
use crate::expr::Span;
use crate::{instr::Instr, type_system::DataType};
use ariadne::{Color, Label, Report, ReportKind, Source};
use inline_colorization::*;
use lalrpop_util::ParseError;
use lalrpop_util::lexer::Token;
use smol_str::{SmolStr, ToSmolStr};
use std::fmt::Arguments;

#[cold]
#[inline(always)]
pub fn dev_error(file: &str, function: &str, additional_data: Arguments) -> ! {
    unreachable!(
        "\n--------------\n{color_red}KEEL COMPILATION ERROR:{color_reset}\nFROM FILE: {}\nFROM FUNCTION: {}\nADDITIONAL DATA: {}\n--------------",
        file, function, additional_data
    );
}

pub struct ErrorCtx {
    pub instr_src: Vec<(Instr, Span, u16)>,
    pub sources: Vec<(SmolStr, String)>,
}

impl From<std::io::ErrorKind> for ErrType<'_> {
    fn from(value: std::io::ErrorKind) -> Self {
        match value {
            std::io::ErrorKind::AlreadyExists => ErrType::IOAlreadyExists,
            std::io::ErrorKind::Deadlock => ErrType::IODeadlock,
            std::io::ErrorKind::FileTooLarge => ErrType::IOFileTooLarge,
            std::io::ErrorKind::Interrupted => ErrType::IOInterrupted,
            std::io::ErrorKind::InvalidData => ErrType::IOInvalidData,
            std::io::ErrorKind::InvalidFilename => ErrType::IOInvalidFilename,
            std::io::ErrorKind::IsADirectory => ErrType::IOIsADirectory,
            std::io::ErrorKind::NotADirectory => ErrType::IONotADirectory,
            std::io::ErrorKind::NotFound => ErrType::IONotFound,
            std::io::ErrorKind::PermissionDenied => ErrType::IOPermissionDenied,
            std::io::ErrorKind::OutOfMemory => ErrType::IOOutOfMemory,
            std::io::ErrorKind::ReadOnlyFilesystem => ErrType::IOReadOnlyFilesystem,
            std::io::ErrorKind::StorageFull => ErrType::IOStorageFull,
            std::io::ErrorKind::TimedOut => ErrType::IOTimedOut,
            other => ErrType::Custom(other.to_smolstr()),
        }
    }
}

impl From<std::num::IntErrorKind> for ErrType<'_> {
    fn from(value: std::num::IntErrorKind) -> Self {
        match value {
            std::num::IntErrorKind::Empty => ErrType::IntEmpty,
            std::num::IntErrorKind::InvalidDigit => ErrType::IntInvalidDigit,
            std::num::IntErrorKind::NegOverflow => ErrType::IntNegOverflow,
            std::num::IntErrorKind::PosOverflow => ErrType::IntPosOverflow,
            std::num::IntErrorKind::Zero => dev_error(
                file!(),
                "impl From<std::num::IntErrorKind> for ErrType<'_>",
                format_args!("Encountered std::num::IntErrorKind::Zero"),
            ),
            _ => unreachable!(),
        }
    }
}

/// Error types, largely borrowed from Rust
pub enum ErrType<'a> {
    Custom(SmolStr),

    // IO ERRORS
    IOAlreadyExists,
    IODeadlock,
    IOFileTooLarge,
    IOInterrupted,
    IOInvalidData,
    IOInvalidFilename,
    /// When a file was expected...
    IOIsADirectory,
    /// When a directory was expected...
    IONotADirectory,
    IONotFound,
    IOPermissionDenied,
    IOOutOfMemory,
    IOReadOnlyFilesystem,
    IOStorageFull,
    IOTimedOut,

    // INT PARSING ERRORS
    IntEmpty,
    IntInvalidDigit,
    IntNegOverflow,
    IntPosOverflow,

    // FLOAT PARSING ERRORS
    FloatParsingError,

    // BOOL PARSING ERRORS
    BoolParsingError,

    /// IndexOutOfBounds(length, index)
    IndexOutOfBounds(usize, i32),

    /// RangeOutOfBounds(length, idx_start, idx_end)
    RangeOutOfBounds(usize, i32, i32),

    InvalidFloat,
    IntTooBig,

    // PARSER ERRORS
    UnknownVariable(&'a str),
    UnknownFunction(&'a str),
    UnknownNamespace(&'a str),
    /// When an array holds two or more different types
    ArrayWithDiffType,
    NotIndexable(&'a DataType),
    InvalidIndexType(&'a DataType),
    /// CannotPushTypeToArray(elem_type, array_type)
    CannotPushTypeToArray(&'a DataType, &'a DataType),
    CannotInferType(&'a str),
    /// IncorrectFuncArgCount(fn_name, expected, received)
    IncorrectFuncArgCount(&'a str, u16, u16),
    IncorrectFuncArgCountVariable(&'a str, u16, u16, u16),
    /// InvalidType(expected_type, received_type)
    InvalidType(DataType, &'a DataType),
    /// OpError(l, r, op)
    OpError(&'a DataType, &'a DataType, &'a str),
    /// InvalidOp(type, op)
    InvalidOp(&'a DataType, &'a str),
    InvalidConditionalExpression,
    FunctionAlreadyExists(&'a str),
    CannotReadImportedFile(&'a str),
    /// CircularImport(path)
    CircularImport(&'a str),
    /// DuplicateFunctionInImport(fn_name, file_path)
    DuplicateFunctionInImport(&'a str, &'a str),
    IsNotAnIterator(&'a DataType),
    /// InvalidArgType(expected_types, received_type)
    InvalidArgType(&'a [DataType], DataType),
    /// InvalidObjType(expected_description, received_type)
    InvalidObjType(&'a str, &'a DataType),
    DivisionByZero,
    ModuloByZero,
}

impl From<ErrType<'_>> for SmolStr {
    fn from(value: ErrType) -> Self {
        match value {
            ErrType::Custom(m) => m,
            ErrType::CannotReadImportedFile(filename) => format_args!("Cannot read imported file {color_bright_red}{style_bold}{filename}{color_reset}{style_reset}").to_smolstr(),
            ErrType::IntTooBig => "The integer is too big".into(),
            ErrType::InvalidFloat => "Invalid float".into(),
            ErrType::IndexOutOfBounds(length, index) => format_args!("Tried to get index {color_bright_red}{style_bold}{index}{color_reset}{style_reset} but the length is {color_bright_blue}{style_bold}{length}{color_reset}{style_reset}").to_smolstr(),
            ErrType::RangeOutOfBounds(length, idx_start, idx_end) => format_args!("Invalid range {color_bright_red}{style_bold}{idx_start}{color_reset}{style_reset}..{color_bright_red}{style_bold}{idx_end}{color_reset}{style_reset} for collection with length {color_bright_blue}{style_bold}{length}{color_reset}{style_reset}").to_smolstr(),
            ErrType::BoolParsingError => "The string could not be parsed into a boolean".into(),
            ErrType::FloatParsingError => "The string could not be parsed into a float".into(),
            ErrType::IntEmpty => "The parsing string is empty".into(),
            ErrType::IntInvalidDigit => "The parsing string contains an invalid digit".into(),
            ErrType::IntPosOverflow => "The integer is too large".into(),
            ErrType::IntNegOverflow => "The integer is too small".into(),
            ErrType::IOAlreadyExists => "The entity (directory, file, ...) already exists".into(),
            ErrType::IODeadlock => "This operation would result in a deadlock".into(),
            ErrType::IOFileTooLarge => "The file is too large".into(),
            ErrType::IOInterrupted => "This operation was interrupted".into(),
            ErrType::IOInvalidData => "Malformed or invalid data were encountered".into(),
            ErrType::IOInvalidFilename => "The filename is invalid or too long".into(),
            ErrType::IOIsADirectory => {
                "This operation encountered a directory, when a non-directory was expected".into()
            }
            ErrType::IONotADirectory => {
                "This operation encountered a non-directory, when a directory was expected".into()
            }
            ErrType::IONotFound => "The entity (directory, file, ...) was not found".into(),
            ErrType::IOPermissionDenied => {
                "This operation lacked the necessary privileges to complete".into()
            }
            ErrType::IOOutOfMemory => {
                "This operation could not be completed, because it failed to allocate enough memory"
                    .into()
            }
            ErrType::IOReadOnlyFilesystem => {
                "The filesystem or storage medium is read-only, but a write operation was attempted"
                    .into()
            }
            ErrType::IOStorageFull => "Storage is full".into(),
            ErrType::IOTimedOut => "This operation timed out".into(),
            ErrType::UnknownFunction(f) => format_args!(
                "Unknown function {color_bright_blue}{style_bold}{f}{color_reset}{style_reset}"
            )
            .to_smolstr(),
            ErrType::UnknownVariable(v) => format_args!(
                "Unknown variable {color_bright_blue}{style_bold}{v}{color_reset}{style_reset}"
            )
            .to_smolstr(),
            ErrType::UnknownNamespace(n) => format_args!(
                "Unknown namespace {color_bright_blue}{style_bold}{n}{color_reset}{style_reset}"
            )
            .to_smolstr(),
            ErrType::ArrayWithDiffType => "Arrays can only hold a single type".into(),
            ErrType::NotIndexable(t) => format_args!(
                "The type {color_bright_blue}{style_bold}{t}{color_reset}{style_reset} cannot be indexed"
            )
            .to_smolstr(),
            ErrType::InvalidIndexType(t) => format_args!(
                "The type {color_bright_blue}{style_bold}{t}{color_reset}{style_reset} is not a valid index"
            )
            .to_smolstr(),
            ErrType::CannotPushTypeToArray(elem_t, array_t) => format_args!("Cannot insert {color_bright_blue}{style_bold}{elem_t}{color_reset}{style_reset} in {array_t}").to_smolstr(),
            ErrType::CannotInferType(t) => format_args!(
                "Cannot infer the type of {color_bright_blue}{style_bold}{t}{color_reset}{style_reset}"
            )
            .to_smolstr(),
            ErrType::IncorrectFuncArgCount(fn_name, expected, received) => format_args!("Function {color_bright_blue}{style_bold}{fn_name}{color_reset}{style_reset} expects {expected} argument{} but received {received}", if expected != 1 {"s"} else {""} ).to_smolstr(),
            ErrType::IncorrectFuncArgCountVariable(fn_name, expected_min, expected_max, received) => format_args!("Function {color_bright_blue}{style_bold}{fn_name}{color_reset}{style_reset} expects between {expected_min} and {expected_max} arguments but received {received}").to_smolstr(),
            ErrType::InvalidType(expected, received) => format_args!("Expected type {expected}, found {color_bright_blue}{style_bold}{received}{color_reset}{style_reset}").to_smolstr(),
            ErrType::OpError(l, r, op) => format_args!(
                "Cannot perform operation {color_bright_blue}{style_bold}{l} {color_red}{op}{color_bright_blue} {r}{color_reset}{style_reset}").to_smolstr(),
            ErrType::InvalidOp(t, op) => format_args!(
                "Operation {color_bright_red}{style_bold}{op}{color_reset}{style_reset} is not supported for type {color_bright_blue}{style_bold}{t}{color_reset}{style_reset}").to_smolstr(),
            ErrType::InvalidConditionalExpression => "Conditional expressions must have an else clause".into(),
            ErrType::FunctionAlreadyExists(fn_name) => format_args!(
                "Function {color_bright_red}{style_bold}{fn_name}{color_reset}{style_reset} is already defined",
            ).to_smolstr(),
            ErrType::CircularImport(path) => format_args!(
                "Circular import detected: {color_bright_red}{style_bold}{path}{color_reset}{style_reset} is already being imported"
            ).to_smolstr(),
            ErrType::DuplicateFunctionInImport(fn_name, file_path) => format_args!(
                "Function {color_bright_blue}{style_bold}{fn_name}{color_reset}{style_reset} imported from {color_bright_red}{style_bold}{file_path}{color_reset}{style_reset} is already defined"
            ).to_smolstr(),
            ErrType::IsNotAnIterator(t) => format_args!("The type {color_bright_red}{style_bold}{t}{color_reset}{style_reset} is not a collection").to_smolstr(),
            ErrType::InvalidArgType(expected, received) => {
                let expected_str = expected
                    .iter()
                    .map(|x| format!("{color_bright_blue}{style_bold}{x}{color_reset}{style_reset}"))
                    .collect::<Vec<String>>()
                    .join(" or ");
                format_args!(
                    "Expected {expected_str}, found {color_bright_red}{style_bold}{received}{color_reset}{style_reset}"
                ).to_smolstr()
            }
            ErrType::InvalidObjType(expected, received) => format_args!(
                "Expected {color_bright_blue}{style_bold}{expected}{color_reset}{style_reset}, found {color_bright_red}{style_bold}{received}{color_reset}{style_reset}"
            ).to_smolstr(),
            ErrType::DivisionByZero => "Division by zero. I'm sorry Dave, I'm afraid I can't do that.".into(),
            ErrType::ModuloByZero => "Modulo by zero. I'm sorry Dave, I'm afraid I can't do that.".into()
        }
    }
}

#[cold]
#[inline(never)]
pub fn throw_error(ctx: &ErrorCtx, instr: &Instr, t: ErrType) -> ! {
    let (_, Span { start, end }, file_idx) = ctx
        .instr_src
        .iter()
        .find(|(x, _, _)| x == instr)
        .unwrap_or(&(Instr::Halt(1), Span { start: 0, end: 0 }, 0));
    let src = &ctx.sources[*file_idx as usize];
    let err_message: SmolStr = t.into();
    eprintln!("{color_red}KEEL ERROR{color_reset}");
    let report = Report::build(
        ReportKind::Error,
        (src.0.as_str(), (*start as usize)..(*end as usize)),
    )
    .with_label(
        Label::new((src.0.as_str(), (*start as usize)..(*end as usize)))
            .with_message(err_message)
            .with_color(Color::Red),
    )
    .finish();

    #[cfg(not(target_arch = "wasm32"))]
    report
        .eprint((src.0.as_str(), Source::from(src.1.as_str())))
        .unwrap();

    #[cfg(target_arch = "wasm32")]
    report
        .write(
            (src.0.as_str(), Source::from(src.1.as_str())),
            crate::wasm_output::WasmWriter,
        )
        .unwrap();

    #[cfg(debug_assertions)]
    panic!();

    #[cfg(not(any(debug_assertions, target_arch = "wasm32")))]
    std::process::exit(1);

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen::throw_str("keel_error");
}

#[cold]
#[inline(never)]
#[cfg(target_arch = "wasm32")]
pub fn wasm_error(msg: &str) -> ! {
    crate::wasm_output::print(&format!("KEEL ERROR\n{msg}\n"));
    wasm_bindgen::throw_str("keel error");
}

#[cold]
#[inline(never)]
pub fn throw_parser_error(src: (&str, &str), Span { start, end }: &Span, t: ErrType) -> ! {
    let err_message: SmolStr = t.into();
    eprintln!("{color_red}KEEL ERROR{color_reset}");
    let report = Report::build(
        ReportKind::Error,
        (src.0, (*start as usize)..(*end as usize)),
    )
    .with_label(
        Label::new((src.0, (*start as usize)..(*end as usize)))
            .with_message(err_message)
            .with_color(Color::Red),
    )
    .finish();

    #[cfg(not(target_arch = "wasm32"))]
    report.eprint((src.0, Source::from(src.1))).unwrap();

    #[cfg(target_arch = "wasm32")]
    report
        .write((src.0, Source::from(src.1)), crate::wasm_output::WasmWriter)
        .unwrap();

    #[cfg(debug_assertions)]
    panic!();

    #[cfg(not(any(debug_assertions, target_arch = "wasm32")))]
    std::process::exit(1);

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen::throw_str("keel_error");
}

#[cold]
#[inline(never)]
pub fn lalrpop_error<'a, T>(x: ParseError<usize, T, &str>, file: &str, filename: &str) -> !
where
    Token<'a>: From<T>,
{
    eprintln!("{color_red}KEEL ERROR{color_reset}");
    match x {
        ParseError::InvalidToken { location } => {
            let report = Report::build(ReportKind::Error, (filename, location..location + 1))
                .with_message("Invalid token")
                .with_label(
                    Label::new((filename, location..location + 1))
                        .with_message(format_args!("This token is invalid"))
                        .with_color(Color::Red),
                )
                .finish();
            #[cfg(not(target_arch = "wasm32"))]
            report.eprint((filename, Source::from(file))).unwrap();
            #[cfg(target_arch = "wasm32")]
            report
                .write(
                    (filename, Source::from(file)),
                    crate::wasm_output::WasmWriter,
                )
                .unwrap();
        }
        ParseError::UnrecognizedEof {
            location,
            expected: _,
        } => {
            let report = Report::build(ReportKind::Error, (filename, location..location + 1))
                .with_message("Unrecognized EOF")
                .with_label(
                    Label::new((filename, location..location + 1))
                        .with_message(format_args!(
                            "Expected one or more {color_bright_blue}{style_bold}}}{style_reset}{color_reset}"
                        ))
                        .with_color(Color::Red),
                )
                .finish();
            #[cfg(not(target_arch = "wasm32"))]
            report.eprint((filename, Source::from(file))).unwrap();
            #[cfg(target_arch = "wasm32")]
            report
                .write(
                    (filename, Source::from(file)),
                    crate::wasm_output::WasmWriter,
                )
                .unwrap();
        }
        ParseError::UnrecognizedToken { token, expected } => {
            let begin = token.0;
            let end = token.2;

            let expected_tokens = expected
                .into_iter()
                .filter_map(|x| {
                    if x == "\"false\"" {
                        None
                    } else {
                        Some(format!(
                            "{color_bright_blue}{style_bold}{}{style_reset}{color_reset}",
                            token_recognition(&x)
                        ))
                    }
                })
                .collect::<Vec<String>>()
                .join(" OR ");

            let report = Report::build(ReportKind::Error, (filename, begin..end))
                .with_message("Unrecognized token")
                .with_label(
                    Label::new((filename, begin..end))
                        .with_message(format_args!("Expected {}", expected_tokens))
                        .with_color(Color::Red),
                )
                .finish();
            #[cfg(not(target_arch = "wasm32"))]
            report.eprint((filename, Source::from(file))).unwrap();
            #[cfg(target_arch = "wasm32")]
            report
                .write(
                    (filename, Source::from(file)),
                    crate::wasm_output::WasmWriter,
                )
                .unwrap();
        }
        _ => unreachable!(),
    }

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen::throw_str("keel_error");
    #[cfg(not(target_arch = "wasm32"))]
    panic!();
}
