use std::hint::cold_path;

use crate::expr::{Expr, Span};
use logos::Logos;
use smol_strc::SmolStr;
use winnow::combinator::alt;
use winnow::error::ParserError;
use winnow::stream::Stream;
use winnow::stream::{ContainsToken, TokenSlice};
use winnow::{Parser, Result};

fn parse_int(input: &mut Input<'_>) -> Result<Expr> {
    let t = input.next_token().ok_or_else(|| {
        cold_path();
        ParserError::from_input(input)
    })?;
    if let Token::Int(i) = t.token {
        Ok(Expr::Int(i))
    } else {
        cold_path();
        Err(ParserError::from_input(input))
    }
}

fn parse_float(input: &mut Input<'_>) -> Result<Expr> {
    let t = input.next_token().ok_or_else(|| {
        cold_path();
        ParserError::from_input(input)
    })?;
    if let Token::Float(i) = t.token {
        Ok(Expr::Float(i))
    } else {
        cold_path();
        Err(ParserError::from_input(input))
    }
}

fn parse_string(input: &mut Input<'_>) -> Result<Expr> {
    let t = input.next_token().ok_or_else(|| {
        cold_path();
        ParserError::from_input(input)
    })?;
    if let Token::String(i) = t.token {
        Ok(Expr::String(crate::util::parse_string(i)))
    } else {
        cold_path();
        Err(ParserError::from_input(input))
    }
}

fn parse_identifier(input: &mut Input<'_>) -> Result<Expr> {
    let t = input.next_token().ok_or_else(|| {
        cold_path();
        ParserError::from_input(input)
    })?;
    if let Token::Identifier(i) = t.token {
        Ok(Expr::Var(SmolStr::new(i), (t.start, t.end).into()))
    } else {
        cold_path();
        Err(ParserError::from_input(input))
    }
}

fn parse_bool(input: &mut Input<'_>) -> Result<Expr> {
    let t = input.next_token().ok_or_else(|| {
        cold_path();
        ParserError::from_input(input)
    })?;
    if t.token == Token::True {
        Ok(Expr::Bool(true))
    } else if t.token == Token::False {
        Ok(Expr::Bool(false))
    } else {
        cold_path();
        Err(ParserError::from_input(input))
    }
}

fn parse_term(input: &mut Input<'_>) -> Result<Expr> {
    let t = input.next_token().ok_or_else(|| {
        cold_path();
        ParserError::from_input(input)
    })?;
    match t.token {
        Token::Int(i) => Ok(Expr::Int(i)),
        Token::Float(f) => Ok(Expr::Float(f)),
        Token::Identifier(s) => Ok(Expr::Var(SmolStr::new(s), (t.start, t.end).into())),
        Token::String(s) => Ok(Expr::String(crate::util::parse_string(s))),
        Token::True => Ok(Expr::Bool(true)),
        Token::False => Ok(Expr::Bool(false)),
        Token::Null => Ok(Expr::Null),
        Token::LBracket => {
            let start = t.start;
            let end : u32;
            let mut elems = Vec::new();
                loop {
                    if input.first().is_some_and(|t| t.token == Token::RBracket) {
                        end = input.next_token().unwrap().end;
                        break;
                    }
                    elems.push(parse_expr(input)?);
                    if input.first().is_some_and(|t| t.token == Token::Comma) {
                        input.next_token();
                    }
                }
                Ok(Expr::Array(Box::from(elems), (start,end).into()))
        }
        _ => {
            cold_path();
            Err(ParserError::from_input(input))
        }
    }
}

fn parse_expr(input: &mut Input<'_>) -> Result<Expr> {
    parse_term(input)
}

impl From<(u32, u32)> for Span {
    fn from((start, end): (u32, u32)) -> Self {
        Span { start, end }
    }
}

type Input<'a> = TokenSlice<'a, KeelToken<'a>>;

#[derive(Clone, Copy, Debug)]
struct KeelToken<'a> {
    pub token: Token<'a>,
    pub start: u32,
    pub end: u32,
}

impl PartialEq for KeelToken<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(&self.token) == std::mem::discriminant(&other.token)
    }
}

impl ContainsToken<KeelToken<'_>> for KeelToken<'_> {
    fn contains_token(&self, token: KeelToken) -> bool {
        std::mem::discriminant(&self.token) == std::mem::discriminant(&token.token)
    }
}

pub fn experimental_parser() {
    let input = r#"[1"hello"3]"#;
    let tokens: Vec<KeelToken> = Token::lexer(input)
        .spanned()
        .map(|(tok, span)| KeelToken {
            token: tok.unwrap(),
            start: span.start as u32,
            end: span.end as u32,
        })
        .collect();
    dbg!(&tokens);
    let output = parse_term.parse_next(&mut TokenSlice::new(&tokens));
    dbg!(&output);
}

#[derive(Logos, Debug, PartialEq, Clone, Copy)]
#[logos(skip r"[ \t\n\f]+")] // Ignore whitespaces
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

    #[regex(r#"\"(?:[^\"\\]|\\.)*\""#, |lex|lex.slice())]
    String(&'a str),

    #[regex("[a-zA-Z_][a-zA-Z0-9_]*", |lex|lex.slice())]
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
