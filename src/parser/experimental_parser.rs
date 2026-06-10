use core::range::Range;
use std::hint::{cold_path, unreachable_unchecked};

use crate::{
    experimental_parser::Token::RangeDot,
    expr::{Expr, Span},
};
use logos::{Logos, SpannedIter};
use smol_strc::{SmolStr, ToSmolStr};
use std::iter::Peekable;

trait KeelParser<'a> {
    fn next_token(&mut self) -> (Token<'a>, Span);
    fn peek_token(&mut self) -> Token<'a>;
    fn peek_token_start(&mut self) -> u32;
    fn peek_token_end(&mut self) -> u32;
    fn peek_token_end_opt(&mut self) -> Option<u32>;
    fn peek_token_opt(&mut self) -> Option<Token<'a>>;
    fn peek_span_opt(&mut self) -> Option<Span>;
    fn next_token_expect(&mut self, expected: Token, msg: &str);
    fn next_token_expect_end(&mut self, expected: Token, msg: &str) -> u32;
}

type TokenIter<'a> = Peekable<SpannedIter<'a, Token<'a>>>;

impl<'a> KeelParser<'a> for TokenIter<'a> {
    #[inline(always)]
    fn next_token(&mut self) -> (Token<'a>, Span) {
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
            (t.1.start, t.1.end).into(),
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
    fn peek_token_end(&mut self) -> u32 {
        self.peek()
            .unwrap_or_else(
                #[cold]
                || {
                    cold_path();
                    panic!("Unexpected EOF")
                },
            )
            .1
            .end as u32
    }
    #[inline(always)]
    fn peek_token_end_opt(&mut self) -> Option<u32> {
        self.peek().map(|t| t.1.end as u32)
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
    fn peek_span_opt(&mut self) -> Option<Span> {
        self.peek()
            .map(|x| (x.1.start as u32, x.1.end as u32).into())
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
        span.end
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
    let (args, arg_markers, end) = parse_args(input);
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
            end = next_token_span.end;
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
                    let (namespace, terminator) = parse_namespace(input, SmolStr::new(s));
                    if terminator == Token::LParen {
                        // FUNCTION CALL WITH NAMESPACE:
                        // (Identifier DoubleColon)+ Identifier LParen Expr RParen
                        return parse_fn_call(input, Box::from(namespace), start);
                    } else if terminator == Token::LBrace {
                        // STRUCT WITH NAMESPACE
                        return parse_struct(input, Box::from(namespace), start);
                    } else {
                        cold_path();
                        panic!("Expected LParen, LBrace, or DoubleColon, but got {terminator:?}");
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
        Token::OpSub => match parse_expr_with_precedence(input, 8) {
            Expr::Int(i) => Expr::Int(-i),
            Expr::Float(f) => Expr::Float(-f),
            other => Expr::Neg(
                Box::new(other),
                (t_span.start as u32, input.peek_token_start()).into(),
            ),
        },
        // ! Expr
        Token::OpNot => match parse_expr_with_precedence(input, 8) {
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
                    output_code.push(Expr::ElseIfBlock(
                        Box::new(else_if_condition),
                        Box::new([else_if_value]),
                    ));
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

fn parse_expr_with_precedence<'a>(input: &mut TokenIter<'a>, min_precedence: u8) -> Expr {
    let start = input.peek_token_start();
    let mut end = input.peek_token_end();
    let mut lhs = parse_term(input);
    end = input.peek_token_end_opt().unwrap_or(end);
    lhs = parse_postfix_op(input, lhs, (start, end).into());
    loop {
        let Some(peek) = input.peek_token_opt() else {
            break;
        };
        let Some((op, op_precedence)) = check_op(peek, min_precedence) else {
            break;
        };
        input.next_token();
        let end = input.peek_token_end();
        let rhs = parse_expr_with_precedence(input, op_precedence);
        lhs = add_op(op, lhs, rhs, (start, end).into());
    }
    lhs
}

fn add_op(op: Token, lhs: Expr, rhs: Expr, span: Span) -> Expr {
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
            (_, Expr::Int(0)) | (_, Expr::Float(0.0)) => {
                cold_path();
                panic!("Division by zero");
            }
            (Expr::Int(x), Expr::Int(y)) => Expr::Int(x / y),
            (Expr::Float(x), Expr::Float(y)) => Expr::Float(x / y),
            (lhs, rhs) => Expr::Div(Box::new(lhs), Box::new(rhs), span),
        },
        Token::OpMod => match (lhs, rhs) {
            (_, Expr::Int(0)) | (_, Expr::Float(0.0)) => {
                cold_path();
                panic!("Modulo by zero");
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
                    panic!("Cannot raise ints to a negative exponent")
                }
            }
            (Expr::Float(x), Expr::Float(y)) => Expr::Float(x.powf(y)),
            (Expr::Float(x), Expr::Int(y)) => Expr::Float(x.powi(y)),
            (lhs, rhs) => Expr::Pow(Box::new(lhs), Box::new(rhs), span),
        },
        _ => unsafe { unreachable_unchecked() },
    }
}

fn check_op(op: Token, min_precedence: u8) -> Option<(Token, u8)> {
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
fn parse_namespace<'a>(input: &mut TokenIter<'a>, initial: SmolStr) -> (Box<[SmolStr]>, Token<'a>) {
    let mut namespace: Vec<SmolStr> = Vec::with_capacity(2);
    namespace.push(initial);
    loop {
        let (next_token, _) = input.next_token();
        if let Token::Identifier(i) = next_token {
            namespace.push(SmolStr::new(i));
        } else {
            cold_path();
            panic!("Wrong namespace syntax");
        }
        let (next_token, _) = input.next_token();
        if next_token == Token::DoubleColon {
            continue;
        } else {
            return (Box::from(namespace), next_token);
        }
    }
}

// Must be called after LParen is skipped
fn parse_args<'a>(input: &mut TokenIter<'a>) -> (Box<[Expr]>, Box<[Span]>, u32) {
    let mut args = Vec::with_capacity(4);
    let mut arg_markers: Vec<Span> = Vec::with_capacity(4);
    loop {
        if input.peek_token() == Token::RParen {
            let end = input.next_token().1.end as u32;
            return (Box::from(args), Box::from(arg_markers), end);
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
}

fn parse_postfix_op<'a>(input: &mut TokenIter<'a>, mut base: Expr, mut base_span: Span) -> Expr {
    loop {
        match input.peek_token_opt() {
            // Index or slice
            Some(Token::LBracket) => {
                input.next_token();
                if input.peek_token_opt() == Some(Token::RangeDot) {
                    // slice starting at 0
                    input.next_token();
                    let upper_bound = parse_expr(input);
                    let end = input
                        .next_token_expect_end(Token::RBracket, "Unmatched ']'. Invalid slice.");
                    base_span.end = end;
                    base = Expr::ArrayGetSlice(
                        Box::new(base),
                        Box::from(Expr::Int(0)),
                        Box::new(upper_bound),
                        base_span,
                    );
                } else {
                    let lower_bound = parse_expr(input);
                    let (next_token, next_token_span) = input.next_token();
                    if next_token == Token::RBracket {
                        // array index
                        base_span.end = next_token_span.end as u32;
                        base =
                            Expr::ArrayGetIndex(Box::new(base), Box::new(lower_bound), base_span);
                    } else {
                        let upper_bound = parse_expr(input);
                        let end = input.next_token_expect_end(
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
                input.next_token();
                let (id_token, id_span) = input.next_token();
                let Token::Identifier(id) = id_token else {
                    cold_path();
                    panic!("Unexpected token. Expected Identifier but got {id_token:?}");
                };
                let peek_token = input.peek_token_opt();
                if peek_token == Some(Token::LParen) {
                    // ObjFunctionCall
                    input.next_token();
                    let (args, arg_markers, end) = parse_args(input);
                    let obj_function_call = Expr::ObjFunctionCall(
                        Box::new(base),
                        Box::from(args),
                        Box::new([SmolStr::new(id)]),
                        (id_span.start as u32, end).into(),
                        base_span,
                        Box::from(arg_markers),
                    );
                    base_span.end = end;
                    base = obj_function_call;
                } else if peek_token == Some(Token::DoubleColon) {
                    // ObjFunctionCall with namespace
                    input.next_token();
                    let mut namespace: Vec<SmolStr> = Vec::with_capacity(2);
                    namespace.push(SmolStr::new(id));
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
                            break;
                        } else if next_token == Token::DoubleColon {
                            continue;
                        } else {
                            cold_path();
                            panic!(
                                "Expected LParen, LBrace, or DoubleColon, but got {next_token:?}"
                            );
                        }
                    }
                    let (args, arg_markers, end) = parse_args(input);
                    let obj_function_call = Expr::ObjFunctionCall(
                        Box::new(base),
                        Box::from(args),
                        Box::from(namespace),
                        (id_span.start as u32, end).into(),
                        base_span,
                        Box::from(arg_markers),
                    );
                    base_span.end = end;
                    base = obj_function_call;
                } else {
                    let get_struct_field = Expr::GetStructField(
                        Box::new(base),
                        SmolStr::new(id),
                        id_span.into(),
                        base_span,
                    );
                    base_span.end = id_span.end as u32;
                    base = get_struct_field;
                }
            }
            _ => break,
        }
    }
    base
}

fn parse_expr<'a>(input: &mut TokenIter<'a>) -> Expr {
    parse_expr_with_precedence(input, 0)
}

// call right after peeking Token::If
fn parse_condition_block<'a>(input: &mut TokenIter<'a>, start: u32) -> Expr {
    let t = input.next_token();
    debug_assert_eq!(t.0, Token::If);
    let condition = parse_expr(input);
    input.next_token_expect(Token::LBrace, "Expected '{'");
    let mut output_code: Vec<Expr> = Vec::with_capacity(4);
    output_code.extend(parse_code(input));
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
            let else_if_code = parse_code(input);
            end = input.next_token_expect_end(Token::RBrace, "Unmatched '}'");
            output_code.push(Expr::ElseIfBlock(
                Box::new(else_if_condition),
                Box::from(else_if_code),
            ));
        } else if next_token == Some(Token::LBrace) {
            input.next_token();
            let else_code = parse_code(input);
            end = input.next_token_expect_end(Token::RBrace, "Unmatched '}'");
            output_code.push(Expr::ElseBlock(Box::from(else_code)));
            break;
        } else {
            break;
        }
    }
    return Expr::Condition(
        Box::new(condition),
        Box::from(output_code),
        (start, end).into(),
    );
}

/// LBrace Expr RBrace
fn parse_block<'a>(input: &mut TokenIter<'a>) -> Vec<Expr> {
    input.next_token_expect(Token::LBrace, "Blocks need to start with '{'");
    let while_code = parse_code(input);
    input.next_token_expect_end(Token::RBrace, "Unmatched '}‘");
    while_code
}

fn parse_while_block<'a>(input: &mut TokenIter<'a>) -> Expr {
    let t = input.next_token();
    debug_assert_eq!(t.0, Token::While);
    let while_condition = parse_expr(input);
    let while_code = parse_block(input);
    Expr::WhileBlock(Box::new(while_condition), Box::from(while_code))
}

/// Parses ForLoop and IntForLoop
/// for Identifier in Expr LBrace Code RBrace
/// for Identifier in Expr RangeDot Expr LBrace Code Rbrace
/// for Identifier in RangeDot Expr LBrace Code RBrace
fn parse_for_loop<'a>(input: &mut TokenIter<'a>) -> Expr {
    let t = input.next_token();
    debug_assert_eq!(t.0, Token::For);
    let (i_token, _) = input.next_token();
    let id = if let Token::Identifier(id) = i_token {
        SmolStr::new(id)
    } else {
        cold_path();
        panic!("Expected identifier, found {i_token:?}");
    };
    input.next_token_expect(Token::In, "Expected 'in'");
    let start = input.peek_token_start();
    let peek_token = input.peek_token();
    if peek_token == Token::RangeDot {
        // shorthand IntForLoop
        input.next_token();
        let start2 = input.peek_token_start();
        let upper_bound = parse_expr(input);
        let end2 = input.peek_token_end();
        let for_loop_code = parse_block(input);
        Expr::IntForLoop(
            id,
            Box::new(Expr::Int(0)),
            Box::new(upper_bound),
            Box::from(for_loop_code),
            (start, start).into(),
            (start2, end2).into(),
        )
    } else {
        let for_collection = parse_expr(input);
        let end = input.peek_token_end();
        let peek_token = input.peek_token();
        if peek_token == RangeDot {
            input.next_token();
            let start2 = input.peek_token_start();
            let upper_bound = parse_expr(input);
            let end2 = input.peek_token_end();
            let for_loop_code = parse_block(input);
            Expr::IntForLoop(
                id,
                Box::new(for_collection),
                Box::new(upper_bound),
                Box::from(for_loop_code),
                (start, start).into(),
                (start2, end2).into(),
            )
        } else {
            let for_loop_code = parse_block(input);
            Expr::ForLoop(
                id,
                Box::new(for_collection),
                Box::from(for_loop_code),
                (start, end).into(),
            )
        }
    }
}

fn parse_eval_block<'a>(input: &mut TokenIter<'a>) -> Expr {
    Expr::EvalBlock(Box::from(parse_block(input)))
}

fn parse_function<'a>(input: &mut TokenIter<'a>) -> Expr {
    let (t, Span { start, end: _ }) = input.next_token();
    debug_assert_eq!(t, Token::Function);
    let (t_fn_id, _) = input.next_token();
    let fn_name = if let Token::Identifier(fn_name) = t_fn_id {
        SmolStr::new(fn_name)
    } else {
        cold_path();
        panic!("Invalid function name. Expected identifier but got {t_fn_id:?}");
    };
    input.next_token_expect(
        Token::LParen,
        "Function arguments must be delimited by parentheses",
    );
    let mut args: Vec<SmolStr> = Vec::with_capacity(4);
    let end: u32;
    loop {
        if input.peek_token() == Token::RParen {
            end = input.next_token().1.end as u32;
            break;
        }
        let (arg, _) = input.next_token();
        if let Token::Identifier(arg) = arg {
            args.push(SmolStr::new(arg));
        } else {
            cold_path();
            panic!("Invalid function argument. Expected identifier but got {arg:?}")
        }
        if input.peek_token() == Token::Comma {
            input.next_token();
        } else if !(input.peek_token() == Token::RParen) {
            cold_path();
            panic!("Function arguments must be comma-separated");
        }
    }
    let fn_code = parse_block(input);
    Expr::FunctionDecl(
        fn_name,
        Box::from(args),
        std::rc::Rc::from(fn_code),
        (start, end).into(),
    )
}

fn parse_try_catch_block<'a>(input: &mut TokenIter<'a>) -> Expr {
    let (t, Span { start, end: _ }) = input.next_token();
    debug_assert_eq!(t, Token::Try);
    let mut try_code = parse_block(input);
    let mut has_catch = false;
    loop {
        let token_peek = input.peek_token();
        if token_peek != Token::Catch {
            break;
        }
        input.next_token();
        todo!();
    }
    if !has_catch {
        cold_path();
        panic!("A try block must have at least one catch")
    }
    // Expr::TryCatchBlock((), (), ())
    todo!()
}

fn parse_struct_declare<'a>(input: &mut TokenIter<'a>) -> Expr {
    let (t, Span { start, end: _ }) = input.next_token();
    debug_assert_eq!(t, Token::Struct);
    let (next_token, _) = input.next_token();
    let struct_name = if let Token::Identifier(id) = next_token {
        SmolStr::new(id)
    } else {
        cold_path();
        panic!("Struct declaration is missing a name");
    };
    input.next_token_expect(Token::LBrace, "Expected '{'");
    let mut fields: Vec<(SmolStr, SmolStr)> = Vec::with_capacity(4);
    let end: u32;
    loop {
        let (next_token, _) = input.next_token();
        let field_name = if let Token::Identifier(i) = next_token {
            SmolStr::new(i)
        } else {
            cold_path();
            panic!("Invalid struct field {next_token:?}")
        };
        input.next_token_expect(Token::Colon, "A colon must separate a field from its type.");
        let (next_token, _) = input.next_token();
        let field_type = if let Token::Identifier(i) = next_token {
            SmolStr::new(i)
        } else {
            cold_path();
            panic!("Invalid struct field {next_token:?}")
        };
        fields.push((field_name, field_type));
        let (next_token, next_token_span) = input.next_token();
        if next_token == Token::RBrace {
            end = next_token_span.end;
            break;
        } else if next_token != Token::Comma {
            cold_path();
            panic!(
                "Field-type elements must be separated by a comma. Expected comma but got {next_token:?}"
            )
        }
    }
    Expr::StructDeclare(struct_name, Box::from(fields), (start, end).into())
}

fn parse_loop_block<'a>(input: &mut TokenIter<'a>) -> Expr {
    let (t, _) = input.next_token();
    debug_assert_eq!(t, Token::Loop);
    Expr::LoopBlock(Box::from(parse_block(input)))
}

fn parse_statement<'a>(input: &mut TokenIter<'a>) -> Option<Expr> {
    let token = input.peek_token_opt();
    let t_span = input.peek_span_opt();
    match token {
        Some(Token::If) => Some(parse_condition_block(input, t_span.unwrap().start as u32)),
        Some(Token::While) => Some(parse_while_block(input)),
        Some(Token::For) => Some(parse_for_loop(input)),
        Some(Token::Match) => todo!("Match"),
        Some(Token::LBrace) => Some(parse_eval_block(input)),
        Some(Token::Function) => Some(parse_function(input)),
        Some(Token::Loop) => Some(parse_loop_block(input)),
        Some(Token::Try) => Some(parse_try_catch_block(input)),
        Some(Token::Struct) => Some(parse_struct_declare(input)),
        Some(Token::RBrace) => None,
        Some(t) => Some(parse_line(input, t)),
        None => {
            cold_path();
            panic!("")
        }
    }
}

fn parse_var_declare<'a>(input: &mut TokenIter<'a>) -> Expr {
    let (t, _) = input.next_token();
    debug_assert_eq!(t, Token::Let);
    let (t, _) = input.next_token();
    let var_name = if let Token::Identifier(id) = t {
        SmolStr::new(id)
    } else {
        cold_path();
        panic!("{t:?} is not a valid variable name");
    };
    input.next_token_expect(Token::Equals, "Variable declarations need a '='.");
    let var_value = parse_expr(input);
    Expr::VarDeclare(var_name, Box::new(var_value))
}

fn parse_line<'a>(input: &mut TokenIter<'a>, peek: Token<'a>) -> Expr {
    let line_code = match peek {
        Token::Let => parse_var_declare(input),
        _ => parse_expr(input),
    };
    input.next_token_expect(Token::Comma, "Lines need to end with a ';'.");
    line_code
}

fn parse_code<'a>(input: &mut TokenIter<'a>) -> Vec<Expr> {
    let mut output: Vec<Expr> = Vec::with_capacity(2);
    loop {
        if let Some(e) = parse_statement(input) {
            output.push(e);
        } else {
            break;
        }
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
        Self {
            start: start,
            end: end,
        }
    }
}

pub fn experimental_parser() {
    let input = r#"
        struct test {x:string,y:float}
        "#;
    let mut i = Token::lexer(input).spanned().peekable();
    let output = parse_statement(&mut i);
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
