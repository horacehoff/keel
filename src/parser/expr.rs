use crate::errors::dev_error;
use crate::type_system::DataType;
use smol_str::SmolStr;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Float(f64),
    Int(i32),
    Bool(bool),
    Null,
    String(SmolStr),
    /// Var(name, start, end)
    Var(SmolStr, Span),
    /// Array(contents, start, end)
    Array(Box<[Expr]>, Span),
    /// VarDeclare(name, value),
    VarDeclare(SmolStr, Box<Expr>),
    /// VarDeclare(name, value, start, end)
    VarAssign(SmolStr, Box<Expr>, Span),
    /// Condition(condition, code (contains else_if_blocks and potentially else_block), start, end)
    Condition(Box<Expr>, Box<[Expr]>, Span),
    /// InlineCondition - expression-form if/else, always produces a value, must have an else branch
    InlineCondition(Box<Expr>, Box<[Expr]>, Span),
    ElseIfBlock(Box<Expr>, Box<[Expr]>),
    ElseBlock(Box<[Expr]>),

    WhileBlock(Box<Expr>, Box<[Expr]>),
    /// FunctionCall(args, (optional namespace + name), start, end, (arg_start,arg_end))
    FunctionCall(Box<[Expr]>, Box<[SmolStr]>, Span, Box<[Span]>),
    ObjFunctionCall(
        Box<Expr>,
        Box<[Expr]>,
        Box<[SmolStr]>,
        // obj_span
        Span,
        // fn_span
        Span,
        Box<[Span]>,
    ),
    /// FunctionDecl(name+args, code, start, end)
    FunctionDecl(Box<[SmolStr]>, Rc<[Expr]>, Span),

    ReturnVal(Box<Option<Expr>>),

    ArrayGetIndex(Box<Expr>, Box<Expr>, Span),
    /// ArrayGetSlice(array, range_start, range_end, span)
    ArrayGetSlice(Box<Expr>, Box<Expr>, Box<Expr>, Span),
    ArrayModify(Box<Expr>, Box<Expr>, Box<Expr>, Span, Span),

    /// ForLoop(loop_var_name, loop_array+code, obj_markers)
    ForLoop(SmolStr, Box<[Expr]>, Span),
    /// IntForLoop(loop_var_name, first_elem, final_elem, code)
    IntForLoop(SmolStr, Box<Expr>, Box<Expr>, Box<[Expr]>, Span, Span),
    /// Import(lib_path, [(fn_name, fn_args, fn_return_type)], (start, end))
    ImportDynLib(SmolStr, Box<[(SmolStr, Box<[DataType]>, DataType)]>, Span),

    /// ImportFile(path, (start, end))
    ImportFile(SmolStr, Span),

    Break,
    Continue,

    EvalBlock(Box<[Expr]>),
    LoopBlock(Box<[Expr]>),

    Mul(Box<Expr>, Box<Expr>, Span),
    Div(Box<Expr>, Box<Expr>, Span),
    Add(Box<Expr>, Box<Expr>, Span),
    Sub(Box<Expr>, Box<Expr>, Span),
    Mod(Box<Expr>, Box<Expr>, Span),
    Pow(Box<Expr>, Box<Expr>, Span),
    Eq(Box<Expr>, Box<Expr>),
    NotEq(Box<Expr>, Box<Expr>),
    Sup(Box<Expr>, Box<Expr>, Span),
    SupEq(Box<Expr>, Box<Expr>, Span),
    Inf(Box<Expr>, Box<Expr>, Span),
    InfEq(Box<Expr>, Box<Expr>, Span),
    BoolAnd(Box<Expr>, Box<Expr>, Span),
    BoolOr(Box<Expr>, Box<Expr>, Span),
    BoolNeg(Box<Expr>, Span),
    Neg(Box<Expr>, Span),
}

#[cold]
#[inline(never)]
pub fn symbol_of_expr(expr: &Expr) -> &str {
    match expr {
        Expr::Mul(_, _, _) => "*",
        Expr::Div(_, _, _) => "/",
        Expr::Add(_, _, _) => "+",
        Expr::Sub(_, _, _) => "-",
        Expr::Mod(_, _, _) => "%",
        Expr::Pow(_, _, _) => "^",
        Expr::Eq(_, _) => "==",
        Expr::NotEq(_, _) => "!=",
        Expr::Sup(_, _, _) => ">",
        Expr::SupEq(_, _, _) => ">=",
        Expr::Inf(_, _, _) => "<",
        Expr::InfEq(_, _, _) => "<=",
        Expr::BoolAnd(_, _, _) => "&&",
        Expr::BoolOr(_, _, _) => "||",
        Expr::Neg(_, _) => "-",
        other => dev_error(
            "parser.rs",
            "symbol_of_expr",
            format_args!("Tried to get symbol of {other:?}"),
        ),
    }
}

pub fn contains_var_reassign(name: &SmolStr, code: &[Expr]) -> bool {
    code.iter().any(|expr| match expr {
        Expr::VarAssign(n, _, _) => n == name,
        Expr::Condition(_, body, _)
        | Expr::WhileBlock(_, body)
        | Expr::EvalBlock(body)
        | Expr::LoopBlock(body)
        | Expr::InlineCondition(_, body, _) => contains_var_reassign(name, body),
        Expr::ElseIfBlock(_, body) | Expr::ElseBlock(body) => contains_var_reassign(name, body),
        Expr::ForLoop(_, body, _) => contains_var_reassign(name, body),
        Expr::IntForLoop(_, _, _, body, _, _) => contains_var_reassign(name, body),
        _ => false,
    })
}
