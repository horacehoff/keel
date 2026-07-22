use crate::compiler::compiler_data::{InstrSrc, Source};
use crate::compiler::expr::Span;
use crate::{compiler::type_system::DataType, instr::Instr};
use ariadne::FnCache;
use ariadne::{Color, Label, Report, ReportKind};
use smol_strc::{SmolStr, ToSmolStr};
use std::cell::Cell;
use std::cell::RefCell;
use std::hint::unreachable_unchecked;
use std::ops::Range;

/// A structured compile/runtime error, produced instead of printing + exiting
/// whenever a diagnostic sink is installed (see `collect_diagnostic`).
///
/// `span` is a byte range into the source named by `filename`.
/// `code` is a stable, machine-readable error identifier, e.g.
/// `index_out_of_bounds`, `unexpected_eof`, or `compile_error`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub filename: String,
    pub span: Range<usize>,
    pub message: String,
    pub code: String,
}

/// Panic payload used to unwind out of compilation/execution once a
/// diagnostic has been recorded in the active sink.
struct FatalError;

thread_local! {
    static DIAGNOSTIC_SINK: RefCell<Option<Diagnostic>> = const { RefCell::new(None) };
    static SINK_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static SUPPRESS_PANIC_HOOK: Cell<bool> = const { Cell::new(false) };
}

/// Whether errors should be recorded as a `Diagnostic` instead of being
/// printed and aborting the process. This is what keeps the CLI behavior
/// byte-for-byte identical: without an active sink, every error path behaves
/// exactly as before.
#[inline]
pub fn diagnostics_enabled() -> bool {
    SINK_ACTIVE.with(Cell::get)
}

/// Records `Diagnostic` data in the active sink and unwinds to the enclosing
/// `collect_diagnostic` call. Must only be called when `diagnostics_enabled()`.
///
/// The three error funnels are fatal-on-first-error (they return `!`), so a
/// single slot is enough: the first error to fire is the one reported.
#[cold]
#[inline(never)]
pub fn emit_diagnostic(filename: &str, span: Range<usize>, message: String, code: &str) -> ! {
    DIAGNOSTIC_SINK.with(|sink| {
        *sink.borrow_mut() = Some(Diagnostic {
            filename: filename.to_owned(),
            span,
            message,
            code: code.to_owned(),
        });
    });
    std::panic::panic_any(FatalError);
}

/// Installs a panic hook (once) that stays silent only for the internal
/// `FatalError` unwind used to carry a diagnostic out of a `collect_diagnostic`
/// call. Every other panic — including genuine bugs that fire while a sink is
/// active — is forwarded to the previous hook so it stays observable.
fn install_silencing_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let suppress =
                SUPPRESS_PANIC_HOOK.with(Cell::get) && info.payload().is::<FatalError>();
            if !suppress {
                previous(info);
            }
        }));
    });
}

/// Runs `f` with a diagnostic sink installed.
///
/// An error that would normally print a report and exit/panic instead records a
/// [`Diagnostic`] and unwinds back here. Nested calls are supported (the
/// previous state is restored).
///
/// Requires an unwinding panic strategy; under `panic = "abort"` the unwind
/// cannot be caught (the CLI never calls this, so its behavior is unchanged).
///
/// # Errors
///
/// Returns the [`Diagnostic`] recorded by the first error funnel that fired
/// while `f` was running (parser, compiler or runtime).
pub fn collect_diagnostic<R>(f: impl FnOnce() -> R) -> Result<R, Diagnostic> {
    install_silencing_panic_hook();
    let previous_active = SINK_ACTIVE.replace(true);
    let previous_sink = DIAGNOSTIC_SINK.with(|sink| sink.borrow_mut().take());
    let previous_suppress = SUPPRESS_PANIC_HOOK.replace(true);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    let recorded = DIAGNOSTIC_SINK.with(|sink| sink.borrow_mut().take());
    DIAGNOSTIC_SINK.with(|sink| *sink.borrow_mut() = previous_sink);
    SINK_ACTIVE.set(previous_active);
    SUPPRESS_PANIC_HOOK.set(previous_suppress);
    match outcome {
        Ok(value) => Ok(value),
        Err(payload) => {
            if payload.is::<FatalError>() {
                Err(recorded.unwrap_or_else(|| Diagnostic {
                    filename: String::new(),
                    span: 0..0,
                    message: String::from("unknown error"),
                    code: String::from("unknown"),
                }))
            } else {
                std::panic::resume_unwind(payload);
            }
        }
    }
}

/// Removes ANSI escape sequences (colors/bold) from an error message so that
/// structured diagnostics carry plain text.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1B' {
            // Skip a CSI sequence: ESC '[' ... final byte in '@'..='~'
            if chars.next() == Some('[') {
                for f in chars.by_ref() {
                    if ('@'..='~').contains(&f) {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

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

pub struct ErrorCtx {
    pub instr_src: Vec<InstrSrc>,
    pub sources: Vec<Source>,
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
            _ => unsafe { unreachable_unchecked() },
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
            _ => unsafe { unreachable_unchecked() },
        }
    }
}

/// Error types, largely borrowed from Rust
pub enum ErrType<'a> {
    Custom(&'a str),
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
    NullByteInString,
    CArrayReturnTypeNotSupported,
    InvalidReturnType(&'a DataType),
    DivisionByZero,
    ModuloByZero,
}

impl From<ErrType<'_>> for SmolStr {
    fn from(value: ErrType) -> Self {
        match value {
            ErrType::Custom(m) => m.to_smolstr(),
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
            ErrType::FsIsADirectory => "This operation encountered a directory, when a non-directory was expected".into(),
            ErrType::FsNotADirectory => "This operation encountered a non-directory, when a directory was expected".into(),
            ErrType::FsNotFound => "The entity (directory, file, ...) was not found".into(),
            ErrType::FsPermissionDenied => "This operation lacked the necessary privileges to complete".into(),
            ErrType::FsOutOfMemory => "This operation could not be completed, because it failed to allocate enough memory".into(),
            ErrType::FsReadOnlyFilesystem => "The filesystem or storage medium is read-only, but a write operation was attempted".into(),
            ErrType::FsStorageFull => "Storage is full".into(),
            ErrType::FsTimedOut => "This operation timed out".into(),
            ErrType::DivisionByZero => "Division by zero. I'm sorry Dave, I'm afraid I can't do that.".into(),
            ErrType::ModuloByZero => "Modulo by zero. I'm sorry Dave, I'm afraid I can't do that.".into(),
            ErrType::NullByteInString => "String passed to dynamic library function contains an interior null byte".into(),
            ErrType::InvalidReturnType(t) => format_args!("Invalid return type: {RED}{BOLD}{t}{RESET}").to_smolstr(),
            ErrType::CArrayReturnTypeNotSupported => "Array return types are not supported: C does not convey the length of a returned array".into(),
            ErrType::UnknownMapKey(key) => format_args!("Unknown key {RED}{BOLD}{key}{RESET}").to_smolstr(),
        }
    }
}

impl ErrType<'_> {
    pub const fn kind(&self) -> &str {
        match self {
            ErrType::Custom(e) => e,
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
            ErrType::DivisionByZero => "division_by_zero",
            ErrType::ModuloByZero => "modulo_by_zero",
            ErrType::NullByteInString => "null_byte_in_string",
            ErrType::CArrayReturnTypeNotSupported => "c_array_return_type_not_supported",
            ErrType::InvalidReturnType(_) => "invalid_return_type",
            ErrType::UnknownMapKey(_) => "unknown_map_key",
        }
    }
}

#[cold]
#[inline(never)]
pub fn throw_error(ctx: &ErrorCtx, instr: Instr, t: ErrType) -> ! {
    let InstrSrc {
        instr: _,
        span: Span { start, end },
        file_id,
    } = ctx
        .instr_src
        .iter()
        .find(|s| s.instr == instr)
        .unwrap_or(&InstrSrc {
            instr: Instr::Halt(1),
            span: Span { start: 0, end: 0 },
            file_id: 0,
        });
    let src = &ctx.sources[*file_id as usize];
    if diagnostics_enabled() {
        let code = t.kind().to_owned();
        let err_message: SmolStr = t.into();
        emit_diagnostic(
            src.filename.as_str(),
            (*start as usize)..(*end as usize),
            strip_ansi(err_message.as_str()),
            &code,
        );
    }
    let err_message: SmolStr = t.into();
    eprintln!("{RED}KEEL ERROR{RESET}");
    let report = Report::build(
        ReportKind::Error,
        (src.filename.as_str(), (*start as usize)..(*end as usize)),
    )
    .with_label(
        Label::new((src.filename.as_str(), (*start as usize)..(*end as usize)))
            .with_message(err_message.as_str())
            .with_color(Color::Red),
    )
    .finish();

    #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
    report
        .eprint((
            src.filename.as_str(),
            ariadne::Source::from(src.contents.as_str()),
        ))
        .unwrap();

    #[cfg(any(target_arch = "wasm32", feature = "embed"))]
    report
        .write(
            (
                src.filename.as_str(),
                ariadne::Source::from(src.contents.as_str()),
            ),
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
pub fn throw_compiler_error<'a>(
    report: &dyn Fn() -> Report<'a, (&'a str, core::ops::Range<usize>)>,
    sources: &'a [Source],
    file_idx: u16,
    span: Span,
    message: &str,
) -> ! {
    if diagnostics_enabled() {
        emit_diagnostic(
            sources[file_idx as usize].filename.as_str(),
            (span.start as usize)..(span.end as usize),
            strip_ansi(message),
            "compile_error",
        );
    }
    let report = report();

    #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
    report
        .eprint(
            FnCache::new((move |id| Err(format!("Failed to fetch source {id}"))) as fn(&_) -> _)
                .with_sources(
                    sources
                        .iter()
                        .map(|Source { filename, contents }| {
                            (filename.as_str(), ariadne::Source::from(contents.as_str()))
                        })
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
                        .map(|src| {
                            (
                                src.filename.as_str(),
                                ariadne::Source::from(src.contents.as_str()),
                            )
                        })
                        .collect(),
                ),
            crate::captured_output::CapturedOutputWriter,
        )
        .unwrap();

    crash();
}

#[cold]
#[inline(never)]
pub fn crash() -> ! {
    #[cfg(debug_assertions)]
    panic!();

    #[cfg(not(any(debug_assertions, target_arch = "wasm32", feature = "embed")))]
    std::process::exit(1);

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen::throw_str("keel_error");

    #[cfg(all(feature = "embed", not(debug_assertions)))]
    panic!();
}
