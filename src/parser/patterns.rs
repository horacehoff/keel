use super::ParserErr;
use super::lexer::Token;
use super::lexer::parse_string;
use crate::cold_path;
use crate::compiler::expr::ArrayPattern;
use crate::compiler::expr::Expr;
use crate::compiler::expr::Pattern;
use crate::compiler::expr::PatternField;
use crate::compiler::expr::QualifiedName;
use crate::compiler::expr::Span;
use crate::compiler::expr::StructPattern;
use crate::parser::Parser;
use crate::parser::parse_type;

pub fn parse_pattern<'arena>(parser: &mut Parser<'arena>) -> Pattern<'arena> {
    let start = parser.peek_token_span().start;
    let mut options = vec![parse_typed_pattern(parser)];
    while parser.peek_token() == Token::Pipe {
        parser.next_token();
        options.push(parse_typed_pattern(parser));
    }
    if options.len() == 1 {
        return options[0];
    }
    Pattern::Or(parser.bump.alloc_slice_copy(&options), (start, parser.last_token_end).into())
}

fn parse_typed_pattern<'arena>(parser: &mut Parser<'arena>) -> Pattern<'arena> {
    let pattern = parse_main_pattern(parser);
    if parser.peek_token() != Token::Colon {
        return pattern;
    }
    parser.next_token();
    let t = parse_type(parser);
    Pattern::Typed(parser.bump.alloc(pattern), t)
}

fn parse_main_pattern<'arena>(parser: &mut Parser<'arena>) -> Pattern<'arena> {
    let (token, span) = parser.next_token();
    match token {
        Token::Identifier("_") => Pattern::Wildcard,
        Token::Identifier(ident) => parse_binding_pattern(parser, ident, span),
        Token::LBracket => parse_array_pattern(parser),
        Token::LParen => {
            let pattern = parse_pattern(parser);
            parser.next_token_expect(Token::RParen, "");
            pattern
        }
        _ => parse_constant_pattern(parser, token, span),
    }
}

fn parse_binding_pattern<'arena>(
    parser: &mut Parser<'arena>,
    initial: &'arena str,
    initial_span: Span,
) -> Pattern<'arena> {
    let mut path = vec![initial];
    while parser.peek_token() == Token::DoubleColon {
        parser.next_token();
        let (token, span) = parser.next_token();
        let Token::Identifier(path_part) = token else {
            parser.error(span, ParserErr::UnexpectedToken(Token::Identifier(""), token, ""));
        };
        path.push(path_part);
    }
    let peek_token = parser.peek_token();
    if peek_token == Token::LBrace {
        let qualified_name = QualifiedName::new(&path, parser.bump);
        return parse_struct_pattern(parser, qualified_name, initial_span.start);
    }
    if path.len() > 1 {
        let err_span = parser.peek_token_span();
        parser.error(
            err_span,
            ParserErr::UnexpectedToken(
                Token::LBrace,
                peek_token,
                "Paths in patterns must name a struct.",
            ),
        );
    }
    // let sub_pattern = if next == Token::At {
    //     parser.next_token();
    //     // let sub_pattern = parse_
    // };
    // let sub_pattern = None;
    let sub_pattern = if peek_token == Token::At {
        parser.next_token();
        Some(&*parser.bump.alloc(parse_main_pattern(parser)))
    } else {
        None
    };
    Pattern::Binding(initial, sub_pattern, (initial_span.start, parser.last_token_end).into())
}

fn parse_struct_pattern<'arena>(
    parser: &mut Parser<'arena>,
    qualified_name: QualifiedName<'arena>,
    start: u32,
) -> Pattern<'arena> {
    parser.next_token_expect(Token::LBrace, "");
    let mut fields: Vec<PatternField> = Vec::with_capacity(4);
    let mut fill_the_rest = false;
    while parser.peek_token() != Token::RBrace {
        let (token, span) = parser.next_token();
        match token {
            Token::RangeDot => {
                fill_the_rest = true;
                break;
            }
            Token::Identifier(name) => {
                let pattern = if parser.peek_token() == Token::Colon {
                    parser.next_token();
                    parse_pattern(parser)
                } else {
                    Pattern::Binding(name, None, span)
                };
                fields.push(PatternField {
                    name,
                    pattern,
                    span: (span.start, parser.last_token_end).into(),
                });
                if parser.peek_token() != Token::RBrace {
                    parser.next_token_expect(
                        Token::Comma,
                        "Struct pattern fields must be separated by a comma",
                    );
                }
            }
            other => {
                cold_path();
                parser.error(
                    span,
                    ParserErr::UnexpectedTokenStr("a field name, '..', or '}'", other, ""),
                );
            }
        }
    }
    let end = parser
        .next_token_expect(Token::RBrace, "'..' can only be the last element of a struct pattern")
        .end;
    Pattern::Struct(StructPattern {
        qualified_name,
        fields: parser.bump.alloc_slice_copy(&fields),
        fill_the_rest,
        span: (start, end).into(),
    })
}

fn parse_array_pattern<'arena>(parser: &mut Parser<'arena>) -> Pattern<'arena> {
    let mut prefix: Vec<Pattern> = Vec::with_capacity(4);
    let mut suffix: Vec<Pattern> = Vec::new();
    let mut rest: Option<&Pattern> = None;
    while parser.peek_token() != Token::RBracket {
        if parser.peek_token() == Token::RangeDot {
            let (_, range_dot_span) = parser.next_token();
            if rest.is_some() {
                cold_path();
                parser.error(
                    range_dot_span,
                    ParserErr::UnexpectedTokenStr(
                        "a pattern",
                        Token::RangeDot,
                        "Array patterns can only contain one '..'",
                    ),
                );
            }
            let rest_pattern = match parser.peek_token() {
                Token::Identifier(name) if name != "_" => {
                    Pattern::Binding(name, None, parser.next_token().1)
                }
                _ => Pattern::Wildcard,
            };
            rest = Some(parser.bump.alloc(rest_pattern));
        } else if rest.is_some() {
            suffix.push(parse_pattern(parser));
        } else {
            prefix.push(parse_pattern(parser));
        }
        if parser.peek_token() != Token::RBracket {
            parser.next_token_expect(
                Token::Comma,
                "Array pattern elements must be separated by a comma.",
            );
        }
    }
    parser.next_token();
    Pattern::Array(ArrayPattern {
        prefix: parser.bump.alloc_slice_copy(&prefix),
        rest,
        suffix: parser.bump.alloc_slice_copy(&suffix),
    })
}

/// Parses a single literal or a range
fn parse_constant_pattern<'arena>(
    parser: &mut Parser<'arena>,
    token: Token<'arena>,
    span: Span,
) -> Pattern<'arena> {
    let potential_lower_bound = parse_pattern_constant(parser, token, span);
    let is_inclusive = match parser.peek_token() {
        Token::RangeDot => false,
        Token::RangeDotEq => true,
        _ => return Pattern::Constant(parser.bump.alloc(potential_lower_bound)),
    };
    parser.next_token();
    let upper_bound = if is_inclusive
        || matches!(parser.peek_token(), Token::Int(_) | Token::Float(_) | Token::OpSub)
    {
        let (upper_bound_token, upper_bound_span) = parser.next_token();
        Some(parse_pattern_constant(parser, upper_bound_token, upper_bound_span))
    } else {
        None
    };
    if !matches!(potential_lower_bound, Expr::Int(_) | Expr::Float(_))
        || upper_bound.is_some_and(|upper_bound| {
            core::mem::discriminant(&upper_bound) != core::mem::discriminant(&potential_lower_bound)
        })
    {
        cold_path();
        parser.error((span.start, parser.last_token_end).into(), ParserErr::InvalidRangePattern);
    }
    Pattern::Range {
        lower_bound: parser.bump.alloc(potential_lower_bound),
        upper_bound: upper_bound.map(|upper_bound| &*parser.bump.alloc(upper_bound)),
        is_inclusive,
    }
}

fn parse_pattern_constant<'arena>(
    parser: &mut Parser<'arena>,
    token: Token<'arena>,
    span: Span,
) -> Expr<'arena> {
    match token {
        Token::Int(i) => parser.parse_int(i, false, span),
        Token::Float(f) => Expr::Float(f),
        Token::String(s) => Expr::String(parse_string(s, parser.bump).into_bump_str()),
        Token::True => Expr::Bool(true),
        Token::False => Expr::Bool(false),
        Token::Null => Expr::Null,
        Token::OpSub => {
            let (num, num_span) = parser.next_token();
            match num {
                Token::Int(i) => parser.parse_int(i, true, num_span),
                Token::Float(f) => Expr::Float(-f),
                other => {
                    cold_path();
                    parser.error(num_span, ParserErr::UnexpectedTokenStr("a number", other, ""))
                }
            }
        }
        other => {
            cold_path();
            parser.error(span, ParserErr::UnexpectedTokenStr("a pattern", other, ""))
        }
    }
}
