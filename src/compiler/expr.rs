use super::type_system::TypeExpr;
use smol_strc::SmolStr;
use std::{hint::unreachable_unchecked, rc::Rc};

#[derive(PartialEq, Clone, Debug)]
pub struct IfBlockExpr {
    pub condition: Box<Expr>,
    /// Contains any else_if_blocks / else_block
    pub code: Box<[Expr]>,
    pub span: Span,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct DylibFnExpr {
    pub name: SmolStr,
    /// Invariant:
    /// - `args.len() > 0`
    /// - `args[0]` is the function's return type
    pub args: Box<[(TypeExpr, Span)]>,
    pub name_span: Span,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct DylibImportExpr {
    pub path: SmolStr,
    pub functions: Box<[DylibFnExpr]>,
    pub span: Span,
}

#[derive(PartialEq, Clone, Debug)]
pub struct StructFieldExpr {
    pub name: SmolStr,
    pub value: Expr,
    pub name_span: Span,
    pub value_span: Span,
}

#[derive(PartialEq, Clone, Debug)]
pub struct FunctionCallExpr {
    pub qualified_name: QualifiedName,
    pub args: Box<[Expr]>,
    pub span: Span,
    pub arg_spans: Box<[Span]>,
}

#[derive(PartialEq, Clone, Debug)]
pub struct IntForLoopExpr {
    pub var_name: SmolStr,
    /// Invariant:
    /// - code.len() >= 2
    /// - code[0] is the lower bound
    /// - code[1] is the lower bound
    pub code: Box<[Expr]>,
    pub lower_bound_span: Span,
    pub upper_bound_span: Span,
}

impl IntForLoopExpr {
    #[inline(always)]
    pub fn get_lower_bound(&self) -> &Expr {
        unsafe { self.code.get_unchecked(0) }
    }
    #[inline(always)]
    pub fn get_upper_bound(&self) -> &Expr {
        unsafe { self.code.get_unchecked(1) }
    }
    #[inline(always)]
    pub fn get_loop_code(&self) -> &[Expr] {
        if self.code.len() > 2 { unsafe { self.code.get_unchecked(2..) } } else { &[] }
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct FunctionDeclarationArgumentExpr {
    pub name: SmolStr,
    pub enforced_type: Option<TypeExpr>,
}

#[derive(PartialEq, Clone, Debug)]
pub struct FunctionDeclarationExpr {
    pub name: SmolStr,
    pub args: Box<[FunctionDeclarationArgumentExpr]>,
    pub code: Rc<[Expr]>,
    pub span: Span,
}

#[derive(PartialEq, Clone, Debug)]
pub struct StructFieldAssignmentExpr {
    pub struct_expr: Box<Expr>,
    pub field: SmolStr,
    pub field_value: Box<Expr>,
    pub struct_span: Span,
    pub field_span: Span,
    pub value_span: Span,
}

/// A fully-qualified symbol name.
/// Invariant:
/// - len > 0
/// - last element is the symbol's name
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct QualifiedName(Box<[SmolStr]>);

impl QualifiedName {
    pub fn new<T>(src: T) -> Self
    where
        Box<[SmolStr]>: From<T>,
    {
        Self(Box::from(src))
    }
    #[inline(always)]
    pub const fn get_name(&self) -> &SmolStr {
        unsafe { self.0.last().unwrap_unchecked() }
    }
    #[inline(always)]
    pub fn get_namespace(&self) -> &[SmolStr] {
        &self.0[..self.0.len() - 1]
    }
    #[inline(always)]
    pub const fn is_namespace_empty(&self) -> bool {
        self.0.len() < 2
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Float(f64),
    Int(i32),
    Bool(bool),
    Null,
    String(SmolStr),
    Var(SmolStr, Span),

    ConstDeclare(SmolStr, Box<Self>),

    /// Array(contents, [entire_array, elem_spans...])
    Array(Box<[Self]>, Box<[Span]>),
    /// Map(key-value pairs, span)
    Map(Box<[(Self, Span, Self, Span)]>, Span),
    /// Struct(name, fields, span)
    Struct(QualifiedName, Box<[StructFieldExpr]>, Span),
    /// StructDeclare(name, fields, span)
    StructDeclare(SmolStr, Box<[(SmolStr, TypeExpr, Span)]>, Span),
    /// GetStructField(struct_expr, field, struct_span, field_span, value_span)
    GetStructField(Box<Self>, SmolStr, Span, Span),
    SetStructField(StructFieldAssignmentExpr),
    /// VarDeclare(name, value),
    VarDeclare(SmolStr, Box<Self>),
    /// VarDeclare(name, value, start, end)
    VarAssign(SmolStr, Box<Self>, Span),
    IfBlock(IfBlockExpr),
    /// InlineCondition - expression-form if/else, always produces a value, must have an else branch
    InlineCondition(Box<Self>, Box<[Self]>, Span),
    ElseIfBlock(Box<Self>, Box<[Self]>),
    ElseBlock(Box<[Self]>),

    /// AnonymousFunction(args, code, span)
    AnonymousFunction(Box<[(SmolStr, Option<TypeExpr>)]>, Box<[Self]>, Span),
    WhileBlock(Box<Self>, Box<[Self]>),
    FunctionCall(FunctionCallExpr),
    ObjFunctionCall(FunctionCallExpr),
    FunctionDecl(FunctionDeclarationExpr),

    ReturnVal(Box<Option<Self>>),

    ArrayGetIndex(Box<Self>, Box<Self>, Span),
    /// ArrayGetSlice(array, range_start, range_end, span)
    ArrayGetSlice(Box<Self>, Box<Self>, Box<Self>, Span),
    ArrayModify(Box<Self>, Box<Self>, Box<Self>, Span, Span),

    /// ForLoop(loop_var_name, loop_array+code, obj_markers)
    ForLoop(SmolStr, Box<Self>, Box<[Self]>, Span),
    IntForLoop(IntForLoopExpr),
    ImportDylib(DylibImportExpr),

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
        Expr::IfBlock(IfBlockExpr { code, .. })
        | Expr::WhileBlock(_, code)
        | Expr::EvalBlock(code)
        | Expr::LoopBlock(code)
        | Expr::InlineCondition(_, code, _)
        | Expr::ElseIfBlock(_, code)
        | Expr::ElseBlock(code)
        | Expr::ForLoop(_, _, code, _) => code_modifies_variable(var_name, code),
        Expr::IntForLoop(for_loop) => code_modifies_variable(var_name, for_loop.get_loop_code()),
        _ => false,
    })
}

pub fn var_assign(target: Expr, value: Expr, expr_span: Span, value_span: Span) -> Expr {
    if let Expr::Var(n, s) = target {
        Expr::VarAssign(n, Box::from(value), s)
    } else if let Expr::ArrayGetIndex(base, idx, _) = target {
        Expr::ArrayModify(base, idx, Box::from(value), expr_span, value_span)
    } else if let Expr::GetStructField(struct_expr, field, struct_span, field_span) = target {
        Expr::SetStructField(StructFieldAssignmentExpr {
            struct_expr,
            field,
            field_value: Box::from(value),
            struct_span,
            field_span,
            value_span,
        })
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
        Self { start: self.start, end: span.end }
    }
}

impl From<std::range::Range<usize>> for Span {
    #[inline(always)]
    fn from(value: std::range::Range<usize>) -> Self {
        Self { start: value.start as u32, end: value.end as u32 }
    }
}

impl From<std::ops::Range<usize>> for Span {
    #[inline(always)]
    fn from(value: std::ops::Range<usize>) -> Self {
        Self { start: value.start as u32, end: value.end as u32 }
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
        Self { start: start as u32, end: end as u32 }
    }
}

impl From<(u32, u32)> for Span {
    #[inline(always)]
    fn from((start, end): (u32, u32)) -> Self {
        Self { start, end }
    }
}
