use super::type_system::TypeExpr;
use smol_strc::SmolStr;
use std::{hint::unreachable_unchecked, rc::Rc};

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Float(f64),
    Int(i32),
    Bool(bool),
    Null,
    String(SmolStr),
    Var(SmolStr, Span),
    /// Array(contents, [entire_array, elem_spans...])
    Array(Box<[Self]>, Box<[Span]>),
    /// Map(key-value pairs, span)
    Map(Box<[(Self, Span, Self, Span)]>, Span),
    /// Struct(name, fields, span)
    Struct(Box<[SmolStr]>, Box<[(SmolStr, Self, Span, Span)]>, Span),
    /// StructDeclare(name, fields, span)
    StructDeclare(SmolStr, Box<[(SmolStr, TypeExpr, Span)]>, Span),
    /// GetStructField(struct_expr, field, struct_span, field_span, value_span)
    GetStructField(Box<Self>, SmolStr, Span, Span),
    /// SetStructField(struct_expr, field, new_expr, struct_span, field_span, value_span)
    SetStructField(Box<Self>, SmolStr, Box<Self>, Span, Span, Span),
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

    /// AnonymousFunction(args, code, span)
    AnonymousFunction(Box<[SmolStr]>, Box<[Self]>, Span),
    // AnonymousFunction(Box<[(SmolStr, SmolStr)]>, SmolStr, Box<[Self]>, Span),
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
    /// FunctionDecl(name, args, code, span)
    FunctionDecl(
        SmolStr,
        Box<[(SmolStr, Option<TypeExpr>)]>,
        Rc<[Self]>,
        Span,
    ),

    ReturnVal(Box<Option<Self>>),

    ArrayGetIndex(Box<Self>, Box<Self>, Span),
    /// ArrayGetSlice(array, range_start, range_end, span)
    ArrayGetSlice(Box<Self>, Box<Self>, Box<Self>, Span),
    ArrayModify(Box<Self>, Box<Self>, Box<Self>, Span, Span),

    /// ForLoop(loop_var_name, loop_array+code, obj_markers)
    ForLoop(SmolStr, Box<Self>, Box<[Self]>, Span),
    /// IntForLoop(loop_var_name, first_elem, final_elem, code)
    IntForLoop(SmolStr, Box<Self>, Box<Self>, Box<[Self]>, Span, Span),
    /// ImportDylib(lib_path, [(fn_name, fn_args, fn_return_type, fn_name_span)], (start, end))
    ImportDylib(
        SmolStr,
        Box<[(SmolStr, Box<[TypeExpr]>, TypeExpr, Span)]>,
        Span,
    ),

    /// ImportFile(path,alias ,(start, end))
    ImportFile(SmolStr, Option<SmolStr>, Span),

    Break,
    Continue,

    EvalBlock(Box<[Self]>),
    LoopBlock(Box<[Self]>),

    /// TryCatchBlock(try_code, err_var, catch_code)
    TryCatchBlock(Box<[Self]>, SmolStr, Box<[Self]>),

    Mul(Box<Self>, Box<Self>, Span, Span),
    Div(Box<Self>, Box<Self>, Span, Span),
    Add(Box<Self>, Box<Self>, Span, Span),
    Sub(Box<Self>, Box<Self>, Span, Span),
    Mod(Box<Self>, Box<Self>, Span, Span),
    Pow(Box<Self>, Box<Self>, Span, Span),
    Eq(Box<Self>, Box<Self>),
    NotEq(Box<Self>, Box<Self>),
    Sup(Box<Self>, Box<Self>, Span, Span),
    SupEq(Box<Self>, Box<Self>, Span, Span),
    Inf(Box<Self>, Box<Self>, Span, Span),
    InfEq(Box<Self>, Box<Self>, Span, Span),
    BoolAnd(Box<Self>, Box<Self>, Span, Span),
    BoolOr(Box<Self>, Box<Self>, Span, Span),
    BoolNeg(Box<Self>, Span, Span),
    Neg(Box<Self>, Span, Span),
}

#[cold]
#[inline(never)]
pub const fn symbol_of_expr(expr: &Expr) -> &'static str {
    match expr {
        Expr::Mul(_, _, _, _) => "*",
        Expr::Div(_, _, _, _) => "/",
        Expr::Add(_, _, _, _) => "+",
        Expr::Sub(_, _, _, _) | Expr::Neg(_, _, _) => "-",
        Expr::Mod(_, _, _, _) => "%",
        Expr::Pow(_, _, _, _) => "^",
        Expr::Eq(_, _) => "==",
        Expr::NotEq(_, _) => "!=",
        Expr::Sup(_, _, _, _) => ">",
        Expr::SupEq(_, _, _, _) => ">=",
        Expr::Inf(_, _, _, _) => "<",
        Expr::InfEq(_, _, _, _) => "<=",
        Expr::BoolAnd(_, _, _, _) => "&&",
        Expr::BoolOr(_, _, _, _) => "||",
        _ => unsafe { unreachable_unchecked() },
    }
}

pub fn code_modifies_variable(var_name: &SmolStr, code: &[Expr]) -> bool {
    code.iter().any(|expr| match expr {
        Expr::VarAssign(n, _, _) => n == var_name,
        Expr::Condition(_, body, _)
        | Expr::WhileBlock(_, body)
        | Expr::EvalBlock(body)
        | Expr::LoopBlock(body)
        | Expr::InlineCondition(_, body, _)
        | Expr::ElseIfBlock(_, body)
        | Expr::ElseBlock(body)
        | Expr::ForLoop(_, _, body, _)
        | Expr::IntForLoop(_, _, _, body, _, _) => code_modifies_variable(var_name, body),
        _ => false,
    })
}

pub fn var_assign(target: Expr, value: Expr, expr_span: Span, value_span: Span) -> Expr {
    if let Expr::Var(n, s) = target {
        Expr::VarAssign(n, Box::from(value), s)
    } else if let Expr::ArrayGetIndex(base, idx, _) = target {
        Expr::ArrayModify(base, idx, Box::from(value), expr_span, value_span)
    } else if let Expr::GetStructField(obj, field, obj_span, field_span) = target {
        Expr::SetStructField(
            obj,
            field,
            Box::from(value),
            obj_span,
            field_span,
            value_span,
        )
    } else {
        unsafe { unreachable_unchecked() }
    }
}

/// A span of code in a `Source`'s `contents`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    #[inline(always)]
    #[must_use]
    pub const fn extend(self, span: Self) -> Self {
        Self {
            start: self.start,
            end: span.end,
        }
    }
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

impl From<std::ops::Range<usize>> for Span {
    #[inline(always)]
    fn from(value: std::ops::Range<usize>) -> Self {
        Self {
            start: value.start as u32,
            end: value.end as u32,
        }
    }
}

impl From<Span> for std::ops::Range<usize> {
    #[inline(always)]
    fn from(val: Span) -> Self {
        val.start as usize..val.end as usize
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
        Self { start, end }
    }
}
