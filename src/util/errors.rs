use crate::compiler::expr::Span;
use crate::{compiler::type_system::DataType, instr::Instr};
use ariadne::FnCache;
use ariadne::{Color, Label, Report, ReportKind, Source};
use smol_strc::{SmolStr, ToSmolStr};
use std::fmt::Arguments;
use std::rc::Rc;

pub const BLUE: &str = "\x1B[94m";
pub fn blue<F: std::fmt::Display>(t: F) -> String {
    format!("{BLUE}{t}{RESET}")
}
pub const RED: &str = "\x1B[31m";
pub fn red<F: std::fmt::Display>(t: F) -> String {
    format!("{RED}{t}{RESET}")
}
pub const BOLD: &str = "\x1B[1m";
pub fn bold<F: std::fmt::Display>(t: F) -> String {
    format!("{BOLD}{t}{RESET}")
}
pub const GREEN: &str = "\x1B[32m";
pub fn green<F: std::fmt::Display>(t: F) -> String {
    format!("{GREEN}{t}{RESET}")
}
pub const RESET: &str = "\x1B[0m\x1B[39m";

#[cold]
#[inline(always)]
pub fn dev_error(file: &str, function: &str, additional_data: Arguments) -> ! {
    unreachable!(
        "\n--------------\n{RED}KEEL COMPILATION ERROR:{RESET}\nFROM FILE: {}\nFROM FUNCTION: {}\nADDITIONAL DATA: {}\n--------------",
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

    UnknownMapKey(&'a str),
    DuplicateMapKey,
    NotLiteralMapKey,

    NullByteInString,
    CArrayReturnTypeNotSupported,

    // PARSER ERRORS
    UnknownVariable(&'a str),
    UnknownFunction(&'a str),
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
            ErrType::CannotReadImportedFile(filename) => format_args!("Cannot read imported file {RED}{BOLD}{filename}{RESET}").to_smolstr(),
            ErrType::InvalidFloat => "Invalid float".into(),
            ErrType::IndexOutOfBounds(length, index) => format_args!("Tried to get index {RED}{BOLD}{index}{RESET} but the length is {BLUE}{BOLD}{length}{RESET}").to_smolstr(),
            ErrType::SliceOutOfBounds(length, idx_start, idx_end) => format_args!("Invalid range {RED}{BOLD}{idx_start}{RESET}..{RED}{BOLD}{idx_end}{RESET} for collection with length {BLUE}{BOLD}{length}{RESET}").to_smolstr(),
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
                "Unknown function {BLUE}{BOLD}{f}{RESET}"
            )
            .to_smolstr(),
            ErrType::UnknownVariable(v) => format_args!(
                "Unknown variable {BLUE}{BOLD}{v}{RESET}"
            )
            .to_smolstr(),
            ErrType::UnknownNamespace(n) => format_args!(
                "Unknown namespace {BLUE}{BOLD}{n}{RESET}"
            )
            .to_smolstr(),
            ErrType::UnknownType(t) => format_args!("Unknown type {RED}{BOLD}{t}{RESET}").to_smolstr(),
            ErrType::InvalidStructFieldCount(name, expected, received) => format_args!(
                "Struct {BLUE}{BOLD}{name}{RESET} expects {expected} fields while this has {RED}{BOLD}{received}{RESET} fields").to_smolstr(),
            ErrType::StructUnknownField(name, field) => format_args!(
                "Unknown field {RED}{BOLD}{field}{RESET} in struct {BLUE}{BOLD}{name}{RESET}").to_smolstr(),
            ErrType::StructMissingField(name, field) => format_args!(
                "Missing field {RED}{BOLD}{field}{RESET} in struct {BLUE}{BOLD}{name}{RESET}").to_smolstr(),
            ErrType::ArrayWithDiffType => "Arrays can only hold a single type".into(),
            ErrType::NotIndexable(t) => format_args!(
                "The type {BLUE}{BOLD}{t}{RESET} cannot be indexed"
            )
            .to_smolstr(),
            ErrType::InvalidIndexType(t) => format_args!(
                "The type {BLUE}{BOLD}{t}{RESET} is not a valid index"
            )
            .to_smolstr(),
            ErrType::CannotPushTypeToArray(elem_t, array_t) => format_args!("Cannot insert {BLUE}{BOLD}{elem_t}{RESET} in {array_t}").to_smolstr(),
            ErrType::CannotInferType(t) => format_args!(
                "Cannot infer the type of {BLUE}{BOLD}{t}{RESET}"
            )
            .to_smolstr(),
            ErrType::IncorrectArgCount(fn_name, expected, received) => format_args!("Function {BLUE}{BOLD}{fn_name}{RESET} expects {expected} argument{} but received {received}", if expected == 1 {""} else {"s"}).to_smolstr(),
            ErrType::IncorrectArgCountVariable(fn_name, expected_min, expected_max, received) => format_args!("Function {BLUE}{BOLD}{fn_name}{RESET} expects between {expected_min} and {expected_max} arguments but received {received}").to_smolstr(),
            ErrType::InvalidType(expected, received) => format_args!("Expected type {expected}, found {BLUE}{BOLD}{received}{RESET}").to_smolstr(),
            ErrType::OpError(l, r, op) => format_args!(
                "Cannot perform operation {BLUE}{BOLD}{l} {RED}{op}{BLUE} {r}{RESET}").to_smolstr(),
            ErrType::InvalidOp(t, op) => format_args!(
                "Operation {RED}{BOLD}{op}{RESET} is not supported for type {BLUE}{BOLD}{t}{RESET}").to_smolstr(),
            ErrType::InvalidConditionalExpression => "Conditional expressions must have an else clause".into(),
            ErrType::FunctionAlreadyExists(fn_name) => format_args!(
                "Function {RED}{BOLD}{fn_name}{RESET} is already defined",
            ).to_smolstr(),
            ErrType::CircularImport(path) => format_args!(
                "Circular import detected: {RED}{BOLD}{path}{RESET} is already being imported"
            ).to_smolstr(),
            ErrType::DuplicateFunctionInImport(fn_name, file_path) => format_args!(
                "Function {BLUE}{BOLD}{fn_name}{RESET} imported from {RED}{BOLD}{file_path}{RESET} is already defined"
            ).to_smolstr(),
            ErrType::IsNotAnIterator(t) => format_args!("The type {RED}{BOLD}{t}{RESET} is not a collection").to_smolstr(),
            ErrType::InvalidArgType(expected, received) => {
                let expected_str = expected
                    .iter()
                    .map(|t| format!("{BLUE}{BOLD}{t}{RESET}"))
                    .collect::<Vec<String>>()
                    .join(" or ");
                format_args!(
                    "Expected {expected_str}, found {RED}{BOLD}{received}{RESET}",
                ).to_smolstr()
            }
            ErrType::InvalidObjType(expected, received) => format_args!(
                "Expected {BLUE}{BOLD}{expected}{RESET}, found {RED}{BOLD}{received}{RESET}",
            ).to_smolstr(),
            ErrType::DivisionByZero => "Division by zero. I'm sorry Dave, I'm afraid I can't do that.".into(),
            ErrType::ModuloByZero => "Modulo by zero. I'm sorry Dave, I'm afraid I can't do that.".into(),
            ErrType::NullByteInString => "String passed to dynamic library function contains an interior null byte".into(),
            ErrType::InvalidReturnType(t) => format_args!("Invalid return type: {RED}{BOLD}{t}{RESET}").to_smolstr(),
            ErrType::CArrayReturnTypeNotSupported => "Array return types are not supported: C does not convey the length of a returned array".into(),
            ErrType::UnknownMapKey(key) => format_args!("Unknown key {RED}{BOLD}{key}{RESET}").to_smolstr(),
            ErrType::DuplicateMapKey => format_args!("Duplicate key in map").to_smolstr(),
            ErrType::NotLiteralMapKey => format_args!("Map keys must be literals").to_smolstr(),
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
            ErrType::IncorrectArgCount(_, _, _)
            | ErrType::IncorrectArgCountVariable(_, _, _, _) => "incorrect_arg_count",
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
            ErrType::UnknownMapKey(_) => "unknown_map_key",
            ErrType::DuplicateMapKey => "duplicate_map_key",
            ErrType::NotLiteralMapKey => "not_literal_map_key",
        }
    }
}

#[cold]
#[inline(never)]
pub fn throw_error(ctx: &ErrorCtx, instr: Instr, t: ErrType) -> ! {
    let (_, Span { start, end }, file_idx) = ctx
        .instr_src
        .iter()
        .find(|(x, _, _)| x == &instr)
        .unwrap_or(&(Instr::Halt(1), Span { start: 0, end: 0 }, 0));
    let src = &ctx.sources[*file_idx as usize];
    let err_message: SmolStr = t.into();
    eprintln!("{RED}KEEL ERROR{RESET}");
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

    crash();
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
pub fn throw_compiler_error_exp<'a, F: Fn() -> Report<'a, (&'a str, core::ops::Range<usize>)>>(
    report: F,
    sources: &'a [(SmolStr, Rc<String>)],
) -> ! {
    let report = report();

    #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
    report
        .eprint(
            FnCache::new((move |id| Err(format!("Failed to fetch source {id}"))) as fn(&_) -> _)
                .with_sources(
                    sources
                        .iter()
                        .map(|(name, contents)| (name.as_str(), Source::from(contents.as_str())))
                        .collect(),
                ),
        )
        .unwrap();

    #[cfg(any(target_arch = "wasm32", feature = "embed"))]
    report
        .write(
            FnCache::new((move |id| Err(format!("Failed to fetch source {id}"))) as fn(&_) -> _)
                .with_sources(
                    sources
                        .iter()
                        .map(|(name, contents)| (name.as_str(), Source::from(contents.as_str())))
                        .collect(),
                ),
            crate::captured_output::CapturedOutputWriter,
        )
        .unwrap();

    crash();
}

#[cold]
#[inline(never)]
fn crash() -> ! {
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
pub fn throw_compiler_error(src: (&str, &str), Span { start, end }: Span, t: ErrType) -> ! {
    let err_message: SmolStr = t.into();
    eprintln!("{RED}KEEL ERROR{RESET}");
    let report = Report::build(ReportKind::Error, (src.0, (start as usize)..(end as usize)))
        .with_label(
            Label::new((src.0, (start as usize)..(end as usize)))
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

    crash();
}
