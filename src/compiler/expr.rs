use super::type_system::TypeExpr;
use bumpalo::Bump;
use smol_strc::SmolStr;
use std::{hint::unreachable_unchecked, rc::Rc};

#[derive(PartialEq, Clone, Debug)]
pub struct IfBlockExpr<'arena> {
    pub condition: Box<Expr<'arena>>,
    /// if .. { <THEN> }
    pub then: Box<[Expr<'arena>]>,
    /// if .. {..} else { <OTHERWISE> }
    pub otherwise: Box<[Expr<'arena>]>,
    pub span: Span,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct DylibFnExpr<'arena> {
    pub name: SmolStr,
    /// Invariant:
    /// - `args.len() > 0`
    /// - `args[0]` is the function's return type
    pub args: Box<[(TypeExpr<'arena>, Span)]>,
    pub name_span: Span,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct DylibImportExpr<'arena> {
    pub path: SmolStr,
    pub functions: Box<[DylibFnExpr<'arena>]>,
    pub span: Span,
}

#[derive(PartialEq, Clone, Debug)]
pub struct StructFieldExpr<'arena> {
    pub name: SmolStr,
    pub value: Expr<'arena>,
    pub name_span: Span,
    pub value_span: Span,
}

#[derive(PartialEq, Clone, Debug)]
pub struct FunctionCallExpr<'arena> {
    pub qualified_name: QualifiedName<'arena>,
    pub args: Box<[Expr<'arena>]>,
    /// Invariant
    /// - `spans.len() >= 1`
    /// - `spans[0]` is the span for the whole function call
    pub spans: Box<[Span]>,
}

impl<'arena> FunctionCallExpr<'arena> {
    #[inline(always)]
    pub fn get_call_span(&self) -> Span {
        unsafe { *self.spans.get_unchecked(0) }
    }
    #[inline(always)]
    pub fn get_arg_spans(&self) -> &[Span] {
        if self.spans.len() > 1 { unsafe { self.spans.get_unchecked(1..) } } else { &[] }
    }
    #[inline(always)]
    pub fn get_nth_arg_span(&self, idx: usize) -> Span {
        debug_assert!(self.spans.len() > idx + 1);
        unsafe { *self.spans.get_unchecked(idx + 1) }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct IntForLoopExpr<'arena> {
    pub var_name: SmolStr,
    /// Invariant:
    /// - `code.len() >= 2`
    /// - `code[0]` is the lower bound
    /// - `code[1]` is the lower bound
    pub code: Box<[Expr<'arena>]>,
    pub lower_bound_span: Span,
    pub upper_bound_span: Span,
}

impl<'arena> IntForLoopExpr<'arena> {
    #[inline(always)]
    pub fn get_lower_bound(&self) -> &Expr<'_> {
        unsafe { self.code.get_unchecked(0) }
    }
    #[inline(always)]
    pub fn get_upper_bound(&self) -> &Expr<'_> {
        unsafe { self.code.get_unchecked(1) }
    }
    #[inline(always)]
    pub fn get_loop_code(&self) -> &[Expr<'_>] {
        if self.code.len() > 2 { unsafe { self.code.get_unchecked(2..) } } else { &[] }
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct FunctionDeclarationArgumentExpr<'arena> {
    pub name: SmolStr,
    pub enforced_type: Option<TypeExpr<'arena>>,
}

#[derive(PartialEq, Clone, Debug)]
pub struct FunctionDeclarationExpr<'arena> {
    pub name: SmolStr,
    pub args: Box<[FunctionDeclarationArgumentExpr<'arena>]>,
    pub code: &'arena [Expr<'arena>],
    pub span: Span,
}

#[derive(PartialEq, Clone, Debug)]
pub struct StructFieldAssignmentExpr<'arena> {
    pub struct_expr: Box<Expr<'arena>>,
    pub field: SmolStr,
    pub field_value: Box<Expr<'arena>>,
    /// Invariant:
    /// - `spans.len() == 3`
    /// - `spans[0]` = struct_span
    /// - `spans[1]` = field_span
    /// - `spans[2]` = value_span
    pub spans: Box<[Span]>,
}

/// A fully-qualified symbol name.
/// Invariant:
/// - `len > 0`
/// - last element is the symbol's name
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct QualifiedName<'arena>(&'arena [&'arena str]);

impl<'arena> QualifiedName<'arena> {
    pub fn new(src: &[&'arena str], bump: &'arena Bump) -> Self {
        let allocated = bump.alloc_slice_copy(src);
        Self(allocated)
    }
    // pub fn new<T>(src: T) -> Self
    // where
    //     &'arena [&'arena str]: From<T>,
    // {
    //     Self(src)
    // }
    #[inline(always)]
    pub const fn get_name(&'arena self) -> &'arena str {
        unsafe { self.0.last().unwrap_unchecked() }
    }
    #[inline(always)]
    pub fn get_namespace(&'arena self) -> &'arena [&'arena str] {
        &self.0[..self.0.len() - 1]
    }
    #[inline(always)]
    pub const fn is_namespace_empty(&self) -> bool {
        self.0.len() < 2
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariableDeclarationExpr<'arena> {
    pub name: SmolStr,
    pub value: &'arena Expr<'arena>,
    pub var_type: Option<Box<(TypeExpr<'arena>, Span)>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr<'arena> {
    Float(f64),
    Int(i32),
    Bool(bool),
    Null,
    String(SmolStr),
    Var(SmolStr, Span),
    NamespacedVar(QualifiedName<'arena>, Span),

    /// Array(contents, [entire_array, elem_spans...])
    Array(Box<[Self]>, Box<[Span]>),
    /// Map(key-value pairs, span)
    Map(Box<[(Self, Span, Self, Span)]>, Span),
    /// Struct(name, fields, span)
    Struct(QualifiedName<'arena>, Box<[StructFieldExpr<'arena>]>, Span),
    /// StructDeclare(name, fields, span)
    StructDeclare(SmolStr, Box<[(SmolStr, TypeExpr<'arena>, Span)]>, Span),
    /// GetStructField(struct_expr, field, struct_span, field_span, value_span)
    GetStructField(Box<Self>, SmolStr, Span, Span),
    SetStructField(StructFieldAssignmentExpr<'arena>),
    /// VarDeclare(name, value),
    VarDeclare(VariableDeclarationExpr<'arena>),
    /// VarDeclare(name, value, start, end)
    VarAssign(SmolStr, Box<Self>, Span),
    NamespacedVarAssign(QualifiedName<'arena>, Box<Self>, Span),
    IfBlock(IfBlockExpr<'arena>),

    /// AnonymousFunction(args, code, span)
    AnonymousFunction(Box<[(SmolStr, Option<TypeExpr<'arena>>)]>, Box<[Self]>, Span),
    WhileBlock(Box<Self>, Box<[Self]>),
    FunctionCall(FunctionCallExpr<'arena>),
    ObjFunctionCall(FunctionCallExpr<'arena>),
    FunctionDecl(FunctionDeclarationExpr<'arena>),

    ReturnVal(Box<Option<Self>>),

    ArrayGetIndex(Box<Self>, Box<Self>, Span),
    /// ArrayGetSlice(array, range_start, range_end, span)
    ArrayGetSlice(Box<Self>, Box<Self>, Box<Self>, Span),
    ArrayModify(Box<Self>, Box<Self>, Box<Self>, Span, Span),

    /// ForLoop(loop_var_name, loop_array+code, obj_markers)
    ForLoop(SmolStr, Box<Self>, Box<[Self]>, Span),
    IntForLoop(IntForLoopExpr<'arena>),
    ImportDylib(DylibImportExpr<'arena>),

    /// ImportFile(path,alias ,(start, end))
    ImportFile(SmolStr, Option<SmolStr>, Span),

    Break,
    Continue,

    EvalBlock(Box<[Self]>),
    LoopBlock(Box<[Self]>),

    /// TryCatchBlock(try_code, err_var, catch_code)
    TryCatchBlock(Box<[Self]>, SmolStr, Box<[Self]>),

    /// TypeEq(value, type, span)
    TypeEq(Box<Self>, TypeExpr<'arena>, Span),

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
        Expr::IfBlock(if_block) => {
            code_modifies_variable(var_name, &if_block.then)
                || code_modifies_variable(var_name, &if_block.otherwise)
        }
        Expr::WhileBlock(_, code)
        | Expr::EvalBlock(code)
        | Expr::LoopBlock(code)
        | Expr::ForLoop(_, _, code, _) => code_modifies_variable(var_name, code),
        Expr::IntForLoop(for_loop) => code_modifies_variable(var_name, for_loop.get_loop_code()),
        _ => false,
    })
}

pub fn var_assign<'arena>(
    target: Expr<'arena>,
    value: Expr<'arena>,
    expr_span: Span,
    value_span: Span,
) -> Expr<'arena> {
    if let Expr::Var(n, s) = target {
        Expr::VarAssign(n, Box::from(value), s)
    } else if let Expr::ArrayGetIndex(base, idx, _) = target {
        Expr::ArrayModify(base, idx, Box::from(value), expr_span, value_span)
    } else if let Expr::GetStructField(struct_expr, field, struct_span, field_span) = target {
        Expr::SetStructField(StructFieldAssignmentExpr {
            struct_expr,
            field,
            field_value: Box::from(value),
            spans: Box::new([struct_span, field_span, value_span]),
        })
    } else if let Expr::NamespacedVar(n, s) = target {
        Expr::NamespacedVarAssign(n, Box::from(value), s)
    } else {
        unsafe { unreachable_unchecked() }
    }
}

/// A span of code in a `Source`'s `contents`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
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
