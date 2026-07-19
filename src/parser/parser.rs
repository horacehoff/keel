use crate::BOLD;
use crate::RED;
use crate::RESET;
use crate::compiler::compiler_data::Source;
use crate::compiler::expr::{Expr, Span, var_assign};
use crate::compiler::type_system::TypeExpr;
use crate::errors::BLUE;
use crate::errors::blue;
use crate::errors::crash;
use ariadne::Color;
use ariadne::Label;
use ariadne::Report;
use ariadne::ReportKind;
use blocks::parse_condition_block;
use blocks::parse_eval_block;
use blocks::parse_for_loop;
use blocks::parse_function;
use blocks::parse_loop_block;
use blocks::parse_match;
use blocks::parse_struct_declare;
use blocks::parse_try_catch_block;
use blocks::parse_while_block;
use lexer::parse_string;
use logos::SpannedIter;
use parser_expr::add_op;
use parser_expr::parse_expr;
use smol_strc::SmolStr;
use std::hint::{cold_path, unreachable_unchecked};
use std::iter::Peekable;

use lexer::Token;
use logos::Logos;

mod blocks;
mod lexer;
mod parser_expr;
mod term;

type TokenIter<'a> = Peekable<SpannedIter<'a, Token<'a>>>;

struct ParserCtx<'a> {
    src: Source<'a>,
}

struct Parser<'a> {
    input: TokenIter<'a>,
    ctx: ParserCtx<'a>,
    last_token_end: usize,
}

#[derive(Clone, Copy)]
enum ParserErr<'a> {
    UnexpectedEOF,
    UnknownToken,
    /// (expected, received)
    UnexpectedToken(Token<'a>, Token<'a>, &'static str),
    /// (expected, received)
    UnexpectedTokenStr(&'static str, Token<'a>, &'static str),
    ArrayElementsMissingComma,
    InlineConditionNoElseBlock,
    DivisionByZero,
    ModuloByZero,
    IntegerNegativeExponent,
    ArgumentsMissingCommaSeparator,
    TryBlockNoCatch,
    MatchBlockNoNonWildcardArm,
    MatchBlockZeroArms,
}

#[cold]
#[inline(never)]
fn throw_parser_error(src: Source, Span { start, end }: Span, t: ParserErr) -> ! {
    let err_message = match t {
        ParserErr::UnexpectedEOF => "Unexpected EOF",
        ParserErr::UnknownToken => "Unknown token",
        ParserErr::UnexpectedToken(expected, received, msg) => &format_args!(
            "Expected {BLUE}{BOLD}{expected}{RESET}, but got {RED}{BOLD}{received}{RESET}. {msg}"
        )
        .to_string(),
        ParserErr::UnexpectedTokenStr(expected, received, msg) => &format_args!(
            "Expected {BLUE}{BOLD}{expected}{RESET}, but got {RED}{BOLD}{received}{RESET}. {msg}"
        )
        .to_string(),
        ParserErr::ArrayElementsMissingComma => "Array elements must be separated by a comma",
        ParserErr::InlineConditionNoElseBlock => "Inline conditions must have an else block",
        ParserErr::DivisionByZero => "Division by zero",
        ParserErr::ModuloByZero => "Modulo by zero",
        ParserErr::IntegerNegativeExponent => "Integers cannot be raised to a negative exponent",
        ParserErr::ArgumentsMissingCommaSeparator => "Arguments must be separated by a comma",
        ParserErr::TryBlockNoCatch => {
            "A {BLUE}{BOLD}try{RESET} block must have at least one {BLUE}{BOLD}catch{RESET} block"
        }
        ParserErr::MatchBlockNoNonWildcardArm => {
            "{BLUE}{BOLD}Match blocks{RESET} must have {BOLD}at least one non-wildcard arm{RESET}"
        }
        ParserErr::MatchBlockZeroArms => {
            "{BLUE}{BOLD}Match blocks{RESET} must have {BOLD}at least one arm{RESET}"
        }
    };
    eprintln!("{RED}KEEL ERROR{RESET}");
    let report = Report::build(
        ReportKind::Error,
        (src.filename, (start as usize)..(end as usize)),
    )
    .with_label(
        Label::new((src.filename, (start as usize)..(end as usize)))
            .with_message(err_message)
            .with_color(Color::Red),
    )
    .finish();

    #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
    report
        .eprint((src.filename, ariadne::Source::from(src.contents)))
        .unwrap();

    #[cfg(any(target_arch = "wasm32", feature = "embed"))]
    report
        .write(
            (src.filename, ariadne::Source::from(src.contents)),
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

impl<'a> Parser<'a> {
    #[inline(always)]
    fn eof_span(&self) -> Span {
        let end = self.ctx.src.contents.len();
        (end, end).into()
    }
    #[cold]
    #[inline(never)]
    fn error(&self, span: Span, error: ParserErr) -> ! {
        throw_parser_error(self.ctx.src, span, error)
    }
    #[inline(always)]
    fn next_token(&mut self) -> (Token<'a>, Span) {
        let t = self.input.next().unwrap_or_else(
            #[cold]
            || {
                self.error(self.eof_span(), ParserErr::UnexpectedEOF);
            },
        );
        self.last_token_end = t.1.end;
        (
            t.0.unwrap_or_else(
                #[cold]
                |()| self.error((t.1.start, t.1.end).into(), ParserErr::UnknownToken),
            ),
            (t.1.start, t.1.end).into(),
        )
    }
    #[inline(always)]
    fn peek_token(&mut self) -> Token<'a> {
        let Some((t, start, end)) = self
            .input
            .peek()
            .map(|(t, span)| (*t, span.start, span.end))
        else {
            self.error(self.eof_span(), ParserErr::UnexpectedEOF);
        };
        t.unwrap_or_else(
            #[cold]
            |()| self.error((start, end).into(), ParserErr::UnknownToken),
        )
    }
    #[inline(always)]
    fn peek_token_span(&mut self) -> Span {
        let Some((_, start, end)) = self
            .input
            .peek()
            .map(|(t, span)| (*t, span.start, span.end))
        else {
            self.error(self.eof_span(), ParserErr::UnexpectedEOF);
        };
        Span {
            start: start as u32,
            end: end as u32,
        }
    }
    #[inline(always)]
    fn peek_token_opt(&mut self) -> Option<Token<'a>> {
        let (t, start, end) = self
            .input
            .peek()
            .map(|(t, span)| (*t, span.start, span.end))?;
        Some(t.unwrap_or_else(
            #[cold]
            |()| self.error((start, end).into(), ParserErr::UnknownToken),
        ))
    }
    #[inline(always)]
    fn peek_token_opt_span(&mut self) -> Option<Span> {
        self.input
            .peek()
            .map(|x| (x.1.start as u32, x.1.end as u32).into())
    }
    #[inline(always)]
    fn next_token_expect(&mut self, expected: Token, msg: &'static str) -> Span {
        let (next_token, span) = self.next_token();
        if next_token != expected {
            self.error(span, ParserErr::UnexpectedToken(expected, next_token, msg));
        }
        span
    }
    #[inline(always)]
    fn next_token_expect_closer(
        &mut self,
        opener: Token,
        opener_span: Span,
        expected_closer: Token,
    ) -> u32 {
        if let Some(t) = self.peek_token_opt() {
            if t == expected_closer {
                self.next_token().1.end
            } else {
                let span = self.peek_token_span();
                error_unclosed_delimiter(
                    self,
                    opener,
                    opener_span,
                    expected_closer,
                    Some((t, span)),
                );
            }
        } else {
            error_unclosed_delimiter(self, opener, opener_span, expected_closer, None);
        }
    }
    #[inline(never)]
    #[cold]
    pub fn throw_parser_err<'b, F: Fn() -> Report<'b, (&'b str, core::ops::Range<usize>)>>(
        &self,
        report: F,
    ) -> ! {
        let report = report();

        #[cfg(not(any(target_arch = "wasm32", feature = "embed")))]
        report
            .eprint((
                self.ctx.src.filename,
                ariadne::Source::from(self.ctx.src.contents),
            ))
            .unwrap();

        #[cfg(any(target_arch = "wasm32", feature = "embed"))]
        report
            .write(
                (
                    self.ctx.src.filename,
                    ariadne::Source::from(self.ctx.src.contents),
                ),
                crate::captured_output::CapturedOutputWriter,
            )
            .unwrap();

        crash();
    }
}

// Call after DoubleColon is skipped
// Returns end
fn parse_namespace(parser: &mut Parser<'_>, initial: SmolStr) -> (Box<[SmolStr]>, u32) {
    let mut namespace: Vec<SmolStr> = Vec::with_capacity(2);
    namespace.push(initial);
    let mut end: u32;
    loop {
        let (next_token, span) = parser.next_token();
        if let Token::Identifier(i) = next_token {
            namespace.push(SmolStr::new(i));
            end = span.end;
        } else {
            cold_path();
            parser.error(
                span,
                ParserErr::UnexpectedToken(
                    Token::Identifier(""),
                    next_token,
                    "Wrong namespace syntax",
                ),
            );
        }
        let next_token = parser.peek_token();
        if next_token == Token::DoubleColon {
            continue;
        }
        return (Box::from(namespace), end);
    }
}

// Must be called after LParen is skipped
fn parse_args(parser: &mut Parser<'_>) -> (Box<[Expr]>, Box<[Span]>, u32) {
    let mut args = Vec::with_capacity(4);
    let mut arg_markers: Vec<Span> = Vec::with_capacity(4);
    loop {
        if parser.peek_token() == Token::RParen {
            let end = parser.next_token().1.end;
            return (Box::from(args), Box::from(arg_markers), end);
        }
        let arg_start: u32 = parser.peek_token_span().start;
        args.push(parse_expr(parser));
        arg_markers.push((arg_start, parser.peek_token_span().start).into());
        if parser.peek_token() == Token::Comma {
            parser.next_token();
        } else if !(parser.peek_token() == Token::RParen) {
            cold_path();
            let span = parser.peek_token_span();
            parser.error(span, ParserErr::ArgumentsMissingCommaSeparator);
        }
    }
}

fn parse_statement(parser: &mut Parser<'_>) -> Option<Expr> {
    let token = parser.peek_token_opt()?;
    let t_span = parser.peek_token_span();
    match token {
        Token::If => Some(parse_condition_block(parser, t_span.start)),
        Token::While => Some(parse_while_block(parser)),
        Token::For => Some(parse_for_loop(parser)),
        Token::Match => Some(parse_match(parser)),
        Token::LBrace => Some(parse_eval_block(parser)),
        Token::Function => Some(parse_function(parser)),
        Token::Loop => Some(parse_loop_block(parser)),
        Token::Try => Some(parse_try_catch_block(parser)),
        Token::Struct => Some(parse_struct_declare(parser)),
        Token::RBrace => None,
        t => Some(parse_line(parser, t)),
    }
}

fn parse_var_declare(parser: &mut Parser<'_>) -> Expr {
    let (t, _) = parser.next_token();
    debug_assert_eq!(t, Token::Let);
    let (t, span) = parser.next_token();
    let var_name = if let Token::Identifier(id) = t {
        SmolStr::new(id)
    } else {
        cold_path();
        parser.error(
            span,
            ParserErr::UnexpectedToken(
                Token::Identifier(""),
                t,
                "Variable names must be identifiers.",
            ),
        );
    };
    parser.next_token_expect(
        Token::Equals,
        "Variable declarations need a '=' to separate the name from the value.",
    );
    let var_value = parse_expr(parser);
    Expr::VarDeclare(var_name, Box::new(var_value))
}

fn parse_var_assign(input: &mut Parser<'_>, e: Expr, e_start: u32) -> Expr {
    let (t, _) = input.next_token();
    debug_assert_eq!(t, Token::Equals);
    let e_end = input.peek_token_span().end;
    let v_start = input.peek_token_span().start;
    let v = parse_expr(input);
    let v_end = input.peek_token_span().start;
    var_assign(e, v, (e_start, e_end).into(), (v_start, v_end).into())
}

fn parse_op_var_assign(input: &mut Parser<'_>, e: Expr, e_start: u32, op: Token<'_>) -> Expr {
    let operand_end = input.last_token_end as u32;
    let (t, _) = input.next_token();
    debug_assert_eq!(t, op);
    let e_end = input.peek_token_span().end;
    let v_start = input.peek_token_span().start;
    let v = parse_expr(input);
    let v_end = input.last_token_end as u32;
    let op = match op {
        Token::AssignOpAdd => Token::OpAdd,
        Token::AssignOpSub => Token::OpSub,
        Token::AssignOpMul => Token::OpMul,
        Token::AssignOpDiv => Token::OpDiv,
        Token::AssignOpMod => Token::OpMod,
        Token::AssignOpPow => Token::OpPow,
        _ => unsafe { unreachable_unchecked() },
    };
    var_assign(
        e.clone(),
        add_op(
            input,
            op,
            e,
            v,
            (e_start, operand_end).into(),
            (v_start, v_end).into(),
        ),
        (e_start, e_end).into(),
        (v_start, v_end).into(),
    )
}

fn parse_return(input: &mut Parser<'_>) -> Expr {
    let (t, _) = input.next_token();
    debug_assert_eq!(t, Token::Return);
    if input.peek_token_opt() == Some(Token::SemiColon) {
        Expr::ReturnVal(Box::new(None))
    } else {
        let e = parse_expr(input);
        Expr::ReturnVal(Box::new(Some(e)))
    }
}

fn parse_line(input: &mut Parser<'_>, peek: Token<'_>) -> Expr {
    let line_code = match peek {
        Token::Let => parse_var_declare(input),
        Token::Return => parse_return(input),
        Token::Break => {
            input.next_token();
            Expr::Break
        }
        Token::Continue => {
            input.next_token();
            Expr::Continue
        }
        _ => {
            let e_start = input.peek_token_span().start;
            let e = parse_expr(input);
            let peek_token = input.peek_token_opt();
            match peek_token {
                Some(Token::Equals) => parse_var_assign(input, e, e_start),
                Some(
                    op @ (Token::AssignOpAdd
                    | Token::AssignOpSub
                    | Token::AssignOpMul
                    | Token::AssignOpDiv
                    | Token::AssignOpMod
                    | Token::AssignOpPow),
                ) => parse_op_var_assign(input, e, e_start, op),
                _ => e,
            }
        }
    };
    if input.peek_token_opt() != Some(Token::SemiColon) {
        error_missing_semicolon(input);
    }
    input.next_token();
    // input.next_token_expect(Token::SemiColon, "Lines must end with a ';'.");
    line_code
}

#[cold]
#[inline(never)]
fn error_unclosed_delimiter(
    parser: &Parser<'_>,
    opener_token: Token,
    opener_span: Span,
    expected_closer_token: Token,
    end: Option<(Token, Span)>,
) -> ! {
    parser.throw_parser_err(|| {
        let mut report = Report::build(
            ariadne::ReportKind::Error,
            (parser.ctx.src.filename, opener_span.into()),
        )
        .with_message("Unclosed delimiter")
        .with_label(
            Label::new((parser.ctx.src.filename, opener_span.into()))
                .with_message(format_args!("This {opener_token} is never closed"))
                .with_color(ariadne::Color::Red),
        );

        if let Some((actual_closer_token, actual_closer_token_span)) = end {
            report = report.with_label(
                Label::new((parser.ctx.src.filename, actual_closer_token_span.into()))
                    .with_message(format_args!(
                        "Expected {expected_closer_token} but found {actual_closer_token}"
                    ))
                    .with_color(ariadne::Color::Red),
            );
        } else {
            report = report
                .with_label(
                    Label::new((parser.ctx.src.filename, parser.eof_span().into()))
                        .with_message(format_args!(
                            "Expected {expected_closer_token} but the file ends here"
                        ))
                        .with_color(ariadne::Color::Red),
                )
                .with_help(format_args!(
                    "Add a {} here to close it",
                    blue(expected_closer_token)
                ));
        }

        report.finish()
    })
}

#[cold]
#[inline(never)]
fn error_missing_semicolon(parser: &Parser<'_>) -> ! {
    parser.throw_parser_err(|| {
        Report::build(
            ariadne::ReportKind::Error,
            (
                parser.ctx.src.filename,
                (parser.last_token_end..parser.last_token_end),
            ),
        )
        .with_message("Missing semicolon")
        .with_label(
            Label::new((
                parser.ctx.src.filename,
                (parser.last_token_end..parser.last_token_end),
            ))
            .with_message(format_args!("Add a {} here", blue(';')))
            .with_color(ariadne::Color::Blue),
        )
        .with_help("All statements end with a ';'")
        .finish()
    })
}

fn parse_code(input: &mut Parser<'_>) -> Vec<Expr> {
    let mut output: Vec<Expr> = Vec::with_capacity(4);
    while let Some(e) = parse_statement(input) {
        output.push(e);
    }
    output
}

fn parse_file_import(parser: &mut Parser<'_>) -> Expr {
    let (t, Span { start, end: _ }) = parser.next_token();
    debug_assert_eq!(t, Token::Import);
    let (next_token, span) = parser.next_token();
    let path = if let Token::String(s) = next_token {
        SmolStr::new(parse_string(s))
    } else {
        cold_path();
        parser.error(
            span,
            ParserErr::UnexpectedToken(Token::String(""), next_token, "Paths must be strings."),
        );
    };
    let end = span.end;
    let peek_token = parser.peek_token_opt();
    if peek_token == Some(Token::As) {
        parser.next_token();
        let (next_token, span) = parser.next_token();
        let alias = if let Token::Identifier(id) = next_token {
            SmolStr::new(id)
        } else {
            cold_path();
            parser.error(
                span,
                ParserErr::UnexpectedToken(
                    Token::Identifier(""),
                    next_token,
                    "Module aliases must be identifiers.",
                ),
            );
        };
        parser.next_token_expect(
            Token::SemiColon,
            "Import statements must end with a semicolon",
        );
        Expr::ImportFile(path, Some(alias), (start, span.end).into())
    } else {
        parser.next_token_expect(
            Token::SemiColon,
            "Import statements must end with a semicolon",
        );
        Expr::ImportFile(path, None, (start, end).into())
    }
}

fn parse_type(parser: &mut Parser<'_>) -> TypeExpr {
    let t = parse_atomic_type(parser);
    if parser.peek_token() == Token::Pipe {
        let mut poly = Vec::with_capacity(2);
        poly.push(t);
        while parser.peek_token() == Token::Pipe {
            parser.next_token();
            poly.push(parse_atomic_type(parser));
        }
        TypeExpr::Union(poly.into_boxed_slice())
    } else {
        t
    }
}

fn parse_atomic_type(parser: &mut Parser<'_>) -> TypeExpr {
    let (next_token, span) = parser.next_token();
    let mut t = if next_token == Token::LBrace {
        let key_t = parse_type(parser);
        parser.next_token_expect(
            Token::Colon,
            "A colon must separate key and value types in map types",
        );
        let value_t = parse_type(parser);
        parser.next_token_expect(Token::RBrace, "Unmatched '{'");
        TypeExpr::Map(Box::new(key_t), Box::new(value_t))
    } else if let Token::Identifier(i) = next_token {
        TypeExpr::Identifier(SmolStr::new(i))
    } else {
        cold_path();
        parser.error(
            span,
            ParserErr::UnexpectedToken(Token::Identifier(""), next_token, "Invalid type"),
        );
    };
    loop {
        if parser.peek_token() == Token::LBracket {
            parser.next_token();
            parser.next_token_expect(Token::RBracket, "Unmatched '['");
            t = TypeExpr::Array(Box::new(t));
        } else {
            break;
        }
    }
    t
}

fn parse_dylib_import(parser: &mut Parser<'_>) -> Expr {
    let (t, Span { start, end: _ }) = parser.next_token();
    debug_assert_eq!(t, Token::Dylib);
    let (next_token, span) = parser.next_token();
    let path = if let Token::String(s) = next_token {
        SmolStr::new(parse_string(s))
    } else {
        cold_path();
        parser.error(
            span,
            ParserErr::UnexpectedToken(Token::String(""), next_token, "Paths must be strings."),
        );
    };
    parser.next_token_expect(Token::LBrace, "Blocks need to start with '{'.");
    let mut fn_signatures: Vec<(SmolStr, Box<[TypeExpr]>, TypeExpr)> = Vec::new();
    let end: u32;
    loop {
        if parser.peek_token() == Token::RBrace {
            end = parser.next_token().1.end;
            break;
        }

        let type_start = parser.peek_token();
        let first = parse_type(parser);
        let (return_type, fn_name) = if parser.peek_token() == Token::LParen {
            if let TypeExpr::Identifier(name) = first {
                (TypeExpr::Identifier(SmolStr::new_static("null")), name)
            } else {
                parser.error(
                    span,
                    ParserErr::UnexpectedToken(
                        Token::Identifier(""),
                        type_start,
                        "Function names must be identifiers.",
                    ),
                );
            }
        } else {
            let (next_token, span) = parser.next_token();
            let fn_name = if let Token::Identifier(name) = next_token {
                SmolStr::new(name)
            } else {
                cold_path();
                parser.error(
                    span,
                    ParserErr::UnexpectedToken(
                        Token::Identifier(""),
                        next_token,
                        "Function names must be identifiers.",
                    ),
                );
            };
            (first, fn_name)
        };
        parser.next_token_expect(
            Token::LParen,
            "Function arguments must be delimited by parentheses",
        );
        let mut args: Vec<TypeExpr> = Vec::with_capacity(2);
        loop {
            if parser.peek_token() == Token::RParen {
                break;
            }
            args.push(parse_type(parser));
            if parser.peek_token() == Token::Comma {
                parser.next_token();
            } else if !(parser.peek_token() == Token::RParen) {
                cold_path();
                let span = parser.peek_token_span();
                parser.error(span, ParserErr::ArgumentsMissingCommaSeparator);
            }
        }
        parser.next_token_expect(Token::RParen, "Unmatched ')'");
        parser.next_token_expect(
            Token::SemiColon,
            "Function definitions must end with a semicolon",
        );
        fn_signatures.push((fn_name, Box::from(args), return_type));
    }
    Expr::ImportDylib(path, Box::from(fn_signatures), (start, end).into())
}

#[inline(always)]
fn parse_file(parser: &mut Parser<'_>) -> Vec<Expr> {
    let mut output: Vec<Expr> = Vec::with_capacity(2);
    // parse file statements
    while let Some(t) = parser.peek_token_opt() {
        output.push(match t {
            Token::Function => parse_function(parser),
            Token::Import => parse_file_import(parser),
            Token::Struct => parse_struct_declare(parser),
            Token::Dylib => parse_dylib_import(parser),
            unexpected => {
                cold_path();
                let span = parser.peek_token_span();
                parser.error(span, ParserErr::UnexpectedTokenStr("'fn' (function declaration), 'import', 'struct' (struct declaration), or 'dylib' (dynamic library import)", unexpected, "Invalid file statement."));
            }
        });
    }
    output
}

pub fn parse(input: &str, src: (&str, &str)) -> Vec<Expr> {
    parse_file(&mut Parser {
        input: Token::lexer(input).spanned().peekable(),
        ctx: ParserCtx {
            src: Source {
                filename: src.0,
                contents: src.1,
            },
        },
        last_token_end: 0,
    })
}
