use crate::cold_path;
use crate::errors::ParserErr;
use crate::expr::Expr;
use crate::expr::Span;
use crate::lexer::Token;
use crate::parser::Parser;
use crate::parser::parse_args;
use crate::parser::parse_namespace;
use crate::parser_expr::parse_expr;
use crate::parser_expr::parse_expr_no_struct;
use crate::parser_expr::parse_expr_with_precedence;
use smol_strc::SmolStr;
use smol_strc::ToSmolStr;

// Must be called right after LParen is skipped
// Identifier LParen Expr RParen
// Parses: Expr RParen
pub fn parse_fn_call(parser: &mut Parser<'_>, namespace: Box<[SmolStr]>, start: u32) -> Expr {
    let (args, arg_markers, end) = parse_args(parser);
    Expr::FunctionCall(args, namespace, (start, end).into(), arg_markers)
}

// Must be called right after LParen is skipped
pub fn parse_struct(parser: &mut Parser<'_>, namespace: Box<[SmolStr]>, start: u32) -> Expr {
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

pub fn parse_term(parser: &mut Parser<'_>, allow_struct: bool) -> Expr {
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
