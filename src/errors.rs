use crate::ast::Span;
use crate::compiler::Source;
use crate::compiler::program::InstrSrc;
use ariadne::FnCache;
use ariadne::{Color, Report, ReportKind};
use bumpalo::Bump;
use core::num::NonZeroU32;
use hybrid_format::hformat;

pub const ARIADNE_ERROR_KIND: ReportKind<'static> = ReportKind::Custom("[ERROR]", Color::Red);
pub const ERROR_PREFIX: &str = hformat!("{RED}[ERROR]:{RESET}");

pub const BLUE: &str = "\x1B[94m";
pub const RED: &str = "\x1B[31m";
pub const BOLD: &str = "\x1B[1m";
pub const GREEN: &str = "\x1B[32m";
pub const RESET: &str = "\x1B[0m\x1B[39m";

#[derive(Clone, Copy)]
pub struct SourceLocation {
    pub span: Span,
    pub file_idx: u16,
}

pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Copy)]
pub enum LabelRole {
    Primary,
    Secondary,
}

pub struct Label<'arena> {
    pub loc: SourceLocation,
    pub msg: &'arena str,
    pub role: LabelRole,
}

// Borrowed from Rust
pub enum SuggestionApplicability {
    /// Definitely what the user intended
    MachineApplicable,
    /// Maybe what the user intended, but not sure
    MaybeIncorrect,
    /// The suggestion contains placeholders and can't be applied automatically
    HasPlaceholders,
}

pub struct Suggestion<'arena> {
    pub loc: SourceLocation,
    pub msg: &'arena str,
    pub replacement: &'arena str,
    pub applicability: SuggestionApplicability,
}

#[repr(transparent)]
pub struct ErrDiagnosticId(NonZeroU32);

pub struct Diagnostic<'arena> {
    pub code: ErrorCode,
    pub severity: Severity,
    pub loc: SourceLocation,
    pub title: Option<&'arena str>,
    pub labels: Vec<Label<'arena>>,
    pub notes: Vec<&'arena str>,
    pub help: Vec<&'arena str>,
    pub suggestions: Vec<Suggestion<'arena>>,
    pub parent: Option<ErrDiagnosticId>,
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_diagnostic(diagnostic: &Diagnostic, sources: &[Source]) {
    let mut report = Report::build(
        match diagnostic.severity {
            Severity::Error => ReportKind::Error,
            Severity::Warning => ReportKind::Warning,
        },
        (sources[diagnostic.loc.file_idx as usize].filename, diagnostic.loc.span.into()),
    )
    .with_code(format_args!("E{:04}", diagnostic.code as u16))
    .with_message(diagnostic.title.unwrap_or(diagnostic.code.title()));

    for label in &diagnostic.labels {
        report = report.with_label(
            ariadne::Label::new((
                sources[label.loc.file_idx as usize].filename,
                label.loc.span.into(),
            ))
            .with_message(label.msg)
            .with_color(Color::Red)
            .with_priority(1),
        );
    }
    for note in &diagnostic.notes {
        report = report.with_note(note);
    }
    for help in &diagnostic.help {
        report = report.with_help(help);
    }
    for suggestion in &diagnostic.suggestions {
        report = report.with_help(format!("{}: `{}`", suggestion.msg, suggestion.replacement));
    }

    print_error_report(&report.finish(), sources);
}

pub struct ProgramDiagnostics<'arena> {
    pub diagnostics: Vec<Diagnostic<'arena>>,
    pub error_count: u32,
    pub bump: &'arena Bump,
    current: usize,
    current_file: u16,
}

impl<'arena> ProgramDiagnostics<'arena> {
    #[inline(never)]
    #[cold]
    #[optimize(size)]
    pub fn emit_all(&self, sources: &[Source]) {
        for diagnostic in &self.diagnostics {
            error_diagnostic(diagnostic, sources);
        }
    }
    #[must_use]
    #[inline(always)]
    pub const fn new(bump: &'arena Bump) -> Self {
        Self { diagnostics: Vec::new(), error_count: 0, bump, current: 0, current_file: 0 }
    }
    #[inline(never)]
    #[cold]
    #[optimize(size)]
    pub fn error(&mut self, code: ErrorCode, span: Span, file_idx: u16) -> &mut Self {
        self.error_count += 1;
        self.current = self.diagnostics.len();
        self.current_file = file_idx;
        self.diagnostics.push(Diagnostic {
            code,
            severity: Severity::Error,
            loc: SourceLocation { span, file_idx },
            title: None,
            labels: Vec::new(),
            notes: Vec::new(),
            help: Vec::new(),
            suggestions: Vec::new(),
            parent: None,
        });
        self
    }
    #[inline(always)]
    pub const fn reopen(&mut self, id: ErrDiagnosticId, file_idx: u16) -> &mut Self {
        self.current = id.0.get() as usize - 1;
        self.current_file = file_idx;
        self
    }
    #[inline(always)]
    fn text(&self, msg: core::fmt::Arguments) -> &'arena str {
        if let Some(txt) = msg.as_str() {
            return txt;
        }
        let mut text = bumpalo::collections::String::new_in(self.bump);
        unsafe { core::fmt::Write::write_fmt(&mut text, msg).unwrap_unchecked() };
        text.into_bump_str()
    }
    #[inline(never)]
    #[cold]
    #[optimize(size)]
    fn label(&mut self, role: LabelRole, span: Span, msg: core::fmt::Arguments) -> &mut Self {
        let loc = SourceLocation { span, file_idx: self.current_file };
        let msg = self.text(msg);
        self.diagnostics[self.current].labels.push(Label { loc, msg, role });
        self
    }
    #[inline(always)]
    pub fn primary(&mut self, span: Span, msg: core::fmt::Arguments) -> &mut Self {
        self.label(LabelRole::Primary, span, msg)
    }
    #[inline(always)]
    pub fn secondary(&mut self, span: Span, msg: core::fmt::Arguments) -> &mut Self {
        self.label(LabelRole::Secondary, span, msg)
    }
    #[inline(never)]
    #[cold]
    #[optimize(size)]
    pub fn note(&mut self, msg: core::fmt::Arguments) -> &mut Self {
        let note = self.text(msg);
        self.diagnostics[self.current].notes.push(note);
        self
    }
    #[inline(never)]
    #[cold]
    #[optimize(size)]
    pub fn help(&mut self, msg: core::fmt::Arguments) -> &mut Self {
        let note = self.text(msg);
        self.diagnostics[self.current].help.push(note);
        self
    }
    #[inline(never)]
    #[cold]
    #[optimize(size)]
    pub fn suggestion(
        &mut self,
        span: Span,
        msg: core::fmt::Arguments,
        replacement: &str,
        applicability: SuggestionApplicability,
    ) -> &mut Self {
        let suggestion = Suggestion {
            loc: SourceLocation { span, file_idx: self.current_file },
            msg: self.text(msg),
            replacement: self.bump.alloc_str(replacement),
            applicability,
        };
        self.diagnostics[self.current].suggestions.push(suggestion);
        self
    }
    #[inline(always)]
    #[must_use]
    pub const fn finish(&self) -> ErrDiagnosticId {
        ErrDiagnosticId(unsafe { NonZeroU32::new_unchecked(self.current as u32 + 1) })
    }
}

#[derive(Clone, Copy)]
pub enum ErrorCode {
    UnknownVariable = 0,
    ImportedFileNotFound = 1,
    FunctionAlreadyDefined = 2,
    StructAlreadyDefined = 3,
    GlobalAlreadyDefined = 4,
    AmbiguousImport = 5,
    CannotLoadDynamicLibrary = 6,
    CannotFindDynamicLibrarySymbol = 7,
}

impl ErrorCode {
    pub const fn title(self) -> &'static str {
        match self {
            Self::UnknownVariable => "Unknown variable",
            Self::ImportedFileNotFound => "Imported file not found",
            Self::FunctionAlreadyDefined => "Function already defined",
            Self::StructAlreadyDefined => "Struct already defined",
            Self::GlobalAlreadyDefined => "Global variable already defined",
            Self::AmbiguousImport => "Ambiguous import",
            Self::CannotLoadDynamicLibrary => "Cannot load dynamic library",
            Self::CannotFindDynamicLibrarySymbol => "Cannot find symbol in dynamic library",
        }
    }
}

#[macro_export]
macro_rules! error {
    ($diagnostics:expr) => {{
        core::hint::cold_path();
        return Err($diagnostics.finish());
    }};
}

pub struct ErrorCtx<'a> {
    pub instr_src: Vec<InstrSrc>,
    pub sources: Vec<Source<'a>>,
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
            _ => ErrType::FsOther,
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
    FsOther,
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
    DivisionByZero,
    ModuloByZero,
    IntegerNegativeExponent,
    StackOverflow,
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
            ErrType::FsOther => "fs_other",
            ErrType::InvalidInt => "invalid_int",
            ErrType::InvalidFloat => "invalid_float",
            ErrType::InvalidBool => "invalid_bool",
            ErrType::IndexOutOfBounds(_, _) => "index_out_of_bounds",
            ErrType::SliceOutOfBounds(_, _, _) => "slice_out_of_bounds",
            ErrType::DivisionByZero => "division_by_zero",
            ErrType::ModuloByZero => "modulo_by_zero",
            ErrType::NullByteInString => "null_byte_in_string",
            ErrType::UnknownMapKey(_) => "unknown_map_key",
            ErrType::IntegerNegativeExponent => "integer_negative_exponent",
            ErrType::StackOverflow => "stack_overflow",
        }
    }
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn throw_error(ctx: &ErrorCtx, instr_idx: usize, t: &ErrType) -> ! {
    let InstrSrc { span: Span { start, end }, file_id, .. } = ctx
        .instr_src
        .iter()
        .find(|s| s.instr_idx as usize == instr_idx)
        .expect("Please report this bug.");
    let src = &ctx.sources[*file_id as usize];
    let formatted: String;
    let error_message: &str = match t {
        ErrType::Custom(s) => s,
        ErrType::InvalidFloat => "Invalid float",
        ErrType::IndexOutOfBounds(length, index) => {
            formatted = hformat!(
                "Tried to get index {RED}{BOLD}{index}{RESET} but the length is {BLUE}{BOLD}{length}{RESET}",
            );
            formatted.as_str()
        }
        ErrType::SliceOutOfBounds(length, idx_start, idx_end) => {
            formatted = hformat!(
                "Invalid range {RED}{BOLD}{idx_start}{RESET}..{RED}{BOLD}{idx_end}{RESET} for collection with length {BLUE}{BOLD}{length}{RESET}",
            );
            formatted.as_str()
        }
        ErrType::InvalidBool => "The string could not be parsed into a boolean",
        ErrType::InvalidInt => "Invalid integer",
        ErrType::FsAlreadyExists => "The entity (directory, file, ...) already exists",
        ErrType::FsDeadlock => "This operation would result in a deadlock",
        ErrType::FsFileTooLarge => "The file is too large",
        ErrType::FsInterrupted => "This operation was interrupted",
        ErrType::FsInvalidData => "Malformed or invalid data were encountered",
        ErrType::FsInvalidFilename => "The filename is invalid or too long",
        ErrType::FsIsADirectory => {
            "This operation encountered a directory, when a non-directory was expected"
        }
        ErrType::FsNotADirectory => {
            "This operation encountered a non-directory, when a directory was expected"
        }
        ErrType::FsNotFound => "The entity (directory, file, ...) was not found",
        ErrType::FsPermissionDenied => "This operation lacked the necessary privileges to complete",
        ErrType::FsOutOfMemory => {
            "This operation could not be completed, because it failed to allocate enough memory"
        }
        ErrType::FsReadOnlyFilesystem => {
            "The filesystem or storage medium is read-only, but a write operation was attempted"
        }
        ErrType::FsStorageFull => "Storage is full",
        ErrType::FsTimedOut => "This operation timed out",
        ErrType::FsOther => "File system operation failed",
        ErrType::DivisionByZero => "Division by zero. I'm sorry Dave, I'm afraid I can't do that.",
        ErrType::ModuloByZero => "Modulo by zero. I'm sorry Dave, I'm afraid I can't do that.",
        ErrType::NullByteInString => {
            "String passed to dynamic library function contains an interior null byte"
        }
        ErrType::UnknownMapKey(key) => {
            formatted = hformat!("Unknown key {RED}{BOLD}{key}{RESET}");
            formatted.as_str()
        }
        ErrType::IntegerNegativeExponent => "Integers cannot be raised to a negative exponent",
        ErrType::StackOverflow => "Stack overflow. Help!",
    };
    let report =
        Report::build(ARIADNE_ERROR_KIND, (src.filename, (*start as usize)..(*end as usize)))
            .with_label(
                ariadne::Label::new((src.filename, (*start as usize)..(*end as usize)))
                    .with_message(error_message)
                    .with_color(Color::Red),
            )
            .with_message(error_message)
            .finish();

    print_error_report(&report, &ctx.sources);

    crash();
}

#[inline(never)]
#[cold]
#[optimize(size)]
#[cfg(target_arch = "wasm32")]
pub fn wasm_error(msg: &str) -> ! {
    crate::captured_output::print(&hformat!("{ERROR_PREFIX} {msg}\n"));
    wasm_bindgen::throw_str("keel error");
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn print_error_report<'a>(
    report: &Report<'a, (&'a str, core::ops::Range<usize>)>,
    sources: &'a [Source],
) {
    let fn_cache = FnCache::new(
        (move |id: &&str| Err(hformat!("Failed to fetch source {id}"))) as fn(&_) -> _,
    )
    .with_sources(
        sources.iter().map(|src| (src.filename, ariadne::Source::from(src.contents))).collect(),
    );

    #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
    report.eprint(fn_cache).unwrap();

    #[cfg(any(target_arch = "wasm32", feature = "embed"))]
    report.write(fn_cache, crate::captured_output::CapturedOutputWriter).unwrap();
}

#[inline(never)]
#[cold]
#[optimize(size)]
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
