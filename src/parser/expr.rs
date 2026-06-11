use crate::errors::dev_error;
use crate::type_system::DataType;
use smol_strc::SmolStr;
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
    /// Array(contents, span)
    Array(Box<[Self]>, Span),
    /// Struct(name, fields, span)
    Struct(Box<[SmolStr]>, Box<[(SmolStr, Self, Span)]>, Span),
    /// StructDeclare(name, fields, span)
    StructDeclare(SmolStr, Box<[(SmolStr, SmolStr)]>, Span),
    /// GetStructField(struct_expr, field, struct_span, field_span)
    GetStructField(Box<Self>, SmolStr, Span, Span),
    /// SetStructField(struct_expr, field, new_expr, struct_span, field_span)
    SetStructField(Box<Self>, SmolStr, Box<Self>, Span, Span),
    /// VarDeclare(name, value),
    VarDeclare(SmolStr, Box<Self>),
    /// VarDeclare(name, value, start, end)
    VarAssign(SmolStr, Box<Self>, Span),
    /// Condition(condition, code (contains else_if_blocks and potentially else_block), start, end)
    Condition(Box<Self>, Box<[Self]>, Span),
    /// InlineCondition - expression-form if/else, always produces a value, must have an else branch
    InlineCondition(Box<Self>, Box<[Self]>, Span),
    ElseIfBlock(Box<Self>, Box<[Self]>),
    ElseBlock(Box<[Self]>),

    WhileBlock(Box<Self>, Box<[Self]>),
    /// FunctionCall(args, (optional namespace + name), start, end, (arg_start,arg_end))
    FunctionCall(Box<[Self]>, Box<[SmolStr]>, Span, Box<[Span]>),
    /// ObjFunctionCall(obj, args, namespace, obj_span, fn_span, arg_markers)
    ObjFunctionCall(
        // WILL BE REMOVED SOON
        Box<Self>,
        Box<[Self]>,
        Box<[SmolStr]>,
        // obj_span
        Span,
        // fn_span
        Span,
        Box<[Span]>,
    ),
    /// FunctionDecl(name+args, code, start, end)
    FunctionDecl(SmolStr, Box<[SmolStr]>, Rc<[Self]>, Span),

    ReturnVal(Box<Option<Self>>),

    ArrayGetIndex(Box<Self>, Box<Self>, Span),
    /// ArrayGetSlice(array, range_start, range_end, span)
    ArrayGetSlice(Box<Self>, Box<Self>, Box<Self>, Span),
    ArrayModify(Box<Self>, Box<Self>, Box<Self>, Span, Span),

    /// ForLoop(loop_var_name, loop_array+code, obj_markers)
    ForLoop(SmolStr, Box<Self>, Box<[Self]>, Span),
    /// IntForLoop(loop_var_name, first_elem, final_elem, code)
    IntForLoop(SmolStr, Box<Self>, Box<Self>, Box<[Self]>, Span, Span),
    /// ImportDylib(lib_path, [(fn_name, fn_args, fn_return_type)], (start, end))
    ImportDylib(SmolStr, Box<[(SmolStr, Box<[SmolStr]>, SmolStr)]>, Span),

    /// ImportFile(path,alias ,(start, end))
    ImportFile(SmolStr, Option<SmolStr>, Span),

    Break,
    Continue,

    EvalBlock(Box<[Self]>),
    LoopBlock(Box<[Self]>),

    /// TryCatchBlock(try_code, err_var, catch_code)
    TryCatchBlock(Box<[Self]>, SmolStr, Box<[Self]>),

    Mul(Box<Self>, Box<Self>, Span),
    Div(Box<Self>, Box<Self>, Span),
    Add(Box<Self>, Box<Self>, Span),
    Sub(Box<Self>, Box<Self>, Span),
    Mod(Box<Self>, Box<Self>, Span),
    Pow(Box<Self>, Box<Self>, Span),
    Eq(Box<Self>, Box<Self>),
    NotEq(Box<Self>, Box<Self>),
    Sup(Box<Self>, Box<Self>, Span),
    SupEq(Box<Self>, Box<Self>, Span),
    Inf(Box<Self>, Box<Self>, Span),
    InfEq(Box<Self>, Box<Self>, Span),
    BoolAnd(Box<Self>, Box<Self>, Span),
    BoolOr(Box<Self>, Box<Self>, Span),
    BoolNeg(Box<Self>, Span),
    Neg(Box<Self>, Span),
}

#[cold]
#[inline(never)]
pub fn symbol_of_expr(expr: &Expr) -> &str {
    match expr {
        Expr::Mul(_, _, _) => "*",
        Expr::Div(_, _, _) => "/",
        Expr::Add(_, _, _) => "+",
        Expr::Sub(_, _, _) | Expr::Neg(_, _) => "-",
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
        | Expr::InlineCondition(_, body, _)
        | Expr::ElseIfBlock(_, body)
        | Expr::ElseBlock(body)
        | Expr::ForLoop(_, _, body, _)
        | Expr::IntForLoop(_, _, _, body, _, _) => contains_var_reassign(name, body),
        _ => false,
    })
}

pub fn var_assign(target: Expr, value: Expr, expr_span: Span, value_span: Span) -> Expr {
    if let Expr::Var(n, _) = target {
        Expr::VarAssign(n, Box::from(value), expr_span)
    } else if let Expr::ArrayGetIndex(base, idx, _) = target {
        Expr::ArrayModify(base, idx, Box::from(value), expr_span, value_span)
    } else if let Expr::GetStructField(obj, field, obj_span, field_span) = target {
        Expr::SetStructField(obj, field, Box::from(value), obj_span, field_span)
    } else {
        todo!()
    }
}
