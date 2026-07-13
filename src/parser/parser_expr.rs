use crate::cold_path;
use crate::errors::ParserErr;
use crate::expr::Expr;
use crate::expr::Span;
use crate::lexer::Token;
use crate::parser::Parser;
use crate::parser::parse_args;
use crate::term::parse_term;
use smol_strc::SmolStr;
use smol_strc::ToSmolStr;
use std::hint::unreachable_unchecked;

pub fn parse_expr_with_precedence(
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

pub fn add_op(parser: &Parser<'_>, op: Token, lhs: Expr, rhs: Expr, span: Span) -> Expr {
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
                        Expr::GetStructField(Box::new(base), SmolStr::new(id), base_span, id_span);
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
pub fn parse_expr(parser: &mut Parser<'_>) -> Expr {
    parse_expr_with_precedence(parser, 0, true)
}

#[inline(always)]
pub fn parse_expr_no_struct(parser: &mut Parser<'_>) -> Expr {
    parse_expr_with_precedence(parser, 0, false)
}
