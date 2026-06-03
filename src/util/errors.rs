use crate::expr::Span;
use crate::{instr::Instr, type_system::DataType};
use ariadne::{Color, Label, Report, ReportKind, Source};
use inline_colorization::*;
use lalrpop_util::ParseError;
use lalrpop_util::lexer::Token;
use smol_strc::{SmolStr, ToSmolStr};
use std::fmt::Arguments;
use std::rc::Rc;

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
    pub sources: Vec<(SmolStr, Rc<String>)>,
}

impl From<std::io::ErrorKind> for ErrType<'_> {
    fn from(value: std::io::ErrorKind) -> Self {
        match value {
            std::io::ErrorKind::AlreadyExists => ErrType::FsAlreadyExists,
            std::io::ErrorKind::Deadlock => ErrType::FsDeadlock,
            std::io::ErrorKind::FileTooLarge => ErrType::FsFileTooLarge,
            std::io::ErrorKind::Interrupted => ErrType::FsInterrupted,
            std::io::ErrorKind::InvalidData => ErrType::FsInvalidData,
            std::io::ErrorKind::InvalidFilename => ErrType::FsInvalidFilename,
            std::io::ErrorKind::IsADirectory => ErrType::FsIsADirectory,
            std::io::ErrorKind::NotADirectory => ErrType::FsNotADirectory,
            std::io::ErrorKind::NotFound => ErrType::FsNotFound,
            std::io::ErrorKind::PermissionDenied => ErrType::FsPermissionDenied,
            std::io::ErrorKind::OutOfMemory => ErrType::FsOutOfMemory,
            std::io::ErrorKind::ReadOnlyFilesystem => ErrType::FsReadOnlyFilesystem,
            std::io::ErrorKind::StorageFull => ErrType::FsStorageFull,
            std::io::ErrorKind::TimedOut => ErrType::FsTimedOut,
            other => ErrType::Custom(other.to_smolstr()),
        }
    }
}

impl From<std::num::IntErrorKind> for ErrType<'_> {
    fn from(value: std::num::IntErrorKind) -> Self {
        match value {
            std::num::IntErrorKind::Empty
            | std::num::IntErrorKind::InvalidDigit
            | std::num::IntErrorKind::NegOverflow
            | std::num::IntErrorKind::PosOverflow => ErrType::InvalidInt,
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

    // FS ERRORS
    FsAlreadyExists,
    FsDeadlock,
    FsFileTooLarge,
    FsInterrupted,
    FsInvalidData,
    FsInvalidFilename,
    /// When a file was expected...
    FsIsADirectory,
    /// When a directory was expected...
    FsNotADirectory,
    FsNotFound,
    FsPermissionDenied,
    FsOutOfMemory,
    FsReadOnlyFilesystem,
    FsStorageFull,
    FsTimedOut,

    // NUMBER PARSING ERRORS
    InvalidInt,
    InvalidFloat,

    // BOOL PARSING ERRORS
    InvalidBool,

    /// IndexOutOfBounds(length, index)
    IndexOutOfBounds(usize, i32),

    /// SliceOutOfBounds(length, idx_start, idx_end)
    SliceOutOfBounds(usize, i32, i32),

    NullByteInString,
    CArrayReturnTypeNotSupported,

    // PARSER ERRORS
    UnknownVariable(&'a str),
    UnknownFunction(&'a str),
    UnknownStruct(&'a str),
    UnknownNamespace(&'a str),
    UnknownType(&'a str),
    InvalidStructFieldCount(&'a str, u16, u16),
    /// StructMissingField(struct, field)
    StructMissingField(&'a str, &'a str),
    /// StructUnknownField(struct, field)
    StructUnknownField(&'a str, &'a str),
    /// When an array holds two or more different types
    ArrayWithDiffType,
    NotIndexable(&'a DataType),
    InvalidIndexType(&'a DataType),
    /// CannotPushTypeToArray(elem_type, array_type)
    CannotPushTypeToArray(&'a DataType, &'a DataType),
    CannotInferType(&'a str),
    /// IncorrectArgCount(fn_name, expected, received)
    IncorrectArgCount(&'a str, u16, u16),
    IncorrectArgCountVariable(&'a str, u16, u16, u16),
    /// InvalidType(expected_type, received_type)
    InvalidType(&'a DataType, &'a DataType),
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
    InvalidReturnType(&'a DataType),
    DivisionByZero,
    ModuloByZero,
}

impl From<ErrType<'_>> for SmolStr {
    fn from(value: ErrType) -> Self {
        match value {
            ErrType::Custom(m) => m,
            ErrType::CannotReadImportedFile(filename) => format_args!("Cannot read imported file {color_bright_red}{style_bold}{filename}{color_reset}{style_reset}").to_smolstr(),
            ErrType::InvalidFloat => "Invalid float".into(),
            ErrType::IndexOutOfBounds(length, index) => format_args!("Tried to get index {color_bright_red}{style_bold}{index}{color_reset}{style_reset} but the length is {color_bright_blue}{style_bold}{length}{color_reset}{style_reset}").to_smolstr(),
            ErrType::SliceOutOfBounds(length, idx_start, idx_end) => format_args!("Invalid range {color_bright_red}{style_bold}{idx_start}{color_reset}{style_reset}..{color_bright_red}{style_bold}{idx_end}{color_reset}{style_reset} for collection with length {color_bright_blue}{style_bold}{length}{color_reset}{style_reset}").to_smolstr(),
            ErrType::InvalidBool => "The string could not be parsed into a boolean".into(),
            ErrType::InvalidInt => "Invalid integer".into(),
            ErrType::FsAlreadyExists => "The entity (directory, file, ...) already exists".into(),
            ErrType::FsDeadlock => "This operation would result in a deadlock".into(),
            ErrType::FsFileTooLarge => "The file is too large".into(),
            ErrType::FsInterrupted => "This operation was interrupted".into(),
            ErrType::FsInvalidData => "Malformed or invalid data were encountered".into(),
            ErrType::FsInvalidFilename => "The filename is invalid or too long".into(),
            ErrType::FsIsADirectory => {
                "This operation encountered a directory, when a non-directory was expected".into()
            }
            ErrType::FsNotADirectory => {
                "This operation encountered a non-directory, when a directory was expected".into()
            }
            ErrType::FsNotFound => "The entity (directory, file, ...) was not found".into(),
            ErrType::FsPermissionDenied => {
                "This operation lacked the necessary privileges to complete".into()
            }
            ErrType::FsOutOfMemory => {
                "This operation could not be completed, because it failed to allocate enough memory"
                    .into()
            }
            ErrType::FsReadOnlyFilesystem => {
                "The filesystem or storage medium is read-only, but a write operation was attempted"
                    .into()
            }
            ErrType::FsStorageFull => "Storage is full".into(),
            ErrType::FsTimedOut => "This operation timed out".into(),
            ErrType::UnknownFunction(f) => format_args!(
                "Unknown function {color_bright_blue}{style_bold}{f}{color_reset}{style_reset}"
            )
            .to_smolstr(),
            ErrType::UnknownStruct(f) => format_args!(
                "Unknown struct {color_bright_blue}{style_bold}{f}{color_reset}{style_reset}"
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
            ErrType::UnknownType(t) => format_args!("Unknown type {color_red}{style_bold}{t}{color_reset}{style_reset}").to_smolstr(),
            ErrType::InvalidStructFieldCount(name, expected, received) => format_args!(
                "Struct {color_bright_blue}{style_bold}{name}{color_reset}{style_reset} expects {expected} fields while this has {color_red}{style_bold}{received}{color_reset}{style_reset} fields").to_smolstr(),
            ErrType::StructUnknownField(name, field) => format_args!(
                "Unknown field {color_red}{style_bold}{field}{color_reset}{style_reset} in struct {color_bright_blue}{style_bold}{name}{color_reset}{style_reset}").to_smolstr(),
            ErrType::StructMissingField(name, field) => format_args!(
                "Missing field {color_red}{style_bold}{field}{color_reset}{style_reset} in struct {color_bright_blue}{style_bold}{name}{color_reset}{style_reset}").to_smolstr(),
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
            ErrType::IncorrectArgCount(fn_name, expected, received) => format_args!("Function {color_bright_blue}{style_bold}{fn_name}{color_reset}{style_reset} expects {expected} argument{} but received {received}", if expected != 1 {"s"} else {""} ).to_smolstr(),
            ErrType::IncorrectArgCountVariable(fn_name, expected_min, expected_max, received) => format_args!("Function {color_bright_blue}{style_bold}{fn_name}{color_reset}{style_reset} expects between {expected_min} and {expected_max} arguments but received {received}").to_smolstr(),
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
            ErrType::ModuloByZero => "Modulo by zero. I'm sorry Dave, I'm afraid I can't do that.".into(),
            ErrType::NullByteInString => "String passed to dynamic library function contains an interior null byte".into(),
            ErrType::InvalidReturnType(t) => format_args!("Invalid return type: {color_bright_red}{style_bold}{t}{color_reset}{style_reset}").to_smolstr(),
            ErrType::CArrayReturnTypeNotSupported => "Array return types are not supported: C does not convey the length of a returned array".into(),
        }
    }
}

impl ErrType<'_> {
    pub fn kind(&self) -> &str {
        match self {
            ErrType::Custom(e) => e.as_str(),
            ErrType::FsAlreadyExists => "fs_already_exists",
            ErrType::FsDeadlock => "fs_deadlock",
            ErrType::FsFileTooLarge => "fs_file_too_large",
            ErrType::FsInterrupted => "fs_interrupted",
            ErrType::FsInvalidData => "fs_invalid_data",
            ErrType::FsInvalidFilename => "fs_invalid_filename",
            ErrType::FsIsADirectory => "fs_is_a_directory",
            ErrType::FsNotADirectory => "fs_not_a_directory",
            ErrType::FsNotFound => "fs_not_found",
            ErrType::FsPermissionDenied => "fs_permission_denied",
            ErrType::FsOutOfMemory => "fs_out_of_memory",
            ErrType::FsReadOnlyFilesystem => "fs_read_only_filesystem",
            ErrType::FsStorageFull => "fs_storage_full",
            ErrType::FsTimedOut => "fs_timed_out",
            ErrType::InvalidInt => "invalid_int",
            ErrType::InvalidFloat => "invalid_float",
            ErrType::InvalidBool => "invalid_bool",
            ErrType::IndexOutOfBounds(_, _) => "index_out_of_bounds",
            ErrType::SliceOutOfBounds(_, _, _) => "slice_out_of_bounds",
            ErrType::UnknownVariable(_) => "unknown_variable",
            ErrType::UnknownFunction(_) => "unknown_function",
            ErrType::UnknownStruct(_) => "unknown_struct",
            ErrType::UnknownNamespace(_) => "unknown_namespace",
            ErrType::UnknownType(_) => "unknown_type",
            ErrType::InvalidStructFieldCount(_, _, _) => "invalid_struct_field_count",
            ErrType::StructMissingField(_, _) => "struct_missing_field",
            ErrType::StructUnknownField(_, _) => "struct_unknown_field",
            ErrType::ArrayWithDiffType => "array_with_diff_type",
            ErrType::NotIndexable(_) => "not_indexable",
            ErrType::InvalidIndexType(_) => "invalid_index_type",
            ErrType::CannotPushTypeToArray(_, _) => "cannot_push_type_to_array",
            ErrType::CannotInferType(_) => "cannot_infer_type",
            ErrType::IncorrectArgCount(_, _, _) => "incorrect_arg_count",
            ErrType::IncorrectArgCountVariable(_, _, _, _) => "incorrect_arg_count",
            ErrType::InvalidType(_, _) => "invalid_type",
            ErrType::OpError(_, _, _) => "op_error",
            ErrType::InvalidOp(_, _) => "invalid_op",
            ErrType::InvalidConditionalExpression => "invalid_conditional_expression",
            ErrType::FunctionAlreadyExists(_) => "function_already_exists",
            ErrType::CannotReadImportedFile(_) => "cannot_read_imported_file",
            ErrType::CircularImport(_) => "circular_import",
            ErrType::DuplicateFunctionInImport(_, _) => "duplicate_function_in_import",
            ErrType::IsNotAnIterator(_) => "is_not_an_iterator",
            ErrType::InvalidArgType(_, _) => "invalid_arg_type",
            ErrType::InvalidObjType(_, _) => "invalid_obj_type",
            ErrType::DivisionByZero => "division_by_zero",
            ErrType::ModuloByZero => "modulo_by_zero",
            ErrType::NullByteInString => "null_byte_in_string",
            ErrType::CArrayReturnTypeNotSupported => "c_array_return_type_not_supported",
            ErrType::InvalidReturnType(_) => "invalid_return_type",
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

    #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
    report
        .eprint((src.0.as_str(), Source::from(src.1.as_str())))
        .unwrap();

    #[cfg(any(target_arch = "wasm32", feature = "embed"))]
    report
        .write(
            (src.0.as_str(), Source::from(src.1.as_str())),
            crate::captured_output::CapturedOutputWriter,
        )
        .unwrap();

    #[cfg(debug_assertions)]
    panic!();

    #[cfg(not(any(debug_assertions, target_arch = "wasm32", feature = "embed")))]
    std::process::exit(1);

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen::throw_str("keel_error");

    #[cfg(all(feature = "embed", not(debug_assertions)))]
    panic!();
}

#[cold]
#[inline(never)]
#[cfg(target_arch = "wasm32")]
pub fn wasm_error(msg: &str) -> ! {
    crate::captured_output::print(&format!("KEEL ERROR\n{msg}\n"));
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

    #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
    report.eprint((src.0, Source::from(src.1))).unwrap();

    #[cfg(any(target_arch = "wasm32", feature = "embed"))]
    report
        .write(
            (src.0, Source::from(src.1)),
            crate::captured_output::CapturedOutputWriter,
        )
        .unwrap();

    #[cfg(debug_assertions)]
    panic!();

    #[cfg(not(any(debug_assertions, target_arch = "wasm32", feature = "embed")))]
    std::process::exit(1);

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen::throw_str("keel_error");

    #[cfg(all(feature = "embed", not(debug_assertions)))]
    panic!();
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
            #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
            report.eprint((filename, Source::from(file))).unwrap();
            #[cfg(any(target_arch = "wasm32", feature = "embed"))]
            report
                .write(
                    (filename, Source::from(file)),
                    crate::captured_output::CapturedOutputWriter,
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
            #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
            report.eprint((filename, Source::from(file))).unwrap();
            #[cfg(any(target_arch = "wasm32", feature = "embed"))]
            report
                .write(
                    (filename, Source::from(file)),
                    crate::captured_output::CapturedOutputWriter,
                )
                .unwrap();
        }
        ParseError::UnrecognizedToken { token, expected } => {
            let begin = token.0;
            let end = token.2;

            let expected = expected
                .into_iter()
                .map(|x| {
                    {
                        if x == "\"false\"" || x == "\"true\"" {
                            "Boolean"
                        } else if x == "r#\"[a-zA-Z_][a-zA-Z0-9_]*\"#" {
                            "Variable"
                        } else if x.contains("[^") {
                            "String"
                        } else if x == "r#\"[0-9]*[.][0-9]+\"#" {
                            "Float"
                        } else if x == "r#\"[0-9]+\"#" {
                            "Integer"
                        } else {
                            x.trim_matches('\"')
                        }
                    }
                    .to_smolstr()
                })
                .collect::<Vec<SmolStr>>();

            const STATEMENT_KEYWORDS: [&str; 7] =
                ["let", "if", "while", "for", "loop", "match", "return"];
            let is_statement_set = STATEMENT_KEYWORDS
                .iter()
                .all(|k| expected.iter().any(|n| n == k));

            let expected_tokens = if is_statement_set {
                format_args!(
                    "{color_bright_blue}{style_bold}Statement{style_reset}{color_reset} OR {color_bright_blue}{style_bold}{{{style_reset}{color_reset} OR {color_bright_blue}{style_bold}}}{style_reset}{color_reset}"
                )
            } else {
                format_args!(
                    "{color_bright_blue}{style_bold}{}{color_reset}{style_reset}",
                    expected.join(&format!(
                        "{color_reset}{style_reset} OR {color_bright_blue}{style_bold}"
                    ))
                )
            };

            let report = Report::build(ReportKind::Error, (filename, begin..end))
                .with_message("Unrecognized token")
                .with_label(
                    Label::new((filename, begin..end))
                        .with_message(format_args!("Expected {}", expected_tokens))
                        .with_color(Color::Red),
                )
                .finish();
            #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
            report.eprint((filename, Source::from(file))).unwrap();
            #[cfg(any(target_arch = "wasm32", feature = "embed"))]
            report
                .write(
                    (filename, Source::from(file)),
                    crate::captured_output::CapturedOutputWriter,
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
