use super::ParserErr;
use super::lexer::Token;
use super::lexer::parse_string;
use super::parser_expr::parse_expr;
use super::parser_expr::parse_expr_no_struct;
use crate::cold_path;
use crate::compiler::expr::Expr;
use crate::compiler::expr::FunctionCallExpr;
use crate::compiler::expr::FunctionDeclarationArgumentExpr;
use crate::compiler::expr::FunctionDeclarationExpr;
use crate::compiler::expr::IfBlockExpr;
use crate::compiler::expr::IntForLoopExpr;
use crate::compiler::expr::MatchArm;
use crate::compiler::expr::MatchExpr;
use crate::compiler::expr::Pattern;
use crate::compiler::expr::PatternConstructor;
use crate::compiler::expr::PatternConstructorField;
use crate::compiler::expr::QualifiedName;
use crate::compiler::expr::Span;
use crate::parser::Parser;
use crate::parser::TypeExpr;
use crate::parser::parse_code;
use crate::parser::parse_type;

// call right after peeking Token::If
pub fn parse_if_block<'arena>(parser: &mut Parser<'arena>, start: u32) -> Expr<'arena> {
    let t = parser.next_token();
    debug_assert_eq!(t.0, Token::If);
    let condition_start = parser.peek_token_span().start;
    let condition = parse_expr_no_struct(parser);
    let condition_end = parser.last_token_end;
    let output_code = parse_block(parser);
    let otherwise: Box<[Expr]> = if parser.peek_token_opt() == Some(Token::Else) {
        parser.next_token();
        match parser.peek_token_opt() {
            Some(Token::If) => Box::new([parse_if_block(parser, start)]),
            Some(Token::LBrace) => parse_block(parser).into_boxed_slice(),
            _ => Box::new([]),
        }
    } else {
        Box::new([])
    };
    Expr::IfBlock(IfBlockExpr {
        condition: parser.bump.alloc(condition),
        then: parser.bump.alloc_slice_copy(&output_code),
        otherwise: parser.bump.alloc_slice_copy(&otherwise),
        condition_span: (condition_start, condition_end).into(),
        span: (start, parser.last_token_end).into(),
    })
}

/// `LBrace Code RBrace`
#[inline(always)]
pub fn parse_block<'arena>(parser: &mut Parser<'arena>) -> Vec<Expr<'arena>> {
    let opener_token_span =
        parser.next_token_expect(Token::LBrace, "Blocks need to start with '{'");
    let code = parse_code(parser);
    parser.next_token_expect_closer(Token::LBrace, opener_token_span, Token::RBrace);
    code
}

/// `LBrace Expr RBrace`
#[inline(always)]
pub fn parse_block_expr<'arena>(parser: &mut Parser<'arena>) -> Expr<'arena> {
    let opener_token_span =
        parser.next_token_expect(Token::LBrace, "Blocks need to start with '{'");
    let code = parse_expr(parser);
    parser.next_token_expect_closer(Token::LBrace, opener_token_span, Token::RBrace);
    code
}

pub fn parse_while_block<'arena>(parser: &mut Parser<'arena>) -> Expr<'arena> {
    let t = parser.next_token();
    debug_assert_eq!(t.0, Token::While);
    let while_condition = parse_expr_no_struct(parser);
    let while_code = parse_block(parser);
    Expr::WhileBlock(parser.bump.alloc(while_condition), parser.bump.alloc_slice_copy(&while_code))
}

/// Parses `ForLoop` and `IntForLoop`
pub fn parse_for_loop<'arena>(parser: &mut Parser<'arena>) -> Expr<'arena> {
    let t = parser.next_token();
    debug_assert_eq!(t.0, Token::For);
    let (i_token, span) = parser.next_token();
    let Token::Identifier(id) = i_token else {
        cold_path();
        parser.error(span, ParserErr::UnexpectedToken(Token::Identifier(""), i_token, ""));
    };
    parser.next_token_expect(Token::In, "");
    let start = parser.peek_token_span().start;
    let peek_token = parser.peek_token();
    if peek_token == Token::RangeDot {
        // shorthand IntForLoop
        parser.next_token();

        let mut code: Vec<Expr> = Vec::with_capacity(4);
        code.push(Expr::Int(0));

        let start2 = parser.peek_token_span().start;

        let upper_bound = parse_expr_no_struct(parser);
        code.push(upper_bound);

        let end2 = parser.last_token_end;

        let for_loop_code = parse_block(parser);
        code.extend(for_loop_code);

        Expr::IntForLoop(IntForLoopExpr {
            var_name: id,
            code: parser.bump.alloc_slice_copy(&code),
            lower_bound_span: (start, start).into(),
            upper_bound_span: (start2, end2).into(),
        })
    } else {
        let for_collection = parse_expr_no_struct(parser);
        let end = parser.last_token_end;
        let peek_token = parser.peek_token();
        if peek_token == Token::RangeDot {
            parser.next_token();

            let mut code: Vec<Expr> = Vec::with_capacity(4);
            code.push(for_collection); // lower bound

            let start2 = parser.peek_token_span().start;
            let upper_bound = parse_expr_no_struct(parser);
            code.push(upper_bound);

            let end2 = parser.last_token_end;
            let for_loop_code = parse_block(parser);
            code.extend(for_loop_code);

            Expr::IntForLoop(IntForLoopExpr {
                var_name: id,
                code: parser.bump.alloc_slice_copy(&code),
                lower_bound_span: (start, end).into(),
                upper_bound_span: (start2, end2).into(),
            })
        } else {
            let for_loop_code = parse_block(parser);
            Expr::ForLoop(
                id,
                parser.bump.alloc(for_collection),
                parser.bump.alloc_slice_copy(&for_loop_code),
                (start, end).into(),
            )
        }
    }
}

#[inline(always)]
pub fn parse_eval_block<'arena>(parser: &mut Parser<'arena>) -> Expr<'arena> {
    Expr::EvalBlock(parser.bump.alloc_slice_copy(&parse_block(parser)))
}

pub fn parse_function<'arena>(parser: &mut Parser<'arena>) -> Expr<'arena> {
    let (t, _) = parser.next_token();
    debug_assert_eq!(t, Token::Function);
    let (t_fn_id, span) = parser.next_token();
    let Token::Identifier(fn_name) = t_fn_id else {
        cold_path();
        parser.error(
            span,
            ParserErr::UnexpectedToken(Token::Identifier(""), t_fn_id, "Invalid function name."),
        );
    };
    parser.next_token_expect(Token::LParen, "Function arguments must be delimited by parentheses");
    let mut args: Vec<FunctionDeclarationArgumentExpr> = Vec::with_capacity(4);
    loop {
        if parser.peek_token() == Token::RParen {
            parser.next_token();
            break;
        }
        let (arg, span) = parser.next_token();
        if let Token::Identifier(arg) = arg {
            args.push(FunctionDeclarationArgumentExpr {
                name: arg,
                enforced_type: if parser.peek_token() == Token::Colon {
                    parser.next_token();
                    Some(parse_type(parser))
                } else {
                    None
                },
            });
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
    let fn_code = parser.bump.alloc_slice_copy(&parse_block(parser));
    Expr::FunctionDecl(FunctionDeclarationExpr {
        name: fn_name,
        args: parser.bump.alloc_slice_copy(&args),
        code: fn_code,
        span,
    })
}

pub fn parse_try_catch_block<'arena>(parser: &mut Parser<'arena>) -> Expr<'arena> {
    let (t, Span { start, end: _ }) = parser.next_token();
    debug_assert_eq!(t, Token::Try);
    let try_code = parse_block(parser);
    let mut has_catch = false;
    let mut catch_blocks: Vec<(&str, Vec<Expr>)> = Vec::with_capacity(1);
    let mut catch_all_var = "e";
    let mut catch_all_code = None;
    let end: u32;
    loop {
        let token_peek = parser.peek_token();
        if token_peek != Token::Catch {
            end = parser.peek_token_span().end;
            break;
        }
        parser.next_token();
        let (next_token, _) = parser.next_token();
        if let Token::Identifier(i) = next_token {
            // catch-all
            catch_all_var = i;
            catch_all_code = Some(parse_block(parser));
            end = parser.peek_token_span().start;
            has_catch = true;
            break;
        } else if let Token::String(s) = next_token {
            catch_blocks.push((parse_string(s, parser.bump).into_bump_str(), parse_block(parser)));
            has_catch = true;
        }
    }
    if !has_catch {
        cold_path();
        parser.error((start, end).into(), ParserErr::TryBlockNoCatch);
    }
    let catch_all_var_name = QualifiedName::new(&[catch_all_var], parser.bump);
    let usr_var = Expr::Var(catch_all_var_name, (start, end).into());
    let else_code = if let Some(c) = catch_all_code {
        c
    } else {
        vec![Expr::FunctionCall(FunctionCallExpr {
            qualified_name: QualifiedName::new(&["throw"], parser.bump),
            args: parser.bump.alloc_slice_copy(&[usr_var]),
            spans: parser.bump.alloc_slice_copy(&[(start, end).into()]),
        })]
    };

    if catch_blocks.is_empty() {
        return Expr::TryCatchBlock(
            parser.bump.alloc_slice_copy(&try_code),
            catch_all_var,
            parser.bump.alloc_slice_copy(&else_code),
        );
    }

    let mut output_code: Vec<Expr> = Vec::with_capacity(2);
    let mut otherwise_branches: Vec<(Expr, Vec<Expr>)> = Vec::with_capacity(2);
    let mut main_condition = Expr::Null;

    let mut first = true;
    for (e, c) in catch_blocks {
        let condition = Expr::Eq(
            parser.bump.alloc(Expr::String(e)),
            parser.bump.alloc(Expr::Var(catch_all_var_name, (start, end).into())),
        );
        if first {
            first = false;
            main_condition = condition;
            output_code.extend(c);
        } else {
            otherwise_branches.push((condition, c));
        }
    }
    let mut otherwise = else_code;
    for (condition, code) in otherwise_branches.into_iter().rev() {
        otherwise = vec![Expr::IfBlock(IfBlockExpr {
            condition: parser.bump.alloc(condition),
            then: parser.bump.alloc_slice_copy(&code),
            otherwise: parser.bump.alloc_slice_copy(&otherwise),
            condition_span: (start, end).into(),
            span: (start, end).into(),
        })];
    }
    Expr::TryCatchBlock(
        parser.bump.alloc_slice_copy(&try_code),
        catch_all_var,
        parser.bump.alloc_slice_copy(&[Expr::IfBlock(IfBlockExpr {
            condition: parser.bump.alloc(main_condition),
            then: parser.bump.alloc_slice_copy(&output_code),
            otherwise: parser.bump.alloc_slice_copy(&otherwise),
            condition_span: (start, end).into(),
            span: (start, end).into(),
        })]),
    )
}

pub fn parse_struct_declare<'arena>(parser: &mut Parser<'arena>) -> Expr<'arena> {
    let (t, _) = parser.next_token();
    debug_assert_eq!(t, Token::Struct);
    let (next_token, span) = parser.next_token();
    let Token::Identifier(struct_name) = next_token else {
        cold_path();
        parser.error(span, ParserErr::UnexpectedToken(Token::Identifier(""), next_token, ""));
    };
    parser.next_token_expect(Token::LBrace, "Expected '{'");
    let mut fields: Vec<(&str, TypeExpr, Span)> = Vec::with_capacity(4);
    loop {
        let (next_token, _) = parser.next_token();
        let Token::Identifier(field_name) = next_token else {
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
        let field_type_start = parser.peek_token_span().start;
        let field_type = parse_type(parser);
        let field_type_end = parser.peek_token_span().end;
        fields.push((field_name, field_type, (field_type_start, field_type_end).into()));
        let (next_token, span) = parser.next_token();
        if next_token == Token::RBrace {
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
            parser.next_token();
            break;
        }
    }
    Expr::StructDeclare(struct_name, parser.bump.alloc_slice_copy(&fields), span)
}

pub fn parse_loop_block<'arena>(parser: &mut Parser<'arena>) -> Expr<'arena> {
    let (t, _) = parser.next_token();
    debug_assert_eq!(t, Token::Loop);
    Expr::LoopBlock(parser.bump.alloc_slice_copy(&parse_block(parser)))
}

pub fn parse_match<'arena>(parser: &mut Parser<'arena>) -> Expr<'arena> {
    let (t, Span { start, end: _ }) = parser.next_token();
    debug_assert_eq!(t, Token::Match);
    let match_obj = parse_expr_no_struct(parser);
    parser.next_token_expect(Token::LBrace, "Blocks need to start with '{'");
    let mut arms: Vec<MatchArm> = Vec::with_capacity(4);
    let end: u32;
    loop {
        let peek_token = parser.peek_token();
        if peek_token == Token::RBrace {
            if arms.is_empty() {
                cold_path();
                let span = (start, parser.peek_token_span().end).into();
                parser.error(span, ParserErr::MatchBlockZeroArms);
            }
            end = parser.next_token().1.end;
            break;
        }
        let is_arm_wildcard = peek_token == Token::Identifier("_");
        if is_arm_wildcard && arms.is_empty() {
            cold_path();
            let span = (start, parser.peek_token_span().end).into();
            parser.error(span, ParserErr::MatchBlockNoNonWildcardArm);
        }
        let arm_start = parser.peek_token_span().start;
        let pattern = parse_pattern(parser);
        parser.next_token_expect(
            Token::FatArrow,
            "Match arms need to be a pattern followed by '=>' and a block",
        );
        let arm_code = parse_block(parser);
        let arm_end = parser.last_token_end;
        arms.push(MatchArm {
            pattern,
            guard: None,
            code: parser.bump.alloc_slice_copy(&arm_code),
            span: (arm_start, arm_end).into(),
        });
        if is_arm_wildcard {
            end = parser.peek_token_span().end;
            parser.next_token_expect(Token::RBrace, "");
            break;
        }
    }
    Expr::Match(MatchExpr {
        obj: parser.bump.alloc(match_obj),
        arms: parser.bump.alloc_slice_copy(&arms),
        span: (start, end).into(),
    })
}

fn parse_pattern<'arena>(parser: &mut Parser<'arena>) -> Pattern<'arena> {
    let (token, token_span) = parser.next_token();
    let literal_token = match token {
        Token::Identifier("_") => return Pattern::Wildcard(token_span),
        Token::Identifier(id) => return parse_match_constructor(parser, id, token_span),
        Token::Int(i) => Expr::Int(i),
        Token::Float(f) => Expr::Float(f),
        Token::String(s) => Expr::String(parse_string(s, parser.bump).into_bump_str()),
        Token::True => Expr::Bool(true),
        Token::False => Expr::Bool(false),
        Token::Null => Expr::Null,
        Token::OpSub => {
            let (num, num_span) = parser.next_token();
            let neg_number = match num {
                Token::Int(i) => Expr::Int(-i),
                Token::Float(f) => Expr::Float(-f),
                other => panic!(),
            };
            return Pattern::Constant(
                parser.bump.alloc(neg_number),
                (token_span.start, num_span.end).into(),
            );
        }
        other => panic!(),
    };
    Pattern::Constant(parser.bump.alloc(literal_token), token_span)
}

fn parse_match_constructor<'arena>(
    parser: &mut Parser<'arena>,
    initial: &'arena str,
    initial_span: Span,
) -> Pattern<'arena> {
    let mut namespace: Vec<&str> = Vec::with_capacity(2);
    namespace.push(initial);
    let mut end = initial_span.end;
    while parser.peek_token() == Token::DoubleColon {
        parser.next_token();
        let (token, token_span) = parser.next_token();
        let Token::Identifier(path_element) = token else { panic!() };
        namespace.push(path_element);
        end = token_span.end;
    }
    let namespace = QualifiedName::new(&namespace, parser.bump);
    if parser.peek_token() != Token::LBrace {
        return Pattern::Identifier(namespace, (initial_span.start, end).into());
    }
    parser.next_token();
    let mut constructor_fields: Vec<PatternConstructorField> = Vec::with_capacity(2);
    let mut fill_the_rest = false;
    let end: u32;
    loop {
        let (token, t_span) = parser.next_token();
        match token {
            Token::RBrace => {
                end = t_span.end;
                break;
            }
            Token::RangeDot => {
                fill_the_rest = true;
                end = parser.next_token_expect(Token::RBrace, "").end;
                break;
            }
            Token::Identifier(id) => {
                let pattern = if parser.peek_token() == Token::Colon {
                    parser.next_token();
                    parse_pattern(parser)
                } else {
                    Pattern::Identifier(QualifiedName::new(&[id], parser.bump), t_span)
                };
                constructor_fields.push(PatternConstructorField {
                    name: id,
                    pattern,
                    span: (t_span.start, parser.last_token_end).into(),
                });
                match parser.peek_token() {
                    Token::Comma => {
                        parser.next_token();
                    }
                    Token::RBrace => {}
                    other => panic!(),
                }
            }
            other => panic!(),
        }
    }
    Pattern::Constructor(PatternConstructor {
        qualified_name: namespace,
        fields: parser.bump.alloc_slice_copy(&constructor_fields),
        fill_the_rest,
        span: (initial_span.start, end).into(),
    })
}
