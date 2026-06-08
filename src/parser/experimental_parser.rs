use core::range::Range;
use std::hint::cold_path;

use crate::expr::{Expr, Span};
use logos::{Logos, SpannedIter};
use smol_strc::{SmolStr, ToSmolStr};
use std::iter::Peekable;

trait KeelParser<'a> {
    fn next_token(&mut self) -> (Token<'a>, Range<usize>);
    fn peek_token(&mut self) -> Token<'a>;
    fn peek_token_start(&mut self) -> u32;
    fn peek_token_opt(&mut self) -> Option<Token<'a>>;
    fn peek_span_opt(&mut self) -> Option<Range<usize>>;
    fn next_token_expect(&mut self, expected: Token, msg: &str);
    fn next_token_expect_end(&mut self, expected: Token, msg: &str) -> u32;
}

type TokenIter<'a> = Peekable<SpannedIter<'a, Token<'a>>>;

impl<'a> KeelParser<'a> for TokenIter<'a> {
    #[inline(always)]
    fn next_token(&mut self) -> (Token<'a>, Range<usize>) {
        let t = self.next().unwrap_or_else(
            #[cold]
            || {
                cold_path();
                panic!("Unexpected EOF")
            },
        );
        (
            t.0.unwrap_or_else(
                #[cold]
                |_| {
                    cold_path();
                    panic!("Unknown token")
                },
            ),
            t.1.into(),
        )
    }
    #[inline(always)]
    fn peek_token(&mut self) -> Token<'a> {
        self.peek()
            .unwrap_or_else(
                #[cold]
                || {
                    cold_path();
                    panic!("Unexpected EOF")
                },
            )
            .0
            .unwrap_or_else(
                #[cold]
                |_| {
                    cold_path();
                    panic!("Unknown token")
                },
            )
    }
    #[inline(always)]
    fn peek_token_start(&mut self) -> u32 {
        self.peek()
            .unwrap_or_else(
                #[cold]
                || {
                    cold_path();
                    panic!("Unexpected EOF")
                },
            )
            .1
            .start as u32
    }
    #[inline(always)]
    fn peek_token_opt(&mut self) -> Option<Token<'a>> {
        self.peek().map(|x| {
            x.0.unwrap_or_else(
                #[cold]
                |_| {
                    cold_path();
                    panic!("Unknown token")
                },
            )
        })
    }
    #[inline(always)]
    fn peek_span_opt(&mut self) -> Option<Range<usize>> {
        self.peek().map(|x| std::range::Range::from(x.1.clone()))
    }
    #[inline(always)]
    fn next_token_expect(&mut self, expected: Token, msg: &str) {
        let (next_token, _) = self.next_token();
        if next_token != expected {
            cold_path();
            panic!("{msg}. Expected {expected:?} but got {next_token:?}")
        }
    }
    #[inline(always)]
    fn next_token_expect_end(&mut self, expected: Token, msg: &str) -> u32 {
        let (next_token, span) = self.next_token();
        if next_token != expected {
            cold_path();
            panic!("{msg}. Expected {expected:?} but got {next_token:?}")
        }
        span.end as u32
    }
}

fn parse_int<'a>(input: &mut TokenIter<'a>) -> Expr {
    let (t, _) = input.next_token();
    if let Token::Int(i) = t {
        Expr::Int(i)
    } else {
        cold_path();
        panic!("Expected int, found {t:?}")
    }
}

fn parse_float<'a>(input: &mut TokenIter<'a>) -> Expr {
    let (t, _) = input.next_token();
    if let Token::Float(i) = t {
        Expr::Float(i)
    } else {
        cold_path();
        panic!("Expected float, found {t:?}")
    }
}

fn parse_string<'a>(input: &mut TokenIter<'a>) -> Expr {
    let (t, _) = input.next_token();
    if let Token::String(i) = t {
        Expr::String(crate::util::parse_string(i))
    } else {
        cold_path();
        panic!("Expected string, found {t:?}")
    }
}

fn parse_identifier<'a>(input: &mut TokenIter<'a>) -> Expr {
    let (t, s) = input.next_token();
    if let Token::Identifier(i) = t {
        return Expr::Var(SmolStr::new(i), Span::from(s));
    } else {
        cold_path();
        panic!("Expected identifier, found {t:?}")
    }
}

fn parse_bool<'a>(input: &mut TokenIter<'a>) -> Expr {
    let (t, _) = input.next_token();
    if t == Token::True {
        Expr::Bool(true)
    } else if t == Token::False {
        Expr::Bool(false)
    } else {
        cold_path();
        panic!("Expected bool, found {t:?}")
    }
}

// Must be called right after LParen is skipped
// Identifier LParen Expr RParen
// Parses: Expr RParen
fn parse_fn_call<'a>(input: &mut TokenIter<'a>, namespace: Box<[SmolStr]>, start: u32) -> Expr {
    let mut args = Vec::with_capacity(4);
    let mut arg_markers: Vec<Span> = Vec::with_capacity(4);
    let end: u32;
    loop {
        if input.peek_token() == Token::RParen {
            end = input.next_token().1.end as u32;
            break;
        }
        let arg_start: u32 = input.peek_token_start();
        args.push(parse_expr(input));
        arg_markers.push((arg_start, input.peek_token_start()).into());
        if input.peek_token() == Token::Comma {
            input.next_token();
        } else if !(input.peek_token() == Token::RParen) {
            cold_path();
            panic!("Function arguments must be comma-separated");
        }
    }
    Expr::FunctionCall(
        Box::from(args),
        namespace,
        (start, end).into(),
        Box::from(arg_markers),
    )
}

// Must be called right after LParen is skipped
fn parse_struct<'a>(input: &mut TokenIter<'a>, namespace: Box<[SmolStr]>, start: u32) -> Expr {
    let mut fields: Vec<(SmolStr, Expr, Span)> = Vec::with_capacity(4);
    let end: u32;
    loop {
        let (next_token, _) = input.next_token();
        let field_name = if let Token::Identifier(i) = next_token {
            SmolStr::new(i)
        } else {
            cold_path();
            panic!("Invalid struct field {next_token:?}")
        };
        input.next_token_expect(
            Token::Colon,
            "A colon must separate a field from its value.",
        );
        let field_start: u32 = input.peek_token_start();
        let field_value = parse_expr(input);
        fields.push((
            field_name,
            field_value,
            (field_start, input.peek_token_start()).into(),
        ));
        let (next_token, next_token_span) = input.next_token();
        if next_token == Token::RBrace {
            end = next_token_span.end as u32;
            break;
        } else if next_token != Token::Comma {
            cold_path();
            panic!(
                "Field-value elements must be separated by a comma. Expected comma but got {next_token:?}"
            )
        }
    }

    Expr::Struct(namespace, Box::from(fields), (start, end).into())
}

fn parse_term<'a>(input: &mut TokenIter<'a>) -> Expr {
    let (t, t_span) = input.next_token();
    match t {
        Token::Int(i) => Expr::Int(i),
        Token::Float(f) => Expr::Float(f),
        Token::String(s) => Expr::String(crate::util::parse_string(s)),
        Token::True => Expr::Bool(true),
        Token::False => Expr::Bool(false),
        Token::Null => Expr::Null,
        Token::Identifier(s) => {
            let start = t_span.start as u32;
            match input.peek_token_opt() {
                // FUNCTION CALL:
                // Identifier LParen Expr RParen
                Some(Token::LParen) => {
                    input.next_token();
                    parse_fn_call(input, Box::new([s.to_smolstr()]), start)
                }
                // STRUCT
                Some(Token::LBrace) => {
                    input.next_token();
                    parse_struct(input, Box::from([SmolStr::new(s)]), start)
                }
                // NAMESPACE
                Some(Token::DoubleColon) => {
                    input.next_token();
                    let mut namespace: Vec<SmolStr> = Vec::with_capacity(2);
                    namespace.push(SmolStr::new(s));
                    loop {
                        let (next_token, _) = input.next_token();
                        if let Token::Identifier(i) = next_token {
                            namespace.push(SmolStr::new(i));
                        } else {
                            cold_path();
                            panic!("Wrong namespace syntax");
                        }
                        let (next_token, _) = input.next_token();
                        if next_token == Token::LParen {
                            // FUNCTION CALL WITH NAMESPACE:
                            // (Identifier DoubleColon)+ Identifier LParen Expr RParen
                            return parse_fn_call(input, Box::from(namespace), start);
                        } else if next_token == Token::LBrace {
                            // STRUCT WITH NAMESPACE
                            return parse_struct(input, Box::from(namespace), start);
                        } else if next_token == Token::DoubleColon {
                            continue;
                        } else {
                            cold_path();
                            panic!(
                                "Expected LParen, LBrace, or DoubleColon, but got {next_token:?}"
                            );
                        }
                    }
                }
                _ => Expr::Var(SmolStr::new(s), (t_span.start, t_span.end).into()),
            }
        }
        Token::LBracket => {
            let start = t_span.start as u32;
            let end: u32;
            let mut elems = Vec::with_capacity(4);
            loop {
                if input.peek_token_opt() == Some(Token::RBracket) {
                    end = input.next_token().1.end as u32;
                    break;
                }
                elems.push(parse_expr(input));
                if input.peek_token_opt() == Some(Token::Comma) {
                    input.next_token();
                } else if !(input.peek_token_opt() == Some(Token::RBracket)) {
                    cold_path();
                    panic!("Array elements must be comma-separated");
                }
            }
            Expr::Array(Box::from(elems), (start, end).into())
        }
        // LParen Expr RParen
        Token::LParen => {
            let v = parse_expr(input);
            input.next_token_expect(Token::RParen, "Unmatched ')'");
            v
        }
        // - Expr
        Token::OpSub => match parse_expr(input) {
            Expr::Int(i) => Expr::Int(-i),
            Expr::Float(f) => Expr::Float(-f),
            other => Expr::Neg(
                Box::new(other),
                (t_span.start as u32, input.peek_token_start()).into(),
            ),
        },
        // ! Expr
        Token::OpNot => match parse_expr(input) {
            Expr::Bool(b) => Expr::Bool(!b),
            other => Expr::BoolNeg(
                Box::new(other),
                (t_span.start as u32, input.peek_token_start()).into(),
            ),
        },
        // Inline condition
        Token::If => {
            let condition = parse_expr(input);
            input.next_token_expect(Token::LBrace, "Expected '{'");
            let mut output_code: Vec<Expr> = Vec::with_capacity(2);
            output_code.push(parse_expr(input));
            let mut end = input.next_token_expect_end(Token::RBrace, "Unmatched '}'");
            loop {
                let next_token = input.peek_token_opt();
                if next_token != Some(Token::Else) {
                    break;
                }
                input.next_token();
                // if -> else if
                // lbrace -> else
                // else -> end
                let next_token = input.peek_token_opt();
                if next_token == Some(Token::If) {
                    input.next_token();
                    let else_if_condition = parse_expr(input);
                    input.next_token_expect(Token::LBrace, "Expected '{'");
                    let else_if_value = parse_expr(input);
                    end = input.next_token_expect_end(Token::RBrace, "Unmatched '}'");
                    output_code.push(Expr::ElseIfBlock(Box::new(else_if_condition), Box::new([else_if_value])));
                } else if next_token == Some(Token::LBrace) {
                    input.next_token();
                    let else_value = parse_expr(input);
                    end = input.next_token_expect_end(Token::RBrace, "Unmatched '}'");
                    output_code.push(Expr::ElseBlock(Box::new([else_value])));
                    break;
                } else {
                    break;
                }
            }
            if !matches!(output_code.last().unwrap(), Expr::ElseBlock(_)) {
                cold_path();
                panic!("Inline conditions need an else block");
            }
            return Expr::InlineCondition(
                Box::new(condition),
                Box::from(output_code),
                (t_span.start as u32, end).into(),
            );
        }
        unexpected => {
            cold_path();
            panic!("Expected term, found {unexpected:?}")
        }
    }
}

fn parse_expr<'a>(input: &mut TokenIter<'a>) -> Expr {
    parse_term(input)
}

impl From<std::range::Range<usize>> for Span {
    #[inline(always)]
    fn from(value: std::range::Range<usize>) -> Self {
        Span {
            start: value.start as u32,
            end: value.end as u32,
        }
    }
}

impl From<(usize, usize)> for Span {
    #[inline(always)]
    fn from((start, end): (usize, usize)) -> Self {
        Span {
            start: start as u32,
            end: end as u32,
        }
    }
}

impl From<(u32, u32)> for Span {
    #[inline(always)]
    fn from((start, end): (u32, u32)) -> Self {
        Span {
            start: start,
            end: end,
        }
    }
}

pub fn experimental_parser() {
    let input = r#"
        print(if true {"yes"} else if false {"no"} else {"false"})
        "#;
    let mut i = Token::lexer(input).spanned().peekable();
    let output = parse_expr(&mut i);
    dbg!(output);
}

#[derive(Logos, Debug, PartialEq, Clone, Copy)]
#[logos(skip r"[ \t\n\f]+")] // Ignore whitespace
#[logos(skip(r"//[^\n\r]*", allow_greedy = true))] // Ignore comments
enum Token<'a> {
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
        lexical_core::parse::<i32>(slice.as_bytes()).unwrap()
    })]
    Int(i32),
}
