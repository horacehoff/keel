use crate::ast::Expr;
use crate::ast::Span;
use crate::compiler::DataType;
use crate::compiler::Source;
use crate::compiler::program::Function;
use crate::compiler::program::Struct;
use crate::compiler::program::StructField;
use crate::compiler::scope::Scope;
use crate::compiler::state::State;
use crate::errors::ARIADNE_ERROR_KIND;
use crate::errors::BLUE;
use crate::errors::BOLD;
use crate::errors::Diagnostic;
use crate::errors::ErrorCode;
use crate::errors::GREEN;
use crate::errors::Label;
use crate::errors::LabelRole;
use crate::errors::RED;
use crate::errors::RESET;
use crate::errors::Severity;
use crate::errors::SourceLocation;
use crate::errors::Suggestion;
use crate::errors::SuggestionApplicability;
use crate::errors::crash;
use crate::errors::error_diagnostic;
use crate::errors::print_error_report;
use ariadne::Color;
use ariadne::Report;
use core::fmt::Display;
use core::ops::Range;
use hybrid_format::hformat;

struct ReportLabel<'a> {
    message: &'a dyn Display,
    color: Color,
    span: Span,
    file_idx: u16,
    priority: i32,
}

impl<'a> ReportLabel<'a> {
    #[inline(always)]
    const fn new(message: &'a dyn Display, color: Color, span: Span, file_idx: u16) -> Self {
        Self { message, color, span, file_idx, priority: 0 }
    }

    #[inline(always)]
    fn location<'s>(&self, sources: &[Source<'s>]) -> (&'s str, Range<usize>) {
        (sources[self.file_idx as usize].filename, self.span.into())
    }
}

#[inline(never)]
#[cold]
#[optimize(size)]
fn compiler_error(
    title: &dyn Display, // the extra runtime cost is fine here since it quits anyway
    location: (Span, u16),
    labels: &[ReportLabel],
    note: Option<&dyn Display>,
    help: Option<&dyn Display>,
    sources: &[Source],
) -> ! {
    let mut report = Report::build(
        ARIADNE_ERROR_KIND,
        (sources[location.1 as usize].filename, location.0.into()),
    )
    .with_message(title);
    for label in labels {
        report = report.with_label(
            ariadne::Label::new(label.location(sources))
                .with_message(label.message)
                .with_color(label.color)
                .with_priority(label.priority),
        );
    }
    if let Some(note) = note {
        report = report.with_note(note);
    }
    if let Some(help) = help {
        report = report.with_help(help);
    }
    print_error_report(&report.finish(), sources);
    crash()
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub(super) fn error_array_diff_types(
    file_idx: u16,
    sources: &[Source],
    array_span: Span,
    array_elem_type: &DataType,
    failing_elem_span: Span,
    failing_elem_type: &DataType,
) -> ! {
    compiler_error(
        &"Invalid array types",
        (array_span, file_idx),
        &[
            ReportLabel::new(
                &format_args!("This expression is of type {BLUE}{array_elem_type}{RESET}"),
                Color::Blue,
                array_span,
                file_idx,
            ),
            ReportLabel::new(
                &format_args!("This expression is of type {RED}{failing_elem_type}{RESET}"),
                Color::Red,
                failing_elem_span,
                file_idx,
            ),
        ],
        Some(&"Arrays are homogeneous and can only hold elements of a single type"),
        None,
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub(super) fn error_invalid_type(
    expected_type: &DataType,
    perceived_type: &DataType,
    span: Span,
    help: Option<core::fmt::Arguments>,
    note: Option<core::fmt::Arguments>,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    compiler_error(
        &"Invalid type",
        (span, file_idx),
        &[ReportLabel::new(
            &format_args!(
                "Expected {BLUE}{expected_type}{RESET}, but this expression's type is {RED}{perceived_type}{RESET}"
            ),
            Color::Red,
            span,
            file_idx,
        )],
        note.as_ref().map(|note| note as &dyn Display),
        help.as_ref().map(|help| help as &dyn Display),
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_invalid_index_type(t: &DataType, span: Span, file_idx: u16, sources: &[Source]) {
    error_invalid_type(
        &DataType::Int,
        t,
        span,
        Some(format_args!("Try using the {BLUE}int(){RESET} function")),
        Some(format_args!("The {BLUE}{}{RESET} type is the only valid index type", DataType::Int)),
        file_idx,
        sources,
    );
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_expected_function(t: &DataType, span: Span, file_idx: u16, sources: &[Source]) -> ! {
    error_invalid_type(&DataType::Fn(0), t, span, None, None, file_idx, sources);
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_invalid_c_type(t: &DataType, span: Span, file_idx: u16, sources: &[Source]) -> ! {
    compiler_error(
        &"Cannot pass type through FFI",
        (span, file_idx),
        &[ReportLabel::new(
            &format_args!("{RED}{t}{RESET} cannot be passed through FFI"),
            Color::Red,
            span,
            file_idx,
        )],
        Some(&format_args!(
            "The following types can be passed through FFI:\n- {}\n- {}\n- {}\n- {}\n- {}\n- {}",
            DataType::Int,
            DataType::Float,
            DataType::String,
            DataType::Bool,
            DataType::Array(None),
            DataType::Struct(0),
        )),
        None,
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_function_needs_args_typed(
    fn_name: &str,
    span: Span,
    fn_decl_span: (Span, u16),
    file_idx: u16,
    sources: &[Source],
) -> ! {
    compiler_error(
        &"Cannot use function as a value",
        (span, file_idx),
        &[
            ReportLabel::new(
                &"Function is defined here",
                Color::Blue,
                fn_decl_span.0,
                fn_decl_span.1,
            ),
            ReportLabel::new(
                &format_args!("{BLUE}{fn_name}{RESET} has an untyped argument"),
                Color::Red,
                span,
                file_idx,
            ),
        ],
        Some(&"Functions referenced by name need to have all of their arguments typed"),
        None,
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_division_by_zero(modulo: bool, span: Span, file_idx: u16, sources: &[Source]) -> ! {
    compiler_error(
        &format_args!("{} by zero", if modulo { "Modulo" } else { "Division" }),
        (span, file_idx),
        &[ReportLabel::new(
            &format_args!("This performs {} by zero!", if modulo { "modulo" } else { "division" }),
            Color::Red,
            span,
            file_idx,
        )],
        None,
        None,
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_cannot_push_type_to_array(
    array_type: &DataType,
    elem_type: &DataType,
    array_span: Span,
    span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    compiler_error(
        &format_args!("Cannot insert {RED}{elem_type}{RESET} in {RED}{array_type}{RESET}"),
        (span, file_idx),
        &[
            ReportLabel::new(
                &format_args!("This array's type is {BLUE}{array_type}{RESET}"),
                Color::Blue,
                array_span,
                file_idx,
            ),
            ReportLabel::new(
                &format_args!("But this expression's type is {RED}{elem_type}{RESET}"),
                Color::Red,
                span,
                file_idx,
            ),
        ],
        None,
        None,
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_type_not_indexable(
    t: &DataType,
    span: Span,
    iterator_error: bool,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    let msg = if iterator_error { "iterated on" } else { "indexed" };
    compiler_error(
        &"Invalid type",
        (span, file_idx),
        &[ReportLabel::new(
            &format_args!("This expression's type is {RED}{t}{RESET}. This type cannot be {msg}."),
            Color::Red,
            span,
            file_idx,
        )],
        Some(&format_args!(
            "The following types can be {msg}:\n- {BLUE}{}{RESET}\n- {BLUE}{}{RESET}",
            DataType::Array(None),
            DataType::String
        )),
        None,
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_ambiguous_import(span: Span, file_idx: u16, sources: &[Source], name: &str) -> ! {
    compiler_error(
        &"Ambiguous import",
        (span, file_idx),
        &[ReportLabel::new(
            &format_args!("Both `{name}.kl` and `{name}/lib.kl` exist for this import."),
            Color::Red,
            span,
            file_idx,
        )],
        None,
        None,
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_cannot_load_dynlib(span: Span, file_idx: u16, sources: &[Source]) -> ! {
    compiler_error(
        &"Cannot load dynamic library",
        (span, file_idx),
        &[ReportLabel::new(
            &"This dynamic library cannot be found/loaded.",
            Color::Red,
            span,
            file_idx,
        )],
        None,
        None,
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_cannot_find_dynlib_symbol(
    symbol: &str,
    symbol_span: Span,
    dynlib_span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    compiler_error(
        &"Cannot find symbol in dynamic library",
        (dynlib_span, file_idx),
        &[
            ReportLabel::new(
                &"This dynamic library is loaded here.",
                Color::Blue,
                dynlib_span,
                file_idx,
            ),
            ReportLabel::new(
                &format_args!("Cannot find symbol {RED}{symbol}{RESET} in this dynamic library"),
                Color::Red,
                symbol_span,
                file_idx,
            ),
        ],
        None,
        None,
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_map_diff_types(
    file_idx: u16,
    sources: &[Source],
    map_span: Span,
    map_elem_type: &DataType,
    failing_elem_span: Span,
    failing_elem_type: &DataType,
) -> ! {
    compiler_error(
        &"Invalid map types",
        (map_span, file_idx),
        &[
            ReportLabel::new(
                &format_args!("This expression is of type {BLUE}{map_elem_type}{RESET}"),
                Color::Blue,
                map_span,
                file_idx,
            ),
            ReportLabel::new(
                &format_args!("This expression is of type {RED}{failing_elem_type}{RESET}"),
                Color::Red,
                failing_elem_span,
                file_idx,
            ),
        ],
        Some(&"Maps are homogeneous and can only hold one key-value type"),
        None,
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_unknown_struct(
    struct_name: &str,
    struct_span: Span,
    sources: &[Source],
    file_idx: u16,
) -> ! {
    compiler_error(
        &"Unknown struct",
        (struct_span, file_idx),
        &[ReportLabel::new(
            &format_args!("Unknown struct {RED}{struct_name}{RESET}"),
            Color::Red,
            struct_span,
            file_idx,
        )],
        None,
        None,
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_struct_no_such_field(
    file_idx: u16,
    struct_name: &str,
    struct_span: Span,
    struct_field_span: Span,
    struct_field_name: &str,
    sources: &[Source],
) -> ! {
    compiler_error(
        &"Unknown struct field",
        (struct_field_span, file_idx),
        &[
            ReportLabel::new(&"Struct defined here", Color::Blue, struct_span, file_idx),
            ReportLabel::new(
                &format_args!(
                    "There is no field {RED}{struct_field_name}{RESET} in {BLUE}{struct_name}{RESET}"
                ),
                Color::Red,
                struct_field_span,
                file_idx,
            ),
        ],
        None,
        None,
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_struct_missing_fields(
    file_idx: u16,
    struct_span: Span,
    struct_literal_span: Span,
    sources: &[Source],
    missing_fields: &[&str],
) -> ! {
    compiler_error(
        &"Missing struct fields",
        (struct_literal_span, file_idx),
        &[
            ReportLabel::new(&"Struct defined here", Color::Blue, struct_span, file_idx),
            ReportLabel::new(
                &format_args!(
                    "This is missing field{} {}",
                    if missing_fields.len() > 1 { "s" } else { "" },
                    missing_fields
                        .iter()
                        .map(|field| hformat!("{BLUE}{field}{RESET}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Color::Red,
                struct_literal_span,
                file_idx,
            ),
        ],
        None,
        None,
        sources,
    )
}

pub fn check_args_length(
    args: &[Expr],
    expected_args_len: usize,
    fn_name: &str,
    span: Span,
    sources: &[Source],
    file_idx: u16,
) {
    if args.len() != expected_args_len {
        compiler_error(
            &"Invalid argument count",
            (span, file_idx),
            &[ReportLabel::new(
                &format_args!(
                    "Function {BLUE}{fn_name}{RESET} expects {BOLD}{expected_args_len}{RESET} arguments but {BOLD}{}{RESET} arguments were supplied",
                    args.len()
                ),
                Color::Red,
                span,
                file_idx,
            )],
            None,
            None,
            sources,
        );
    }
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_invalid_obj_type(
    expected_type: &[DataType],
    perceived_type: &DataType,
    fn_name: &str,
    span: Span,
    sources: &[Source],
    file_idx: u16,
) -> ! {
    compiler_error(
        &"Invalid type",
        (span, file_idx),
        &[ReportLabel::new(
            &format_args!(
                "Function {BLUE}{fn_name}{RESET} expects this expression's type to be {BLUE}{}{RESET} but here its type is {RED}{perceived_type}{RESET}",
                expected_type
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(hformat!("{RESET} or {BLUE}")),
            ),
            Color::Red,
            span,
            file_idx,
        )],
        None,
        None,
        sources,
    )
}

pub fn check_args_user_fn(
    args: &[Expr],
    expected_args_len: usize,
    fn_name: &str,
    file_idx: u16,
    span: Span,
    fn_decl_span: (Span, u16),
    state: &State,
    args_indexes: &[Span],
) {
    let args_len = args.len();
    if args_len != expected_args_len {
        let has_excess_args = args_len > expected_args_len;
        let excess_args_span: Span = if has_excess_args {
            (args_indexes[expected_args_len].start, args_indexes[args_indexes.len() - 1].end).into()
        } else {
            span
        };
        compiler_error(
            &"Invalid argument count",
            (span, file_idx),
            &[
                ReportLabel::new(
                    &format_args!("The function {BLUE}{fn_name}{RESET} is defined here"),
                    Color::Blue,
                    fn_decl_span.0,
                    fn_decl_span.1,
                ),
                ReportLabel::new(
                    &format_args!(
                        "Function {BLUE}{fn_name}{RESET} expects {BOLD}{expected_args_len}{RESET} arguments but {BOLD}{args_len}{RESET} arguments were supplied"
                    ),
                    Color::Red,
                    span,
                    file_idx,
                ),
                ReportLabel {
                    priority: -1,
                    ..ReportLabel::new(
                        &format_args!(
                            "{BLUE}Help{RESET}: Remove {}",
                            if args_len == expected_args_len + 1 {
                                "this argument"
                            } else {
                                "those arguments"
                            }
                        ),
                        Color::Blue,
                        excess_args_span,
                        file_idx,
                    )
                },
            ][..2 + usize::from(has_excess_args)],
            None,
            None,
            state.sources,
        );
    }
}

pub fn check_args_range(
    args: &[Expr],
    min_args_len: usize,
    max_args_len: usize,
    fn_name: &str,
    args_indexes: &[Span],
    file_idx: u16,
    sources: &[Source],
    span: Span,
) {
    if args.len() < min_args_len || args.len() > max_args_len {
        let has_excess_args = args.len() > max_args_len;
        let excess_args_span: Span = if has_excess_args {
            (args_indexes[max_args_len].start, args_indexes[args_indexes.len() - 1].end).into()
        } else {
            span
        };
        compiler_error(
            &"Invalid argument count",
            (span, file_idx),
            &[
                ReportLabel::new(
                    &format_args!(
                        "Function {BLUE}{fn_name}{RESET} expects at least {BLUE}{min_args_len}{RESET} and at most {BLUE}{max_args_len}{RESET} arguments but {RED}{}{RESET} were supplied",
                        args.len()
                    ),
                    Color::Red,
                    span,
                    file_idx,
                ),
                ReportLabel {
                    priority: -1,
                    ..ReportLabel::new(
                        &format_args!(
                            "{BLUE}Help{RESET}: Remove {}",
                            if args.len() == max_args_len + 1 {
                                "this argument"
                            } else {
                                "those arguments"
                            }
                        ),
                        Color::Blue,
                        excess_args_span,
                        file_idx,
                    )
                },
            ][..=usize::from(has_excess_args)],
            None,
            None,
            sources,
        );
    }
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_struct_unknown_field(
    file_idx: u16,
    field_span: Span,
    field: &str,
    struct_name: &str,
    fields: &[StructField],
    sources: &[Source],
) -> ! {
    let help = if let Some(similar_field) =
        find_closest_str(field, fields.iter().map(|field| field.name))
    {
        hformat!("A field with a similar name exists: {BLUE}{similar_field}{RESET}")
    } else {
        hformat!(
            "The available fields are: {}",
            fields
                .iter()
                .map(|field| hformat!("{BLUE}{}{RESET}", field.name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    compiler_error(
        &"Unknown field",
        (field_span, file_idx),
        &[ReportLabel::new(
            &format_args!(
                "The field {RED}{field}{RESET} isn't defined in struct {BLUE}{struct_name}{RESET}"
            ),
            Color::Red,
            field_span,
            file_idx,
        )],
        None,
        Some(&help),
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_struct_field_invalid_type(
    file_idx: u16,
    struct_name: &str,
    struct_field_span: Span,
    struct_field_name: &str,
    struct_field_type: &DataType,
    value_span: Span,
    value_type: &DataType,
    sources: &[Source],
) -> ! {
    let conversion_fn = if struct_field_type == &DataType::Int
        && (value_type == &DataType::Float || value_type == &DataType::String)
    {
        Some("int")
    } else if struct_field_type == &DataType::Float
        && (value_type == &DataType::Int || value_type == &DataType::String)
    {
        Some("float")
    } else if struct_field_type == &DataType::Bool && value_type == &DataType::String {
        Some("bool")
    } else if struct_field_type == &DataType::String {
        Some("str")
    } else {
        None
    };
    let help = conversion_fn.map(|f| hformat!("Try using the {BLUE}{f}(){RESET} function"));
    compiler_error(
        &"Incompatible types",
        (struct_field_span, file_idx),
        &[
            ReportLabel::new(
                &format_args!(
                    "Field {BLUE}{struct_field_name}{RESET} in struct {BLUE}{struct_name}{RESET} expects type {BLUE}{struct_field_type}{RESET}"
                ),
                Color::Blue,
                struct_field_span,
                file_idx,
            ),
            ReportLabel::new(
                &format_args!("This expression is of type {RED}{value_type}{RESET}"),
                Color::Red,
                value_span,
                file_idx,
            ),
        ],
        None,
        help.as_ref().map(|help| help as &dyn Display),
        sources,
    )
}

#[optimize(size)]
#[inline(never)]
fn find_closest_str<'a>(name: &'a str, list: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut best: Option<(&str, usize)> = None;
    for candidate in list {
        let dist = levenshtein(name, candidate);
        if dist <= (name.len().max(candidate.len()) / 3).max(1)
            && best.is_none_or(|(_, d)| dist < d)
        {
            best = Some((candidate, dist));
        }
    }
    best.map(|(s, _)| s)
}

#[optimize(size)]
#[inline(always)]
fn levenshtein(a: &str, b: &str) -> usize {
    let m = a.len();
    let n = b.len();

    let mut v0: Vec<usize> = (0..=n).collect();
    let mut v1: Vec<usize> = vec![0; n + 1];

    for i in 0..m {
        v1[0] = i + 1;

        for j in 0..n {
            let deletion_cost = v0[j + 1] + 1;
            let insertion_cost = v1[j] + 1;
            let substitution_cost =
                if a.as_bytes()[i] == b.as_bytes()[j] { v0[j] } else { v0[j] + 1 };
            v1[j + 1] =
                core::cmp::min(deletion_cost, core::cmp::min(insertion_cost, substitution_cost));
        }

        core::mem::swap(&mut v0, &mut v1);
    }

    v0[n]
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_unknown_variable(var_name: &str, span: Span, file_idx: u16, state: &State) -> ! {
    let loc = SourceLocation { span, file_idx };
    let suggestions = find_closest_str(var_name, state.v.iter().map(|v| v.name))
        .map(|similar_var| Suggestion {
            loc,
            msg: "A variable with a similar name exists",
            replacement: state.bump.alloc_str(similar_var),
            applicability: SuggestionApplicability::MaybeIncorrect,
        })
        .into_iter()
        .collect();

    let diagnostic = Diagnostic {
        code: ErrorCode::UnknownVariable,
        severity: Severity::Error,
        loc,
        title: None,
        labels: vec![Label {
            loc,
            msg:
                bumpalo::format!(in state.bump, "Cannot find variable `{}` in this scope", var_name)
                    .into_bump_str(),
            role: LabelRole::Primary,
        }],
        notes: Vec::new(),
        help: Vec::new(),
        suggestions,
        parent: None,
    };

    error_diagnostic(&diagnostic, state.sources);
    crash()
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_unknown_function(
    fn_name: &str,
    span: Span,
    scope: &Scope,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    let help = find_closest_str(fn_name, scope.fns().map(|f| f.0)).map(|similar_fn| {
        hformat!("A function with a similar name exists: {BLUE}{similar_fn}{RESET}")
    });
    compiler_error(
        &"Unknown function",
        (span, file_idx),
        &[ReportLabel::new(
            &format_args!("Cannot find function {RED}{fn_name}{RESET} in this scope"),
            Color::Red,
            span,
            file_idx,
        )],
        None,
        help.as_ref().map(|help| help as &dyn Display),
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_unknown_namespace(
    namespace: &[&str],
    span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    compiler_error(
        &"Unknown namespace",
        (span, file_idx),
        &[ReportLabel::new(
            &format_args!("{RED}{}{RESET} is not a valid namespace", namespace.join("::")),
            Color::Red,
            span,
            file_idx,
        )],
        None,
        None,
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_unknown_function_in_namespace(
    fn_name: &str,
    scope: &Scope,
    path: &[&str],
    span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    let namespace_str = path.join("::");
    let namespace = scope.walk_to_namespace(path).unwrap();
    let help = find_closest_str(fn_name, namespace.fns().map(|s| s.0)).map(|similar_fn| {
        hformat!(
            "A function with a similar name exists: {BLUE}{namespace_str}::{similar_fn}{RESET}"
        )
    });
    compiler_error(
        &"Unknown function in namespace",
        (span, file_idx),
        &[ReportLabel::new(
            &format_args!(
                "Cannot find function {RED}{fn_name}{RESET} in namespace {BLUE}{namespace_str}{RESET}"
            ),
            Color::Red,
            span,
            file_idx,
        )],
        None,
        help.as_ref().map(|help| help as &dyn Display),
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_function_already_defined(
    func: &Function,
    redeclaration_span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    compiler_error(
        &"Function already exists",
        (redeclaration_span, file_idx),
        &[
            ReportLabel::new(
                &"Already defined here",
                Color::Blue,
                func.name_span,
                func.src_file_idx,
            ),
            ReportLabel::new(
                &format_args!("Function {BLUE}{}{RESET} is already defined", func.name),
                Color::Red,
                redeclaration_span,
                file_idx,
            ),
        ],
        None,
        None,
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_struct_already_defined(
    s: &Struct,
    redeclaration_span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    compiler_error(
        &"Struct already exists",
        (redeclaration_span, file_idx),
        &[
            ReportLabel::new(&"Already defined here", Color::Blue, s.name_span, s.src_file_idx),
            ReportLabel::new(
                &format_args!("Struct {BLUE}{}{RESET} is already defined", s.name),
                Color::Red,
                redeclaration_span,
                file_idx,
            ),
        ],
        None,
        None,
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_global_already_defined(
    global_name: &str,
    initial_decl_span: Span,
    redeclaration_span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    compiler_error(
        &"Global variable already defined",
        (redeclaration_span, file_idx),
        &[
            ReportLabel::new(&"Already declared here", Color::Red, initial_decl_span, file_idx),
            ReportLabel::new(
                &format_args!(
                    "A second global variable with the name {BLUE}{global_name}{RESET} is declared here"
                ),
                Color::Red,
                redeclaration_span,
                file_idx,
            ),
        ],
        None,
        None,
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_op(
    l: &DataType,
    r: &DataType,
    op: &str,
    span_l: Span,
    span_r: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    let note = if op == "+" {
        Some(format!(
            "The supported types are:\n- {BLUE}{}{RESET} {RED}{op}{RESET} {GREEN}{}{RESET}\n- {BLUE}{}{RESET} {RED}{op}{RESET} {GREEN}{}{RESET}\n- {BLUE}{}{RESET} {RED}{op}{RESET} {GREEN}{}{RESET}\n- {BLUE}T[]{RESET} {RED}{op}{RESET} {GREEN}T[]{RESET}",
            DataType::Int,
            DataType::Int,
            DataType::Float,
            DataType::Float,
            DataType::String,
            DataType::String,
        ))
    } else if op == "-" && l == &DataType::Null {
        Some(format!(
            "The supported types are:\n- {RED}{op}{RESET} {BLUE}{}{RESET}\n- {RED}{op}{RESET} {GREEN}{}{RESET}",
            DataType::Int,
            DataType::Float,
        ))
    } else if matches!(op, "*" | "/" | "-" | "%" | "^" | ">" | ">=" | "<" | "<=") {
        Some(format!(
            "The supported types are:\n- {BLUE}{}{RESET} {RED}{op}{RESET} {GREEN}{}{RESET}\n- {BLUE}{}{RESET} {RED}{op}{RESET} {GREEN}{}{RESET}",
            DataType::Int,
            DataType::Int,
            DataType::Float,
            DataType::Float,
        ))
    } else if matches!(op, "&&" | "||") {
        Some(format!(
            "The supported types are:\n- {BLUE}{}{RESET} {RED}{op}{RESET} {GREEN}{}{RESET}",
            DataType::Bool,
            DataType::Bool,
        ))
    } else if op == "!" {
        Some(format!(
            "The supported types are:\n- {RED}{op}{RESET} {BLUE}{}{RESET}",
            DataType::Bool
        ))
    } else {
        None
    };
    let note = note.as_ref().map(|note| note as &dyn Display);
    let location = (span_l.extend(span_r), file_idx);

    if (op == "-" && l == &DataType::Null) || op == "!" {
        compiler_error(
            &format_args!("Cannot perform operation {RED}{op}{RESET} {BLUE}{r}{RESET}"),
            location,
            &[ReportLabel::new(
                &format_args!("This expression is of type {BLUE}{r}{RESET}"),
                Color::Red,
                span_r,
                file_idx,
            )],
            note,
            None,
            sources,
        );
    }
    compiler_error(
        &format_args!(
            "Cannot perform operation {BLUE}{l}{RESET} {RED}{op}{RESET} {GREEN}{r}{RESET}"
        ),
        location,
        &[
            ReportLabel::new(
                &format_args!("This expression is of type {BLUE}{l}{RESET}"),
                Color::Red,
                span_l,
                file_idx,
            ),
            ReportLabel::new(
                &format_args!("This expression is of type {GREEN}{r}{RESET}"),
                Color::Red,
                span_r,
                file_idx,
            ),
        ],
        note,
        None,
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_unknown_type(
    span: Span,
    file_idx: u16,
    t: &str,
    sources: &[Source],
    scope: &Scope,
) -> ! {
    let help = find_closest_str(t, scope.structs().map(|s| s.0))
        .map(|s| hformat!("A struct with a similar name exists: {BLUE}{s}{RESET}"));
    compiler_error(
        &format_args!("Unknown type {RED}{t}{RESET}"),
        (span, file_idx),
        &[ReportLabel::new(&"This isn't a valid type", Color::Red, span, file_idx)],
        None,
        help.as_ref().map(|help| help as &dyn Display),
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_unknown_type_with_namespace(
    span: Span,
    file_idx: u16,
    t: &str,
    sources: &[Source],
    scope: &Scope,
    path: &[&str],
) -> ! {
    let namespace_str = path.join("::");
    let namespace = scope.walk_to_namespace(path).unwrap();
    let help = find_closest_str(t, namespace.structs().map(|s| s.0)).map(|s| {
        hformat!(
            "A struct with a similar name exists: {BLUE}{namespace_str}{RESET}::{BLUE}{s}{RESET}"
        )
    });
    compiler_error(
        &format_args!("Unknown type {RED}{t}{RESET}"),
        (span, file_idx),
        &[ReportLabel::new(&"This isn't a valid type", Color::Red, span, file_idx)],
        None,
        help.as_ref().map(|help| help as &dyn Display),
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_duplicate_map_key(
    key_first_span: Span,
    key_repeat_span: Span,
    span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    compiler_error(
        &"Key is defined more than once in map",
        (span, file_idx),
        &[
            ReportLabel::new(
                &"This key is first defined here",
                Color::Blue,
                key_first_span,
                file_idx,
            ),
            ReportLabel::new(&"It's then redefined here", Color::Red, key_repeat_span, file_idx),
        ],
        None,
        None,
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_not_literal_map_key(
    key_span: Span,
    map_span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    compiler_error(
        &"Non-literal map key",
        (map_span, file_idx),
        &[ReportLabel::new(&"This map key is not a literal.", Color::Blue, key_span, file_idx)],
        Some(
            &"Map keys must be literals. However, fret not, this requirement™ will soon be removed!",
        ),
        None,
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
fn throw_function_arg_report(
    message: &dyn Display,
    arg_span: Span,
    fn_decl_span: Option<(Span, u16)>,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    let (decl_span, decl_file_idx) = fn_decl_span.unwrap_or((arg_span, file_idx));
    compiler_error(
        &"Invalid argument type",
        (arg_span, file_idx),
        &[
            ReportLabel::new(&"Function is defined here", Color::Blue, decl_span, decl_file_idx),
            ReportLabel::new(message, Color::Red, arg_span, file_idx),
        ][usize::from(fn_decl_span.is_none())..],
        None,
        None,
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_function_arg_invalid_type(
    perceived_type: &DataType,
    expected_type: &DataType,
    arg_span: Span,
    fn_name: &str,
    fn_decl_span: Option<(Span, u16)>,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_function_arg_report(
        &format_args!(
            "Function {BLUE}{fn_name}{RESET} expects this argument's type to be {GREEN}{expected_type}{RESET}, but this expression's type is {RED}{perceived_type}{RESET}"
        ),
        arg_span,
        fn_decl_span,
        file_idx,
        sources,
    )
}

#[cold]
#[optimize(size)]
#[inline(never)]
pub fn error_function_arg_invalid_type_multiple(
    perceived_type: &DataType,
    expected_type: &[DataType],
    arg_span: Span,
    fn_name: &str,
    fn_decl_span: Option<(Span, u16)>,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    throw_function_arg_report(
        &format_args!(
            "Function {BLUE}{fn_name}{RESET} expects this argument to be of type {GREEN}{}{RESET}, but this expression's type is {RED}{perceived_type}{RESET}",
            expected_type
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(&format_args!("{RESET} or {GREEN}").to_string()),
        ),
        arg_span,
        fn_decl_span,
        file_idx,
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_range_invalid_type(
    span: Span,
    perceived_type: &DataType,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    compiler_error(
        &"Invalid type in range",
        (span, file_idx),
        &[ReportLabel::new(
            &format_args!(
                "Expected {BLUE}{}{RESET}, but this expression's type is {RED}{perceived_type}{RESET}",
                DataType::Int
            ),
            Color::Red,
            span,
            file_idx,
        )],
        Some(&format_args!(
            "A range has the following syntax: {BLUE}start{RESET}..{BLUE}end{RESET},\nwhere both {BLUE}start{RESET} and {BLUE}end{RESET} are of type {BLUE}{BLUE}{}{RESET}{RESET}",
            DataType::Int
        )),
        Some(&hformat!("Try using the {BLUE}int(){RESET} function.")),
        sources,
    )
}

#[inline(never)]
#[cold]
#[optimize(size)]
pub fn error_invalid_pattern(
    msg: core::fmt::Arguments,
    help: Option<core::fmt::Arguments>,
    span: Span,
    file_idx: u16,
    sources: &[Source],
) -> ! {
    compiler_error(
        &"Invalid pattern",
        (span, file_idx),
        &[ReportLabel::new(&msg, Color::Red, span, file_idx)],
        None,
        help.as_ref().map(|help| help as &dyn Display),
        sources,
    )
}
