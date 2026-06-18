#![allow(clippy::redundant_else)]
use std::hint::{cold_path, unreachable_unchecked};

use crate::{
    errors::{ParserErr, throw_parser_error},
    parser::Token::{Identifier, RangeDot},
    expr::{
        Expr::{self},
        Span, var_assign,
    },
};
use logos::{Logos, SpannedIter};
use smol_strc::{SmolStr, ToSmolStr};
use std::iter::Peekable;

type TokenIter<'a> = Peekable<SpannedIter<'a, Token<'a>>>;

struct ParserCtx<'a> {
    /// (filename, contents)
    src: (&'a str, &'a str),
}

struct Parser<'a> {
    input: TokenIter<'a>,
    ctx: ParserCtx<'a>,
}

impl<'a> Parser<'a> {
    #[inline(always)]
    fn eof_span(&self) -> Span {
        let end = self.ctx.src.1.len();
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
                cold_path();
                self.error(self.eof_span(), ParserErr::UnexpectedEOF);
            },
        );
        (
            t.0.unwrap_or_else(
                #[cold]
                |()| {
                    cold_path();
                    self.error((t.1.start, t.1.end).into(), ParserErr::UnknownToken)
                },
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
            cold_path();
            self.error(self.eof_span(), ParserErr::UnexpectedEOF);
        };
        t.unwrap_or_else(
            #[cold]
            |()| {
                cold_path();
                self.error((start, end).into(), ParserErr::UnknownToken)
            },
        )
    }
    #[inline(always)]
    fn peek_token_span(&mut self) -> Span {
        let Some((_, start, end)) = self
            .input
            .peek()
            .map(|(t, span)| (*t, span.start, span.end))
        else {
            cold_path();
            self.error(self.eof_span(), ParserErr::UnexpectedEOF);
        };
        Span {
            start: start as u32,
            end: end as u32,
        }
    }
    #[inline(always)]
    fn peek_token_start(&mut self) -> u32 {
        let Some((_, span)) = self.input.peek() else {
            cold_path();
            self.error(self.eof_span(), ParserErr::UnexpectedEOF);
        };
        span.start as u32
    }
    #[inline(always)]
    fn peek_token_end(&mut self) -> u32 {
        let Some((_, span)) = self.input.peek() else {
            cold_path();
            self.error(self.eof_span(), ParserErr::UnexpectedEOF);
        };
        span.end as u32
    }
    #[inline(always)]
    fn peek_token_end_opt(&mut self) -> Option<u32> {
        self.input.peek().map(|t| t.1.end as u32)
    }
    #[inline(always)]
    fn peek_token_opt(&mut self) -> Option<Token<'a>> {
        let (t, start, end) = self
            .input
            .peek()
            .map(|(t, span)| (*t, span.start, span.end))?;
        Some(t.unwrap_or_else(
            #[cold]
            |()| {
                cold_path();
                self.error((start, end).into(), ParserErr::UnknownToken)
            },
        ))
    }
    #[inline(always)]
    fn peek_span_opt(&mut self) -> Option<Span> {
        self.input
            .peek()
            .map(|x| (x.1.start as u32, x.1.end as u32).into())
    }
    #[inline(always)]
    fn next_token_expect(&mut self, expected: Token, msg: &'static str) {
        let (next_token, span) = self.next_token();
        if next_token != expected {
            cold_path();
            self.error(span, ParserErr::UnexpectedToken(expected, next_token, msg));
        }
    }
    #[inline(always)]
    fn next_token_expect_end(&mut self, expected: Token, msg: &'static str) -> u32 {
        let (next_token, span) = self.next_token();
        if next_token != expected {
            cold_path();
            self.error(span, ParserErr::UnexpectedToken(expected, next_token, msg));
        }
        span.end
    }
}

// Must be called right after LParen is skipped
// Identifier LParen Expr RParen
// Parses: Expr RParen
fn parse_fn_call(parser: &mut Parser<'_>, namespace: Box<[SmolStr]>, start: u32) -> Expr {
    let (args, arg_markers, end) = parse_args(parser);
    Expr::FunctionCall(args, namespace, (start, end).into(), arg_markers)
}

// Must be called right after LParen is skipped
fn parse_struct(parser: &mut Parser<'_>, namespace: Box<[SmolStr]>, start: u32) -> Expr {
    let mut fields: Vec<(SmolStr, Expr, Span)> = Vec::with_capacity(4);
    let end: u32;
    loop {
        let (next_token, span) = parser.next_token();
        let field_name = if let Token::Identifier(i) = next_token {
            SmolStr::new(i)
        } else {
            cold_path();
            parser.error(
                span,
                ParserErr::UnexpectedToken(
                    Token::Identifier(""),
                    next_token,
                    "Struct field names must be identifiers.",
                ),
            )
        };
        parser.next_token_expect(
            Token::Colon,
            "A colon must separate a field from its value.",
        );
        let field_start: u32 = parser.peek_token_start();
        let field_value = parse_expr(parser);
        fields.push((
            field_name,
            field_value,
            (field_start, parser.peek_token_start()).into(),
        ));
        let (next_token, span) = parser.next_token();
        if next_token == Token::RBrace {
            end = span.end;
            break;
        } else if next_token != Token::Comma {
            cold_path();
            parser.error(
                span,
                ParserErr::UnexpectedToken(
                    Token::Comma,
                    next_token,
                    "In structs, fields must be separated by a comma.",
                ),
            );
        } else if parser.peek_token() == Token::RBrace {
            end = parser.next_token().1.end;
            break;
        }
    }

    Expr::Struct(namespace, Box::from(fields), (start, end).into())
}

fn parse_term(parser: &mut Parser<'_>, allow_struct: bool) -> Expr {
    let (t, t_span) = parser.next_token();
    match t {
        Token::Int(i) => Expr::Int(i),
        Token::Float(f) => Expr::Float(f),
        Token::String(s) => Expr::String(crate::util::parse_string(s)),
        Token::True => Expr::Bool(true),
        Token::False => Expr::Bool(false),
        Token::Null => Expr::Null,
        Token::Identifier(s) => {
            let start = t_span.start;
            match parser.peek_token_opt() {
                // FUNCTION CALL:
                // Identifier LParen Expr RParen
                Some(Token::LParen) => {
                    parser.next_token();
                    parse_fn_call(parser, Box::new([s.to_smolstr()]), start)
                }
                // STRUCT
                Some(Token::LBrace) if allow_struct => {
                    parser.next_token();
                    parse_struct(parser, Box::from([SmolStr::new(s)]), start)
                }
                // NAMESPACE
                Some(Token::DoubleColon) => {
                    parser.next_token();
                    let (namespace, terminator, span) = parse_namespace(parser, SmolStr::new(s));
                    if terminator == Token::LParen {
                        // FUNCTION CALL WITH NAMESPACE:
                        // (Identifier DoubleColon)+ Identifier LParen Expr RParen
                        parse_fn_call(parser, namespace, start)
                    } else if terminator == Token::LBrace {
                        // STRUCT WITH NAMESPACE
                        parse_struct(parser, namespace, start)
                    } else {
                        cold_path();
                        parser.error(
                            span,
                            ParserErr::UnexpectedTokenStr(
                                "'(' (function call), '{' (struct), or '::' (namespace)",
                                terminator,
                                "",
                            ),
                        );
                    }
                }
                _ => Expr::Var(SmolStr::new(s), (t_span.start, t_span.end).into()),
            }
        }
        Token::LBracket => {
            let start = t_span.start;
            let end: u32;
            let mut elems = Vec::with_capacity(4);
            loop {
                if parser.peek_token_opt() == Some(Token::RBracket) {
                    end = parser.next_token().1.end;
                    break;
                }
                elems.push(parse_expr(parser));
                if parser.peek_token_opt() == Some(Token::Comma) {
                    parser.next_token();
                } else if !(parser.peek_token_opt() == Some(Token::RBracket)) {
                    cold_path();
                    let span = unsafe { parser.peek_span_opt().unwrap_unchecked() };
                    parser.error(span, ParserErr::ArrayElementsMissingComma);
                }
            }
            Expr::Array(Box::from(elems), (start, end).into())
        }
        // LParen Expr RParen
        Token::LParen => {
            let v = parse_expr(parser);
            parser.next_token_expect(Token::RParen, "Unmatched ')'");
            v
        }
        // - Expr
        Token::OpSub => match parse_expr_with_precedence(parser, 8, allow_struct) {
            Expr::Int(i) => Expr::Int(i.wrapping_neg()),
            Expr::Float(f) => Expr::Float(-f),
            other => Expr::Neg(
                Box::new(other),
                (t_span.start, parser.peek_token_start()).into(),
            ),
        },
        // ! Expr
        Token::OpNot => match parse_expr_with_precedence(parser, 8, allow_struct) {
            Expr::Bool(b) => Expr::Bool(!b),
            other => Expr::BoolNeg(
                Box::new(other),
                (t_span.start, parser.peek_token_start()).into(),
            ),
        },
        // Inline condition
        Token::If => {
            let condition = parse_expr_no_struct(parser);
            parser.next_token_expect(Token::LBrace, "Expected '{'");
            let mut output_code: Vec<Expr> = Vec::with_capacity(2);
            output_code.push(parse_expr(parser));
            let mut end = parser.next_token_expect_end(Token::RBrace, "Unmatched '}'");
            loop {
                let next_token = parser.peek_token_opt();
                if next_token != Some(Token::Else) {
                    break;
                }
                parser.next_token();
                // if -> else if
                // lbrace -> else
                // else -> end
                let next_token = parser.peek_token_opt();
                if next_token == Some(Token::If) {
                    parser.next_token();
                    let else_if_condition = parse_expr_no_struct(parser);
                    parser.next_token_expect(Token::LBrace, "Blocks must begin with a '{'.");
                    let else_if_value = parse_expr(parser);
                    end = parser.next_token_expect_end(Token::RBrace, "Unmatched '}'");
                    output_code.push(Expr::ElseIfBlock(
                        Box::new(else_if_condition),
                        Box::new([else_if_value]),
                    ));
                } else if next_token == Some(Token::LBrace) {
                    parser.next_token();
                    let else_value = parse_expr(parser);
                    end = parser.next_token_expect_end(Token::RBrace, "Unmatched '}'");
                    output_code.push(Expr::ElseBlock(Box::new([else_value])));
                    break;
                } else {
                    break;
                }
            }
            if !matches!(output_code.last().unwrap(), Expr::ElseBlock(_)) {
                cold_path();
                parser.error(
                    (t_span.start, end).into(),
                    ParserErr::InlineConditionNoElseBlock,
                );
                // panic!("Inline conditions need an else block");
            }
            Expr::InlineCondition(
                Box::new(condition),
                Box::from(output_code),
                (t_span.start, end).into(),
            )
        }
        unexpected => {
            cold_path();
            parser.error(
                t_span,
                ParserErr::UnexpectedTokenStr("Term ", unexpected, ""),
            );
        }
    }
}

fn parse_expr_with_precedence(
    input: &mut Parser<'_>,
    min_precedence: u8,
    allow_struct: bool,
) -> Expr {
    let start = input.peek_token_start();
    let mut end = input.peek_token_end();
    let mut lhs = parse_term(input, allow_struct);
    end = input.peek_token_end_opt().unwrap_or(end);
    lhs = parse_postfix_op(input, lhs, (start, end).into());
    while let Some(peek) = input.peek_token_opt() {
        let Some((op, op_precedence)) = check_op(peek, min_precedence) else {
            break;
        };
        input.next_token();
        let end = input.peek_token_end();
        let rhs = parse_expr_with_precedence(input, op_precedence, allow_struct);
        lhs = add_op(input, op, lhs, rhs, (start, end).into());
    }
    lhs
}

fn add_op(parser: &Parser<'_>, op: Token, lhs: Expr, rhs: Expr, span: Span) -> Expr {
    match op {
        Token::OpOr => match (lhs, rhs) {
            (Expr::Bool(false), c) | (c, Expr::Bool(false)) => c,
            (Expr::Bool(true), _) | (_, Expr::Bool(true)) => Expr::Bool(true),
            (lhs, rhs) => Expr::BoolOr(Box::new(lhs), Box::new(rhs), span),
        },
        Token::OpAnd => match (lhs, rhs) {
            (Expr::Bool(false), _) | (_, Expr::Bool(false)) => Expr::Bool(false),
            (Expr::Bool(true), c) | (c, Expr::Bool(true)) => c,
            (lhs, rhs) => Expr::BoolAnd(Box::new(lhs), Box::new(rhs), span),
        },
        Token::OpEq => Expr::Eq(Box::new(lhs), Box::new(rhs)),
        Token::OpNEq => Expr::NotEq(Box::new(lhs), Box::new(rhs)),
        Token::OpInf => match (lhs, rhs) {
            (Expr::Int(x), Expr::Int(y)) => Expr::Bool(x < y),
            (Expr::Float(x), Expr::Float(y)) => Expr::Bool(x < y),
            (lhs, rhs) => Expr::Inf(Box::new(lhs), Box::new(rhs), span),
        },
        Token::OpInfEq => match (lhs, rhs) {
            (Expr::Int(x), Expr::Int(y)) => Expr::Bool(x <= y),
            (Expr::Float(x), Expr::Float(y)) => Expr::Bool(x <= y),
            (lhs, rhs) => Expr::InfEq(Box::new(lhs), Box::new(rhs), span),
        },
        Token::OpSup => match (lhs, rhs) {
            (Expr::Int(x), Expr::Int(y)) => Expr::Bool(x > y),
            (Expr::Float(x), Expr::Float(y)) => Expr::Bool(x > y),
            (lhs, rhs) => Expr::Sup(Box::new(lhs), Box::new(rhs), span),
        },
        Token::OpSupEq => match (lhs, rhs) {
            (Expr::Int(x), Expr::Int(y)) => Expr::Bool(x >= y),
            (Expr::Float(x), Expr::Float(y)) => Expr::Bool(x >= y),
            (lhs, rhs) => Expr::SupEq(Box::new(lhs), Box::new(rhs), span),
        },
        Token::OpAdd => match (lhs, rhs) {
            (Expr::Int(x), Expr::Int(y)) => Expr::Int(x + y),
            (Expr::Float(x), Expr::Float(y)) => Expr::Float(x + y),
            (Expr::String(x), Expr::String(y)) => Expr::String(format_args!("{x}{y}").to_smolstr()),
            (lhs, rhs) => Expr::Add(Box::new(lhs), Box::new(rhs), span),
        },
        Token::OpSub => match (lhs, rhs) {
            (Expr::Int(x), Expr::Int(y)) => Expr::Int(x - y),
            (Expr::Float(x), Expr::Float(y)) => Expr::Float(x - y),
            (lhs, rhs) => Expr::Sub(Box::new(lhs), Box::new(rhs), span),
        },
        Token::OpMul => match (lhs, rhs) {
            (Expr::Int(x), Expr::Int(y)) => Expr::Int(x * y),
            (Expr::Float(x), Expr::Float(y)) => Expr::Float(x * y),
            (lhs, rhs) => Expr::Mul(Box::new(lhs), Box::new(rhs), span),
        },
        Token::OpDiv => match (lhs, rhs) {
            (_, Expr::Int(0)) => {
                cold_path();
                parser.error(span, ParserErr::DivisionByZero);
            }
            (Expr::Int(x), Expr::Int(y)) => Expr::Int(x / y),
            (Expr::Float(x), Expr::Float(y)) => Expr::Float(x / y),
            (lhs, rhs) => Expr::Div(Box::new(lhs), Box::new(rhs), span),
        },
        Token::OpMod => match (lhs, rhs) {
            (_, Expr::Int(0) | Expr::Float(0.0)) => {
                cold_path();
                parser.error(span, ParserErr::ModuloByZero);
            }
            (Expr::Int(x), Expr::Int(y)) => Expr::Int(x % y),
            (Expr::Float(x), Expr::Float(y)) => Expr::Float(x % y),
            (lhs, rhs) => Expr::Mod(Box::new(lhs), Box::new(rhs), span),
        },
        Token::OpPow => match (lhs, rhs) {
            (Expr::Int(x), Expr::Int(y)) => {
                if y >= 0 {
                    Expr::Int(x.pow(y as u32))
                } else {
                    cold_path();
                    parser.error(span, ParserErr::IntegerNegativeExponent);
                }
            }
            (Expr::Float(x), Expr::Float(y)) => Expr::Float(x.powf(y)),
            (Expr::Float(x), Expr::Int(y)) => Expr::Float(x.powi(y)),
            (lhs, rhs) => Expr::Pow(Box::new(lhs), Box::new(rhs), span),
        },
        _ => unsafe { unreachable_unchecked() },
    }
}

#[inline(always)]
const fn check_op(op: Token, min_precedence: u8) -> Option<(Token, u8)> {
    let (op_precedence, is_right_assoc) = match op {
        Token::OpOr => (1, false),
        Token::OpAnd => (2, false),
        Token::OpEq | Token::OpNEq => (3, false),
        Token::OpInf | Token::OpInfEq | Token::OpSup | Token::OpSupEq => (4, false),
        Token::OpAdd | Token::OpSub => (5, false),
        Token::OpMul | Token::OpDiv | Token::OpMod => (6, false),
        Token::OpPow => (7, true),
        _ => return None,
    };
    if (op_precedence > min_precedence) || (is_right_assoc && (op_precedence == min_precedence)) {
        Some((op, op_precedence))
    } else {
        None
    }
}

// Call after DoubleColon is skipped
fn parse_namespace<'a>(
    parser: &mut Parser<'a>,
    initial: SmolStr,
) -> (Box<[SmolStr]>, Token<'a>, Span) {
    let mut namespace: Vec<SmolStr> = Vec::with_capacity(2);
    namespace.push(initial);
    loop {
        let (next_token, span) = parser.next_token();
        if let Token::Identifier(i) = next_token {
            namespace.push(SmolStr::new(i));
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
        let (next_token, s) = parser.next_token();
        if next_token == Token::DoubleColon {
            continue;
        }
        return (Box::from(namespace), next_token, s);
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
        let arg_start: u32 = parser.peek_token_start();
        args.push(parse_expr(parser));
        arg_markers.push((arg_start, parser.peek_token_start()).into());
        if parser.peek_token() == Token::Comma {
            parser.next_token();
        } else if !(parser.peek_token() == Token::RParen) {
            cold_path();
            let span = parser.peek_token_span();
            parser.error(span, ParserErr::ArgumentsMissingCommaSeparator);
        }
    }
}

fn parse_postfix_op(parser: &mut Parser<'_>, mut base: Expr, mut base_span: Span) -> Expr {
    loop {
        match parser.peek_token_opt() {
            // Index or slice
            Some(Token::LBracket) => {
                parser.next_token();
                if parser.peek_token_opt() == Some(Token::RangeDot) {
                    // slice starting at 0
                    parser.next_token();
                    let upper_bound = parse_expr(parser);
                    let end = parser
                        .next_token_expect_end(Token::RBracket, "Unmatched ']'. Invalid slice.");
                    base_span.end = end;
                    base = Expr::ArrayGetSlice(
                        Box::new(base),
                        Box::from(Expr::Int(0)),
                        Box::new(upper_bound),
                        base_span,
                    );
                } else {
                    let lower_bound = parse_expr(parser);
                    let (next_token, next_token_span) = parser.next_token();
                    if next_token == Token::RBracket {
                        // array index
                        base_span.end = next_token_span.end;
                        base =
                            Expr::ArrayGetIndex(Box::new(base), Box::new(lower_bound), base_span);
                    } else {
                        let upper_bound = parse_expr(parser);
                        let end = parser.next_token_expect_end(
                            Token::RBracket,
                            "Unmatched ']'. Invalid slice.",
                        );
                        base_span.end = end;
                        base = Expr::ArrayGetSlice(
                            Box::new(base),
                            Box::from(lower_bound),
                            Box::new(upper_bound),
                            base_span,
                        );
                    }
                }
            }
            // Struct field access or ObjfunctionCall
            Some(Token::Dot) => {
                parser.next_token();
                let (id_token, id_span) = parser.next_token();
                let Token::Identifier(id) = id_token else {
                    cold_path();
                    parser.error(
                        id_span,
                        ParserErr::UnexpectedToken(Token::Identifier(""), id_token, ""),
                    );
                };
                let peek_token = parser.peek_token_opt();
                if peek_token == Some(Token::LParen) {
                    // ObjFunctionCall
                    parser.next_token();
                    let (args, arg_markers, end) = parse_args(parser);
                    let obj_function_call = Expr::ObjFunctionCall(
                        Box::new(base),
                        args,
                        Box::new([SmolStr::new(id)]),
                        (id_span.start, end).into(),
                        base_span,
                        arg_markers,
                    );
                    base_span.end = end;
                    base = obj_function_call;
                } else if peek_token == Some(Token::DoubleColon) {
                    // ObjFunctionCall with namespace
                    parser.next_token();
                    let mut namespace: Vec<SmolStr> = Vec::with_capacity(2);
                    namespace.push(SmolStr::new(id));
                    loop {
                        let (next_token, span) = parser.next_token();
                        if let Token::Identifier(i) = next_token {
                            namespace.push(SmolStr::new(i));
                        } else {
                            cold_path();
                            parser.error(
                                span,
                                ParserErr::UnexpectedToken(Token::Identifier(""), id_token, ""),
                            );
                        }
                        let (next_token, span) = parser.next_token();
                        if next_token == Token::LParen {
                            break;
                        } else if next_token != Token::DoubleColon {
                            cold_path();
                            parser.error(
                                span,
                                ParserErr::UnexpectedTokenStr(
                                    "'(' (function call) or '::' (namespace)",
                                    next_token,
                                    "",
                                ),
                            );
                        }
                    }
                    let (args, arg_markers, end) = parse_args(parser);
                    let obj_function_call = Expr::ObjFunctionCall(
                        Box::new(base),
                        args,
                        Box::from(namespace),
                        (id_span.start, end).into(),
                        base_span,
                        arg_markers,
                    );
                    base_span.end = end;
                    base = obj_function_call;
                } else {
                    let get_struct_field =
                        Expr::GetStructField(Box::new(base), SmolStr::new(id), id_span, base_span);
                    base_span.end = id_span.end;
                    base = get_struct_field;
                }
            }
            _ => break,
        }
    }
    base
}

#[inline(always)]
fn parse_expr(parser: &mut Parser<'_>) -> Expr {
    parse_expr_with_precedence(parser, 0, true)
}

#[inline(always)]
fn parse_expr_no_struct(parser: &mut Parser<'_>) -> Expr {
    parse_expr_with_precedence(parser, 0, false)
}

// call right after peeking Token::If
fn parse_condition_block(parser: &mut Parser<'_>, start: u32) -> Expr {
    let t = parser.next_token();
    debug_assert_eq!(t.0, Token::If);
    let condition = parse_expr_no_struct(parser);
    parser.next_token_expect(Token::LBrace, "Blocks must begin with a '{'.");
    let mut output_code: Vec<Expr> = Vec::with_capacity(4);
    output_code.extend(parse_code(parser));
    let mut end = parser.next_token_expect_end(Token::RBrace, "Unmatched '}'");
    loop {
        let next_token = parser.peek_token_opt();
        if next_token != Some(Token::Else) {
            break;
        }
        parser.next_token();
        // if -> else if
        // lbrace -> else
        // else -> end
        let next_token = parser.peek_token_opt();
        if next_token == Some(Token::If) {
            parser.next_token();
            let else_if_condition = parse_expr_no_struct(parser);
            parser.next_token_expect(Token::LBrace, "Blocks must begin with a '{'.");
            let else_if_code = parse_code(parser);
            end = parser.next_token_expect_end(Token::RBrace, "Unmatched '}'");
            output_code.push(Expr::ElseIfBlock(
                Box::new(else_if_condition),
                Box::from(else_if_code),
            ));
        } else if next_token == Some(Token::LBrace) {
            parser.next_token();
            let else_code = parse_code(parser);
            end = parser.next_token_expect_end(Token::RBrace, "Unmatched '}'");
            output_code.push(Expr::ElseBlock(Box::from(else_code)));
            break;
        } else {
            break;
        }
    }
    Expr::Condition(
        Box::new(condition),
        Box::from(output_code),
        (start, end).into(),
    )
}

/// LBrace Expr RBrace
fn parse_block(input: &mut Parser<'_>) -> Vec<Expr> {
    input.next_token_expect(Token::LBrace, "Blocks need to start with '{'");
    let while_code = parse_code(input);
    input.next_token_expect_end(Token::RBrace, "Unmatched '}‘");
    while_code
}

fn parse_while_block(input: &mut Parser<'_>) -> Expr {
    let t = input.next_token();
    debug_assert_eq!(t.0, Token::While);
    let while_condition = parse_expr_no_struct(input);
    let while_code = parse_block(input);
    Expr::WhileBlock(Box::new(while_condition), Box::from(while_code))
}

/// Parses ForLoop and IntForLoop
/// for Identifier in Expr LBrace Code RBrace
/// for Identifier in Expr RangeDot Expr LBrace Code Rbrace
/// for Identifier in RangeDot Expr LBrace Code RBrace
fn parse_for_loop(parser: &mut Parser<'_>) -> Expr {
    let t = parser.next_token();
    debug_assert_eq!(t.0, Token::For);
    let (i_token, span) = parser.next_token();
    let id = if let Token::Identifier(id) = i_token {
        SmolStr::new(id)
    } else {
        cold_path();
        parser.error(
            span,
            ParserErr::UnexpectedToken(Token::Identifier(""), i_token, ""),
        );
    };
    parser.next_token_expect(Token::In, "");
    let start = parser.peek_token_start();
    let peek_token = parser.peek_token();
    if peek_token == Token::RangeDot {
        // shorthand IntForLoop
        parser.next_token();
        let start2 = parser.peek_token_start();
        let upper_bound = parse_expr_no_struct(parser);
        let end2 = parser.peek_token_end();
        let for_loop_code = parse_block(parser);
        Expr::IntForLoop(
            id,
            Box::new(Expr::Int(0)),
            Box::new(upper_bound),
            Box::from(for_loop_code),
            (start, start).into(),
            (start2, end2).into(),
        )
    } else {
        let for_collection = parse_expr_no_struct(parser);
        let end = parser.peek_token_end();
        let peek_token = parser.peek_token();
        if peek_token == RangeDot {
            parser.next_token();
            let start2 = parser.peek_token_start();
            let upper_bound = parse_expr_no_struct(parser);
            let end2 = parser.peek_token_end();
            let for_loop_code = parse_block(parser);
            Expr::IntForLoop(
                id,
                Box::new(for_collection),
                Box::new(upper_bound),
                Box::from(for_loop_code),
                (start, start).into(),
                (start2, end2).into(),
            )
        } else {
            let for_loop_code = parse_block(parser);
            Expr::ForLoop(
                id,
                Box::new(for_collection),
                Box::from(for_loop_code),
                (start, end).into(),
            )
        }
    }
}

#[inline(always)]
fn parse_eval_block(parser: &mut Parser<'_>) -> Expr {
    Expr::EvalBlock(Box::from(parse_block(parser)))
}

fn parse_function(parser: &mut Parser<'_>) -> Expr {
    let (t, Span { start, end: _ }) = parser.next_token();
    debug_assert_eq!(t, Token::Function);
    let (t_fn_id, span) = parser.next_token();
    let fn_name = if let Token::Identifier(fn_name) = t_fn_id {
        SmolStr::new(fn_name)
    } else {
        cold_path();
        parser.error(
            span,
            ParserErr::UnexpectedToken(Token::Identifier(""), t_fn_id, "Invalid function name."),
        );
    };
    parser.next_token_expect(
        Token::LParen,
        "Function arguments must be delimited by parentheses",
    );
    let mut args: Vec<SmolStr> = Vec::with_capacity(4);
    let end: u32;
    loop {
        if parser.peek_token() == Token::RParen {
            end = parser.next_token().1.end;
            break;
        }
        let (arg, span) = parser.next_token();
        if let Token::Identifier(arg) = arg {
            args.push(SmolStr::new(arg));
        } else {
            cold_path();
            parser.error(
                span,
                ParserErr::UnexpectedToken(
                    Token::Identifier(""),
                    arg,
                    "Invalid function argument.",
                ),
            );
        }
        if parser.peek_token() == Token::Comma {
            parser.next_token();
        } else if !(parser.peek_token() == Token::RParen) {
            cold_path();
            let span = parser.peek_token_span();
            parser.error(span, ParserErr::ArgumentsMissingCommaSeparator);
        }
    }
    let fn_code = parse_block(parser);
    Expr::FunctionDecl(
        fn_name,
        Box::from(args),
        std::rc::Rc::from(fn_code),
        (start, end).into(),
    )
}

fn parse_try_catch_block(parser: &mut Parser<'_>) -> Expr {
    let (t, Span { start, end: _ }) = parser.next_token();
    debug_assert_eq!(t, Token::Try);
    let try_code = parse_block(parser);
    let mut has_catch = false;
    let mut catch_blocks: Vec<(SmolStr, Vec<Expr>)> = Vec::with_capacity(1);
    let mut catch_all_var = SmolStr::new_static("e");
    let mut catch_all_code = None;
    let end: u32;
    loop {
        let token_peek = parser.peek_token();
        if token_peek != Token::Catch {
            end = parser.peek_token_end();
            break;
        }
        parser.next_token();
        let (next_token, _) = parser.next_token();
        if let Token::Identifier(i) = next_token {
            // catch-all
            catch_all_var = SmolStr::new(i);
            catch_all_code = Some(parse_block(parser));
            end = parser.peek_token_start();
            has_catch = true;
            break;
        } else if let Token::String(s) = next_token {
            catch_blocks.push((
                SmolStr::new(crate::util::parse_string(s)),
                parse_block(parser),
            ));
            has_catch = true;
        }
    }
    if !has_catch {
        cold_path();
        parser.error((start, end).into(), ParserErr::TryBlockNoCatch);
    }
    let usr_var = Expr::Var(catch_all_var.clone(), (start, end).into());
    let else_code: Box<[Expr]> = if let Some(c) = catch_all_code {
        Box::from(c)
    } else {
        Box::from([Expr::FunctionCall(
            Box::new([usr_var]),
            Box::from([SmolStr::new("throw")]),
            (start, end).into(),
            Box::from([]),
        )])
    };

    if catch_blocks.is_empty() {
        return Expr::TryCatchBlock(Box::from(try_code), catch_all_var, else_code);
    }

    let mut output_code: Vec<Expr> = Vec::with_capacity(2);
    let mut main_condition = Expr::Null;

    let mut first = true;
    for (e, c) in catch_blocks {
        if first {
            first = false;
            main_condition = Expr::Eq(
                Box::new(Expr::String(e)),
                Box::new(Expr::Var(catch_all_var.clone(), (start, end).into())),
            );
            output_code.extend(c);
        } else {
            output_code.push(Expr::ElseIfBlock(
                Box::new(Expr::Eq(
                    Box::new(Expr::String(e)),
                    Box::new(Expr::Var(catch_all_var.clone(), (start, end).into())),
                )),
                Box::from(c),
            ));
        }
    }
    output_code.push(Expr::ElseBlock(else_code));
    Expr::TryCatchBlock(
        Box::from(try_code),
        catch_all_var,
        Box::from([Expr::Condition(
            Box::from(main_condition),
            Box::from(output_code),
            (start, end).into(),
        )]),
    )
}

fn parse_struct_declare(parser: &mut Parser<'_>) -> Expr {
    let (t, Span { start, end: _ }) = parser.next_token();
    debug_assert_eq!(t, Token::Struct);
    let (next_token, span) = parser.next_token();
    let struct_name = if let Token::Identifier(id) = next_token {
        SmolStr::new(id)
    } else {
        cold_path();
        parser.error(
            span,
            ParserErr::UnexpectedToken(Token::Identifier(""), next_token, ""),
        );
    };
    parser.next_token_expect(Token::LBrace, "Expected '{'");
    let mut fields: Vec<(SmolStr, SmolStr)> = Vec::with_capacity(4);
    let end: u32;
    loop {
        let (next_token, span) = parser.next_token();
        let field_name = if let Token::Identifier(i) = next_token {
            SmolStr::new(i)
        } else {
            cold_path();
            parser.error(
                span,
                ParserErr::UnexpectedToken(
                    Token::Identifier(""),
                    next_token,
                    "Struct field names must be identifiers.",
                ),
            );
        };
        parser.next_token_expect(Token::Colon, "A colon must separate a field from its type.");
        let field_type = parse_type(parser);
        fields.push((field_name, field_type));
        let (next_token, span) = parser.next_token();
        if next_token == Token::RBrace {
            end = span.end;
            break;
        } else if next_token != Token::Comma {
            cold_path();
            parser.error(
                span,
                ParserErr::UnexpectedToken(
                    Token::Comma,
                    next_token,
                    "In structs, fields must be separated by a comma.",
                ),
            );
        } else if parser.peek_token() == Token::RBrace {
            end = parser.next_token().1.end;
            break;
        }
    }
    Expr::StructDeclare(struct_name, Box::from(fields), (start, end).into())
}

fn parse_loop_block(input: &mut Parser<'_>) -> Expr {
    let (t, _) = input.next_token();
    debug_assert_eq!(t, Token::Loop);
    Expr::LoopBlock(Box::from(parse_block(input)))
}

fn parse_match(parser: &mut Parser<'_>) -> Expr {
    let (t, Span { start, end: _ }) = parser.next_token();
    debug_assert_eq!(t, Token::Match);
    let match_obj = parse_expr_no_struct(parser);
    let obj_var = SmolStr::new_static("[MATCH TEMP]");
    parser.next_token_expect(Token::LBrace, "Blocks must be delimited by braces");
    let mut first_condition: Option<Expr> = None;
    let mut output_code: Vec<Expr> = Vec::with_capacity(2);
    let end: u32;
    loop {
        let peek_token = parser.peek_token();
        if peek_token == Identifier("_") {
            if first_condition.is_none() {
                cold_path();
                let span = (start, parser.peek_token_end()).into();
                parser.error(span, ParserErr::MatchBlockNoNonWildcardArm);
            }
            parser.next_token();
            parser.next_token_expect(Token::Arrow, "Expected '=>'");
            let code = parse_block(parser);
            end = parser.peek_token_end();
            parser.next_token_expect(
                Token::RBrace,
                "The wildcard must be the last statement in a match",
            );
            output_code.push(Expr::ElseBlock(Box::from(code)));
            break;
        } else if peek_token == Token::RBrace {
            if first_condition.is_none() {
                cold_path();
                let span = (start, parser.peek_token_end()).into();
                parser.error(span, ParserErr::MatchBlockZeroArms);
            }
            end = parser.peek_token_end();
            parser.next_token();
            break;
        } else {
            let condition = parse_expr(parser);
            let end = parser.peek_token_start();
            parser.next_token_expect(Token::Arrow, "");
            let code = parse_block(parser);
            if first_condition.is_none() {
                first_condition = Some(condition);
                output_code.extend(code);
            } else {
                output_code.push(Expr::ElseIfBlock(
                    Box::new(Expr::Eq(
                        Box::new(Expr::Var(obj_var.clone(), (start, end).into())),
                        Box::new(condition),
                    )),
                    Box::from(code),
                ));
            }
        }
    }
    Expr::EvalBlock(Box::from([
        Expr::VarDeclare(obj_var.clone(), Box::new(match_obj)),
        Expr::Condition(
            Box::from(Expr::Eq(
                Box::new(Expr::Var(obj_var, (start, end).into())),
                Box::from(first_condition.unwrap()),
            )),
            Box::from(output_code),
            (start, end).into(),
        ),
    ]))
}

fn parse_statement(parser: &mut Parser<'_>) -> Option<Expr> {
    let token = parser.peek_token();
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
    let e_end = input.peek_token_end_opt();
    let v_start = input.peek_token_start();
    let v = parse_expr(input);
    let v_end = input.peek_token_start();
    var_assign(
        e,
        v,
        (e_start, e_end.unwrap()).into(),
        (v_start, v_end).into(),
    )
}

fn parse_op_var_assign(input: &mut Parser<'_>, e: Expr, e_start: u32, op: Token<'_>) -> Expr {
    let (t, _) = input.next_token();
    debug_assert_eq!(t, op);
    let e_end = input.peek_token_end_opt();
    let v_start = input.peek_token_start();
    let v = parse_expr(input);
    let v_end = input.peek_token_start();
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
        add_op(input, op, e, v, (e_start, v_end).into()),
        (e_start, e_end.unwrap()).into(),
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
            let e_start = input.peek_token_start();
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
    input.next_token_expect(Token::SemiColon, "Lines must end with a ';'.");
    line_code
}

fn parse_code(input: &mut Parser<'_>) -> Vec<Expr> {
    let mut output: Vec<Expr> = Vec::with_capacity(2);
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
        SmolStr::new(crate::util::parse_string(s))
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
        Expr::ImportFile(path, Some(alias), (start, span.end).into())
    } else {
        Expr::ImportFile(path, None, (start, end).into())
    }
}

fn parse_type(parser: &mut Parser<'_>) -> SmolStr {
    let mut t = String::with_capacity(8);
    let (next_token, span) = parser.next_token();
    if let Token::Identifier(i) = next_token {
        t.push_str(i);
    } else {
        cold_path();
        parser.error(
            span,
            ParserErr::UnexpectedToken(Token::Identifier(""), next_token, "Invalid type."),
        );
    }
    loop {
        let peek_token = parser.peek_token();
        if peek_token == Token::LBracket {
            if t.as_bytes().last().unwrap() == &b'[' {
                cold_path();
                let span = (span.start, parser.peek_token_end()).into();
                parser.error(
                    span,
                    ParserErr::UnexpectedToken(Token::Identifier(""), next_token, "Invalid type."),
                );
            }
            t.push('[');
            parser.next_token();
        } else if peek_token == Token::RBracket {
            if t.as_bytes().last().unwrap() != &b'[' {
                cold_path();
                let span = (span.start, parser.peek_token_end()).into();
                parser.error(
                    span,
                    ParserErr::UnexpectedToken(Token::Identifier(""), next_token, "Invalid type."),
                );
            }
            t.push(']');
            parser.next_token();
        } else {
            break;
        }
    }
    t.to_smolstr()
}

fn parse_dylib_import(parser: &mut Parser<'_>) -> Expr {
    let (t, Span { start, end: _ }) = parser.next_token();
    debug_assert_eq!(t, Token::Dylib);
    let (next_token, span) = parser.next_token();
    let path = if let Token::String(s) = next_token {
        SmolStr::new(crate::util::parse_string(s))
    } else {
        cold_path();
        parser.error(
            span,
            ParserErr::UnexpectedToken(Token::String(""), next_token, "Paths must be strings."),
        );
    };
    parser.next_token_expect(Token::LBrace, "Blocks need to start with '{'.");
    let mut fn_signatures: Vec<(SmolStr, Box<[SmolStr]>, SmolStr)> = Vec::new();
    let end: u32;
    loop {
        if parser.peek_token() == Token::RBrace {
            end = parser.next_token().1.end;
            break;
        }

        let first = parse_type(parser);
        let (return_type, fn_name) = if parser.peek_token() == Token::LParen {
            (SmolStr::new_static("null"), first)
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
        let mut args: Vec<SmolStr> = Vec::with_capacity(2);
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

fn parse_file_statement(parser: &mut Parser<'_>) -> Option<Expr> {
    let peek = parser.peek_token_opt();
    match peek {
        None => None,
        Some(Token::Function) => Some(parse_function(parser)),
        Some(Token::Import) => Some(parse_file_import(parser)),
        Some(Token::Struct) => Some(parse_struct_declare(parser)),
        Some(Token::Dylib) => Some(parse_dylib_import(parser)),
        Some(unexpected) => {
            let span = parser.peek_token_span();
            parser.error(span, ParserErr::UnexpectedTokenStr("'fn' (function declaration), 'import', 'struct' (struct declaration), or 'dylib' (dynamic library import)", unexpected, "Invalid file statement."));
        }
    }
}

fn parse_file(input: &mut Parser<'_>) -> Vec<Expr> {
    let mut output: Vec<Expr> = Vec::with_capacity(2);
    while let Some(e) = parse_file_statement(input) {
        output.push(e);
    }
    output
}

impl From<std::range::Range<usize>> for Span {
    #[inline(always)]
    fn from(value: std::range::Range<usize>) -> Self {
        Self {
            start: value.start as u32,
            end: value.end as u32,
        }
    }
}

impl From<std::ops::Range<usize>> for Span {
    #[inline(always)]
    fn from(value: std::ops::Range<usize>) -> Self {
        Self {
            start: value.start as u32,
            end: value.end as u32,
        }
    }
}

impl From<(usize, usize)> for Span {
    #[inline(always)]
    fn from((start, end): (usize, usize)) -> Self {
        Self {
            start: start as u32,
            end: end as u32,
        }
    }
}

impl From<(u32, u32)> for Span {
    #[inline(always)]
    fn from((start, end): (u32, u32)) -> Self {
        Self { start, end }
    }
}

pub fn experimental_parser(input: &str, src: (&str, &str)) -> Vec<Expr> {
    parse_file(&mut Parser {
        input: Token::lexer(input).spanned().peekable(),
        ctx: ParserCtx { src },
    })
}

impl std::fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Identifier(_) => write!(f, "an identifier"),
            Token::Int(_) => write!(f, "an integer"),
            Token::Float(_) => write!(f, "a float"),
            Token::String(_) => write!(f, "a string"),
            Token::LBrace => write!(f, "'{{'"),
            Token::RBrace => write!(f, "'}}'"),
            Token::LParen => write!(f, "'('"),
            Token::RParen => write!(f, "')'"),
            Token::LBracket => write!(f, "'['"),
            Token::RBracket => write!(f, "']'"),
            Token::Arrow => write!(f, "'=>'"),
            other => write!(f, "{other:?}"),
        }
    }
}

#[derive(Logos, Debug, PartialEq, Clone, Copy)]
#[logos(skip r"[ \t\r\n\f]+")] // Ignore whitespace
#[logos(skip(r"//[^\n\r]*", allow_greedy = true))] // Ignore comments
pub enum Token<'a> {
    // ASSIGNEMENT OPS
    #[token("+=")]
    AssignOpAdd,
    #[token("-=")]
    AssignOpSub,
    #[token("*=")]
    AssignOpMul,
    #[token("/=")]
    AssignOpDiv,
    #[token("%=")]
    AssignOpMod,
    #[token("^=")]
    AssignOpPow,

    // OPS
    #[token("||")]
    OpOr,
    #[token("&&")]
    OpAnd,
    #[token("==")]
    OpEq,
    #[token("!=")]
    OpNEq,
    #[token("<=")]
    OpInfEq,
    #[token("<")]
    OpInf,
    #[token(">=")]
    OpSupEq,
    #[token(">")]
    OpSup,
    #[token("+")]
    OpAdd,
    #[token("-")]
    OpSub,
    #[token("*")]
    OpMul,
    #[token("/")]
    OpDiv,
    #[token("%")]
    OpMod,
    #[token("^")]
    OpPow,
    #[token("!")]
    OpNot,
    #[token("=")]
    Equals,
    #[token("null")]
    Null,
    #[token("false")]
    False,
    #[token("true")]
    True,
    #[token("dylib")]
    Dylib,
    #[token("import")]
    Import,
    #[token("as")]
    As,
    #[token("fn")]
    Function,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("match")]
    Match,
    #[token("while")]
    While,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("try")]
    Try,
    #[token("catch")]
    Catch,
    #[token("struct")]
    Struct,
    #[token("return")]
    Return,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("loop")]
    Loop,
    #[token("let")]
    Let,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token(",")]
    Comma,
    #[token("..")]
    RangeDot,
    #[token(".")]
    Dot,
    #[token("::")]
    DoubleColon,
    #[token(":")]
    Colon,
    #[token(";")]
    SemiColon,
    #[token("=>")]
    Arrow,

    #[regex(r#"\"(?:[^\"\\]|\\.)*\""#, |lex| lex.slice())]
    String(&'a str),

    #[regex("[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice())]
    Identifier(&'a str),

    #[regex(r"[0-9]*[.][0-9]+", |lex| {
        let slice = lex.slice();
        lexical_core::parse::<f64>(slice.as_bytes()).unwrap()
    })]
    Float(f64),

    #[regex(r"[0-9]+", |lex| {
        let slice = lex.slice();
        match lexical_core::parse::<i64>(slice.as_bytes()) {
            Ok(v) if v <= (i32::MAX as i64) => v as i32,
            Ok(2_147_483_648) => i32::MIN,
            _ => {
                cold_path();
                panic!("Invalid float");
            }
        }
    })]
    Int(i32),
}
