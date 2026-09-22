use crate::{
    cold_path,
    errors::{BLUE, RESET},
};
use bumpalo::Bump;
use logos::Logos;
use std::hint::unreachable_unchecked;

impl std::fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Token::Identifier(_) => "an identifier",
            Token::Int(_) => "an integer",
            Token::Float(_) => "a float",
            Token::String(_) => "a string",
            Token::LBrace => "'{'",
            Token::RBrace => "'}'",
            Token::LParen => "'('",
            Token::RParen => "')'",
            Token::LBracket => "'['",
            Token::RBracket => "']'",
            Token::FatArrow => "'=>'",
            Token::Arrow => "'->'",
            Token::TypeInt => "'int'",
            Token::TypeFloat => "'float'",
            Token::TypeBool => "'bool'",
            Token::TypeString => "'string'",
            Token::AssignOpAdd => "'+='",
            Token::AssignOpSub => "'-='",
            Token::AssignOpMul => "'*='",
            Token::AssignOpPow => "'^='",
            Token::AssignOpMod => "'%='",
            Token::AssignOpDiv => "'/='",
            Token::OpOr => "'||'",
            Token::Pipe => "'|'",
            Token::OpAnd => "'&&'",
            Token::OpEq => "'=='",
            Token::OpNEq => "'!='",
            Token::Equals => "'='",
            Token::OpInfEq => "'<='",
            Token::OpInf => "'<'",
            Token::OpSupEq => "'>='",
            Token::OpNot => "'!'",
            Token::OpSup => "'>'",
            Token::OpAdd => "'+'",
            Token::OpDiv => "'/'",
            Token::OpSub => "'-'",
            Token::OpMul => "'*'",
            Token::OpPow => "'^'",
            Token::OpMod => "'%'",
            Token::Null => "'null'",
            Token::True => "'true'",
            Token::False => "'false'",
            Token::Dylib => "'dylib'",
            Token::Loop => "'loop'",
            Token::Let => "'let'",
            Token::Match => "'match'",
            Token::While => "'while'",
            Token::Static => "'static'",
            Token::Import => "'import'",
            Token::If => "'if'",
            Token::Catch => "'catch'",
            Token::Struct => "'struct'",
            Token::Return => "'return'",
            Token::Break => "'break'",
            Token::Continue => "'continue'",
            Token::As => "'as'",
            Token::Else => "'else'",
            Token::Function => "'fn'",
            Token::For => "'for'",
            Token::In => "'in'",
            Token::Try => "'try'",
            Token::Comma => "','",
            Token::RangeDot => "'..'",
            Token::Dot => "'.'",
            Token::DoubleColon => "'::'",
            Token::Colon => "':'",
            Token::SemiColon => "';'",
        })
    }
}

#[derive(Logos, PartialEq, Clone, Copy, Debug)]
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
    #[token("|")]
    Pipe,
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
    #[token("static")]
    Static,
    #[token("int")]
    TypeInt,
    #[token("float")]
    TypeFloat,
    #[token("bool")]
    TypeBool,
    #[token("string")]
    TypeString,
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
    /// =>
    FatArrow,
    #[token("->")]
    /// ->
    Arrow,

    #[regex(r#"\"(?:[^\"\\]|\\.)*\""#, |lex| lex.slice())]
    String(&'a str),

    #[regex("[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice())]
    Identifier(&'a str),

    #[regex(r"[0-9]*\.[0-9]+([eE][+-]?[0-9]+)?|[0-9]+[eE][+-]?[0-9]+", |lex| {
        let slice = lex.slice();
        lexical_core::parse::<f64>(slice.as_bytes()).ok().unwrap()
    })]
    Float(f64),

    #[regex(r"[0-9]+", |lex| {
        let slice = lex.slice();
        match lexical_core::parse::<i64>(slice.as_bytes()) {
            Ok(v) if v <= (i32::MAX as i64) => v as i32,
            Ok(2_147_483_648) => i32::MIN,
            _ => {
                cold_path();
                panic!("{BLUE}{slice}{RESET} is not a valid float");
            }
        }
    })]
    Int(i32),
}

/// Strips the surrounding quotes & processes escape sequences \n \t \r \\ \" \0
pub fn parse_string<'arena>(s: &str, bump: &'arena Bump) -> bumpalo::collections::String<'arena> {
    let inner = &s[1..s.len() - 1]; // Strip the surrounding quotes

    // Return the stripped string directly if it doesn't contain any escape sequences
    let Some(first_escape) = memchr::memchr(b'\\', inner.as_bytes()) else {
        return bumpalo::collections::String::from_str_in(inner, bump);
    };
    let mut processed = bumpalo::collections::String::with_capacity_in(inner.len(), bump);

    // Find returns the first occurence, so we know that inner[..first_escape] does not contain any escape sequence
    processed.push_str(&inner[..first_escape]);
    let mut to_process = &inner[first_escape..];
    loop {
        match memchr::memchr(b'\\', to_process.as_bytes()) {
            None => {
                // there are no escape sequences left
                processed.push_str(to_process);
                break;
            }
            Some(escape_seq_idx) => {
                processed.push_str(&to_process[..escape_seq_idx]);
                let after = &to_process[escape_seq_idx + 1..];
                if after.is_empty() {
                    processed.push('\\');
                    break;
                }
                let escape_seq = after.as_bytes()[0];
                if escape_seq == b'n'
                    || escape_seq == b't'
                    || escape_seq == b'r'
                    || escape_seq == b'\\'
                    || escape_seq == b'"'
                    || escape_seq == b'0'
                {
                    processed.push(match escape_seq {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'\\' => '\\',
                        b'"' => '"',
                        b'0' => '\0',
                        _ => unsafe { unreachable_unchecked() },
                    });
                    to_process = &after[1..];
                } else {
                    // chars() is used to correctly handle multi-byte characters
                    let c = after.chars().next().unwrap();
                    processed.push('\\');
                    processed.push(c);
                    to_process = &after[c.len_utf8()..];
                }
            }
        }
    }
    processed
}
