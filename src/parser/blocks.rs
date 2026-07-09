use crate::cold_path;
use crate::errors::ParserErr;
use crate::expr::Expr;
use crate::expr::Span;
use crate::lexer::Token;
use crate::parser::Parser;
use crate::parser::TypeExpr;
use crate::parser::parse_code;
use crate::parser::parse_type;
use crate::parser_expr::parse_expr;
use crate::parser_expr::parse_expr_no_struct;
use smol_strc::SmolStr;

// call right after peeking Token::If
pub fn parse_condition_block(parser: &mut Parser<'_>, start: u32) -> Expr {
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
    input.next_token_expect_end(Token::RBrace, "Unmatched '}'");
    while_code
}

pub fn parse_while_block(input: &mut Parser<'_>) -> Expr {
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
pub fn parse_for_loop(parser: &mut Parser<'_>) -> Expr {
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
        if peek_token == Token::RangeDot {
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
pub fn parse_eval_block(parser: &mut Parser<'_>) -> Expr {
    Expr::EvalBlock(Box::from(parse_block(parser)))
}

pub fn parse_function(parser: &mut Parser<'_>) -> Expr {
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

pub fn parse_try_catch_block(parser: &mut Parser<'_>) -> Expr {
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

pub fn parse_struct_declare(parser: &mut Parser<'_>) -> Expr {
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
    let mut fields: Vec<(SmolStr, TypeExpr)> = Vec::with_capacity(4);
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

pub fn parse_loop_block(input: &mut Parser<'_>) -> Expr {
    let (t, _) = input.next_token();
    debug_assert_eq!(t, Token::Loop);
    Expr::LoopBlock(Box::from(parse_block(input)))
}

pub fn parse_match(parser: &mut Parser<'_>) -> Expr {
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
        if peek_token == Token::Identifier("_") {
            if first_condition.is_none() {
                cold_path();
                let span = (start, parser.peek_token_end()).into();
                parser.error(span, ParserErr::MatchBlockNoNonWildcardArm);
            }
            parser.next_token();
            parser.next_token_expect(Token::FatArrow, "Expected '=>'");
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
            parser.next_token_expect(Token::FatArrow, "");
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
