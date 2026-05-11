use crate::errors::dev_error;
use crate::type_system::DataType;
use smol_str::SmolStr;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
#[repr(C)]
pub enum Expr {
    Float(f64),
    Int(i32),
    Bool(bool),
    Null,
    String(SmolStr),
    /// Var(name, start, end)
    Var(SmolStr, (usize, usize)),
    /// Array(contents, start, end)
    Array(Box<[Expr]>, (usize, usize)),
    /// VarDeclare(name, value),
    VarDeclare(SmolStr, Box<Expr>),
    /// VarDeclare(name, value, start, end)
    VarAssign(SmolStr, Box<Expr>, (usize, usize)),
    /// Condition(condition, code (contains else_if_blocks and potentially else_block), start, end)
    Condition(Box<Expr>, Box<[Expr]>, (usize, usize)),
    /// InlineCondition — expression-form if/else, always produces a value, must have an else branch
    InlineCondition(Box<Expr>, Box<[Expr]>, (usize, usize)),
    ElseIfBlock(Box<Expr>, Box<[Expr]>),
    ElseBlock(Box<[Expr]>),

    WhileBlock(Box<Expr>, Box<[Expr]>),
    /// FunctionCall(args, (optional namespace + name), start, end, (arg_start,arg_end))
    FunctionCall(
        Box<[Expr]>,
        Box<[SmolStr]>,
        (usize, usize),
        Box<[(usize, usize)]>,
    ),
    ObjFunctionCall(
        Box<Expr>,
        Box<[Expr]>,
        Box<[SmolStr]>,
        (
            // obj_start
            usize,
            // obj_end
            usize,
        ),
        (
            // fn_start
            usize,
            // fn_end
            usize,
        ),
        Box<[(usize, usize)]>,
    ),
    /// FunctionDecl(name+args, code, start, end)
    FunctionDecl(Box<[SmolStr]>, Rc<[Expr]>, (usize, usize)),

    ReturnVal(Box<Option<Expr>>),

    GetIndex(Box<Expr>, Box<[Expr]>, (usize, usize)),
    ArrayModify(
        Box<Expr>,
        Box<[Expr]>,
        Box<Expr>,
        (usize, usize),
        (usize, usize),
    ),

    /// ForLoop(loop_var_name, loop_array+code, obj_markers)
    ForLoop(SmolStr, Box<[Expr]>, (usize, usize)),
    /// IntForLoop(loop_var_name, first_elem, final_elem, code)
    IntForLoop(
        SmolStr,
        Box<Expr>,
        Box<Expr>,
        Box<[Expr]>,
        (usize, usize),
        (usize, usize),
    ),
    /// Import(lib_path, [(fn_name, fn_args, fn_return_type)], (start, end))
    ImportDynLib(
        SmolStr,
        Box<[(SmolStr, Box<[DataType]>, DataType)]>,
        (usize, usize),
    ),

    /// ImportFile(path, (start, end))
    ImportFile(SmolStr, (usize, usize)),

    Break,
    Continue,

    EvalBlock(Box<[Expr]>),
    LoopBlock(Box<[Expr]>),

    Mul(Box<Expr>, Box<Expr>, (usize, usize)),
    Div(Box<Expr>, Box<Expr>, (usize, usize)),
    Add(Box<Expr>, Box<Expr>, (usize, usize)),
    Sub(Box<Expr>, Box<Expr>, (usize, usize)),
    Mod(Box<Expr>, Box<Expr>, (usize, usize)),
    Pow(Box<Expr>, Box<Expr>, (usize, usize)),
    Eq(Box<Expr>, Box<Expr>),
    NotEq(Box<Expr>, Box<Expr>),
    Sup(Box<Expr>, Box<Expr>, (usize, usize)),
    SupEq(Box<Expr>, Box<Expr>, (usize, usize)),
    Inf(Box<Expr>, Box<Expr>, (usize, usize)),
    InfEq(Box<Expr>, Box<Expr>, (usize, usize)),
    BoolAnd(Box<Expr>, Box<Expr>, (usize, usize)),
    BoolOr(Box<Expr>, Box<Expr>, (usize, usize)),
    BoolNeg(Box<Expr>, (usize, usize)),
    Neg(Box<Expr>, (usize, usize)),
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
