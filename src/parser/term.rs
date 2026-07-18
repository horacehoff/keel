use super::ParserErr;
use super::lexer::Token;
use super::lexer::parse_string;
use super::parse_expr;
use super::parser_expr;
use super::parser_expr::parse_expr_no_struct;
use super::parser_expr::parse_expr_with_precedence;
use crate::cold_path;
use crate::compiler::expr::Expr;
use crate::compiler::expr::Span;
use crate::parser::Parser;
use crate::parser::blocks::parse_block;
use crate::parser::blocks::parse_block_expr;
use crate::parser::parse_args;
use crate::parser::parse_namespace;
use smol_strc::SmolStr;
use smol_strc::ToSmolStr;

// Must be called right after LParen is skipped
// Identifier LParen Expr RParen
// Parses: Expr RParen
fn parse_fn_call(parser: &mut Parser<'_>, namespace: Box<[SmolStr]>, span: Span) -> Expr {
    let (args, arg_markers, _) = parse_args(parser);
    Expr::FunctionCall(args, namespace, span, arg_markers)
}

// Must be called right after LParen is skipped
fn parse_struct(parser: &mut Parser<'_>, namespace: Box<[SmolStr]>, start: u32) -> Expr {
    let mut fields: Vec<(SmolStr, Expr, Span, Span)> = Vec::with_capacity(4);
    let end: u32;
    loop {
        let (next_token, field_name_span) = parser.next_token();
        let field_name = if let Token::Identifier(i) = next_token {
            SmolStr::new(i)
        } else {
            cold_path();
            parser.error(
                field_name_span,
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
        let field_start: u32 = parser.peek_token_span().start;
        let field_value = parse_expr(parser);
        fields.push((
            field_name,
            field_value,
            field_name_span,
            (field_start, parser.peek_token_span().start).into(),
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
        Token::String(s) => Expr::String(parse_string(s)),
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
                    parse_fn_call(parser, Box::new([s.to_smolstr()]), t_span)
                }
                // STRUCT
                Some(Token::LBrace) if allow_struct => {
                    parser.next_token();
                    parse_struct(parser, Box::from([SmolStr::new(s)]), start)
                }
                // NAMESPACE
                Some(Token::DoubleColon) => {
                    parser.next_token();
                    let (namespace, end) = parse_namespace(parser, SmolStr::new(s));
                    let (next_token, _) = parser.next_token();
                    if next_token == Token::LParen {
                        // FUNCTION CALL WITH NAMESPACE:
                        // (Identifier DoubleColon)+ Identifier LParen Expr RParen
                        parse_fn_call(parser, namespace, (t_span.start, end).into())
                    } else if next_token == Token::LBrace {
                        // STRUCT WITH NAMESPACE
                        parse_struct(parser, namespace, start)
                    } else {
                        cold_path();
                        parser.error(
                            (t_span.start, end).into(),
                            ParserErr::UnexpectedTokenStr(
                                "'(' (function call), '{' (struct), or '::' (namespace)",
                                next_token,
                                "",
                            ),
                        );
                    }
                }
                _ => Expr::Var(SmolStr::new(s), (t_span.start, t_span.end).into()),
            }
        }
        Token::LBracket => {
            let mut elem_spans: Vec<Span> = Vec::with_capacity(2);
            elem_spans.push((t_span.start, 0u32).into());
            let mut elems = Vec::with_capacity(4);
            loop {
                if parser.peek_token_opt() == Some(Token::RBracket) {
                    elem_spans[0].end = parser.next_token().1.end;
                    break;
                }
                let start = parser.peek_token_span().start;
                elems.push(parse_expr(parser));
                let end = parser.peek_token_span().end - 1;
                elem_spans.push((start, end).into());
                if parser.peek_token_opt() == Some(Token::Comma) {
                    parser.next_token();
                } else if !(parser.peek_token_opt() == Some(Token::RBracket)) {
                    cold_path();
                    let span = unsafe { parser.peek_token_opt_span().unwrap_unchecked() };
                    parser.error(span, ParserErr::ArrayElementsMissingComma);
                }
            }
            Expr::Array(Box::from(elems), Box::from(elem_spans))
        }
        // LParen Expr RParen
        Token::LParen => {
            let v = parse_expr(parser);
            parser.next_token_expect(Token::RParen, "Unmatched ')'");
            v
        }
        // - Expr
        Token::OpSub => {
            let expr_start = parser.peek_token_span().start;
            match parse_expr_with_precedence(parser, 8, allow_struct) {
                Expr::Int(i) => Expr::Int(i.wrapping_neg()),
                Expr::Float(f) => Expr::Float(-f),
                other => Expr::Neg(
                    Box::new(other),
                    (t_span.start, parser.peek_token_span().start).into(),
                    (expr_start, parser.peek_token_span().start).into(),
                ),
            }
        }
        // ! Expr
        Token::OpNot => {
            let expr_start = parser.peek_token_span().start;
            match parse_expr_with_precedence(parser, 8, allow_struct) {
                Expr::Bool(b) => Expr::Bool(!b),
                other => Expr::BoolNeg(
                    Box::new(other),
                    (t_span.start, parser.peek_token_span().start).into(),
                    (expr_start, parser.peek_token_span().start).into(),
                ),
            }
        }
        // Inline condition
        Token::If => {
            let condition = parse_expr_no_struct(parser);
            let mut output_code: Vec<Expr> = Vec::with_capacity(2);
            output_code.push(parse_block_expr(parser));
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
                    parser.next_token_expect(Token::RBrace, "Unmatched '}'");
                    output_code.push(Expr::ElseIfBlock(
                        Box::new(else_if_condition),
                        Box::new([else_if_value]),
                    ));
                } else if next_token == Some(Token::LBrace) {
                    parser.next_token();
                    let else_value = parse_expr(parser);
                    parser.next_token_expect(Token::RBrace, "Unmatched '}'");
                    output_code.push(Expr::ElseBlock(Box::new([else_value])));
                    break;
                } else {
                    break;
                }
            }
            if !matches!(output_code.last().unwrap(), Expr::ElseBlock(_)) {
                cold_path();
                parser.error(
                    (t_span.start, parser.last_token_end as u32).into(),
                    ParserErr::InlineConditionNoElseBlock,
                );
            }
            Expr::InlineCondition(
                Box::new(condition),
                Box::from(output_code),
                (t_span.start, parser.last_token_end as u32).into(),
            )
        }
        // anonymous function
        Token::Function => {
            parser.error(
                t_span,
                ParserErr::UnexpectedTokenStr(
                    "Term",
                    Token::Function,
                    "Higher order functions are still WIP.",
                ),
            );
            let start = t_span.start;
            parser.next_token_expect(
                Token::LParen,
                "Function arguments must be delimited by parentheses",
            );
            let mut args: Vec<SmolStr> = Vec::with_capacity(2);
            // let mut args: Vec<(SmolStr, SmolStr)> = Vec::with_capacity(2);
            loop {
                let (next_token, next_token_span) = parser.next_token();
                let Token::Identifier(arg_name) = next_token else {
                    cold_path();
                    parser.error(
                        next_token_span,
                        ParserErr::UnexpectedToken(Token::Identifier(""), next_token, ""),
                    );
                };
                // parser.next_token_expect(
                //     Token::Colon,
                //     "Argument names and types must be separated by a colon",
                // );
                // let arg_type = parse_type(parser);
                args.push(SmolStr::new(arg_name));
                if parser.peek_token() != Token::Comma {
                    break;
                }
                parser.next_token();
            }
            parser.next_token_expect(
                Token::RParen,
                "Function arguments must be delimited by parentheses",
            );
            let next_token = parser.peek_token_opt();
            // if next_token == Token::Arrow {
            //     // return type
            //     let return_type = parse_type(parser);
            //     parser.next_token_expect(Token::LBrace, "Blocks must begin with a '{'.");
            //     let fn_code = parse_code(parser);
            //     let end = parser.next_token_expect_end(Token::RBrace, "Unmatched '}'");
            //     Expr::AnonymousFunction(
            //         Box::from(args),
            //         Box::from(fn_code),
            //         (start, end).into(),
            //     )
            // } else
            if next_token == Some(Token::LBrace) {
                // returns null
                // let return_type = SmolStr::new_static("null");
                let fn_code = parse_block(parser);
                Expr::AnonymousFunction(
                    Box::from(args),
                    Box::from(fn_code),
                    (start, parser.last_token_end as u32).into(),
                )
            } else {
                parser.error(
                    parser.peek_token_span(),
                    ParserErr::UnexpectedTokenStr(
                        "'->' (return type) OR '{' (function code block)",
                        next_token.unwrap(),
                        "",
                    ),
                );
            }
        }
        // map
        Token::LBrace => {
            let mut kv_pairs: Vec<(Expr, Expr)> = Vec::with_capacity(2);
            let end: u32;
            loop {
                let key = parse_term(parser, allow_struct);
                parser.next_token_expect(
                    Token::Colon,
                    "Key-value pairs must be separated by a colon",
                );
                let value = parser_expr::parse_expr(parser);
                kv_pairs.push((key, value));
                let peek_token = parser.peek_token();
                if peek_token == Token::Comma {
                    parser.next_token();
                } else if peek_token == Token::RBrace {
                    end = parser.next_token().1.end;
                    break;
                } else {
                    let span = parser.peek_token_span();
                    parser.error(
                        span,
                        ParserErr::UnexpectedTokenStr(
                            "',' (another key-value pair) or '}' (end of map)",
                            peek_token,
                            "",
                        ),
                    )
                }
            }
            Expr::Map(Box::from(kv_pairs), (t_span.start, end).into())
        }
        unexpected => {
            cold_path();
            parser.error(
                t_span,
                ParserErr::UnexpectedTokenStr("Term", unexpected, ""),
            );
        }
    }
}
