use crate::ast::Span;
use crate::ast::TypeExpr;
use crate::compiler::Source;
use crate::compiler::compiler_errors::error_function_needs_args_typed;
use crate::compiler::compiler_errors::error_invalid_c_type;
use crate::compiler::compiler_errors::error_unknown_type;
use crate::compiler::compiler_errors::error_unknown_type_with_namespace;
use crate::compiler::compiler_errors::error_unknown_variable;
use crate::compiler::scope::Scope;
use crate::compiler::state::Ctx;
use crate::compiler::state::State;
use core::hint::cold_path;
use core::hint::unreachable_unchecked;
use hybrid_format::hformat;
pub use returns::check_if_returns_void;
use returns::resolve_function_return_type;

#[cfg(not(target_arch = "wasm32"))]
use crate::compiler::program::Struct;

#[cfg(not(target_arch = "wasm32"))]
use libffi::middle::Type;

mod inference;
mod returns;

#[derive(Clone, Copy)]
pub enum VmType {
    Int,
    Float,
    Bool,
    String,
    Null,
    Array,
    Struct { struct_id: u16 },
    Map,
    Fn,
}

pub const fn datatype_to_vmtype(t: &DataType) -> VmType {
    match t {
        DataType::Array(_) => VmType::Array,
        DataType::Float => VmType::Float,
        DataType::Int => VmType::Int,
        DataType::Bool => VmType::Bool,
        DataType::String => VmType::String,
        DataType::Null => VmType::Null,
        DataType::Fn(_) | DataType::FnSignature(_) => VmType::Fn,
        DataType::Struct(struct_id) => VmType::Struct { struct_id: *struct_id },
        DataType::Map(_) => VmType::Map,
        DataType::Union(_) | DataType::Unknown => unsafe { unreachable_unchecked() },
    }
}

#[derive(Debug, Clone)]
pub enum DataType {
    /// Array(None) = Unknown[]
    Array(Option<Box<Self>>),
    Float,
    Int,
    Bool,
    String,
    Null,
    Unknown,
    Union(Box<[Self]>),
    Fn(u16),
    /// Arg types followed by the return type as the last element
    FnSignature(Box<[Self]>),
    Struct(u16),
    Map(Box<(Option<Self>, Option<Self>)>),
}

impl core::fmt::Display for DataType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Float => write!(f, "float"),
            Self::Int => write!(f, "int"),
            Self::Bool => write!(f, "bool"),
            Self::String => write!(f, "string"),
            Self::Array(array_type) => match array_type {
                Some(array_type) => write!(f, "{array_type}[]"),
                None => write!(f, "T[]"),
            },
            Self::Null => write!(f, "null"),
            Self::Unknown => write!(f, "Unknown"),
            Self::Union(types) => write!(
                f,
                "{}",
                types.into_iter().map(|x| format!("{x}")).collect::<Vec<_>>().join("|")
            ),
            Self::Struct(_) => write!(f, "struct"),
            Self::Map(m) => write!(
                f,
                "{{{}: {}}}",
                m.0.as_ref().unwrap_or(&Self::Unknown),
                m.1.as_ref().unwrap_or(&Self::Unknown)
            ),
            Self::Fn(_) => write!(f, "function"),
            Self::FnSignature(sig) => {
                let (args, ret) = sig.split_at(sig.len() - 1);
                write!(
                    f,
                    "fn({}) -> {}",
                    args.iter().map(|t| format!("{t}")).collect::<Vec<_>>().join(", "),
                    ret[0]
                )
            }
        }
    }
}

impl DataType {
    pub fn format_detailed(&self, state: &State<'_, '_>) -> String {
        match self {
            Self::Float => String::from("float"),
            Self::Int => String::from("int"),
            Self::Bool => String::from("bool"),
            Self::String => String::from("string"),
            Self::Array(array_type) => match array_type {
                Some(array_type) => {
                    hformat!("{}[]", array_type.format_detailed(state))
                }
                None => String::from("T[]"),
            },
            Self::Null => String::from("null"),
            Self::Unknown => String::from("Unknown"),
            Self::Union(types) => {
                types.into_iter().map(|x| x.format_detailed(state)).collect::<Vec<_>>().join("|")
            }
            Self::Struct(s) => {
                let s = &state.structs[*s as usize];
                hformat!(
                    "{} {{{}}}",
                    s.name,
                    s.fields
                        .iter()
                        .map(|field| hformat!(
                            "{}: {}",
                            field.name,
                            field.field_type.format_detailed(state)
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Self::Map(m) => hformat!(
                "{{{}: {}}}",
                m.0.as_ref().unwrap_or(&Self::Unknown).format_detailed(state),
                m.1.as_ref().unwrap_or(&Self::Unknown).format_detailed(state)
            ),
            Self::Fn(id) => {
                let f = &state.functions[*id as usize];
                hformat!(
                    "fn ({})",
                    f.args.iter().map(|(a, _)| *a).collect::<Vec<&str>>().join(", ")
                )
            }
            Self::FnSignature(sig) => {
                let (args, ret) = sig.split_at(sig.len() - 1);
                hformat!(
                    "fn({}) -> {}",
                    args.iter().map(|t| t.format_detailed(state)).collect::<Vec<_>>().join(", "),
                    ret[0].format_detailed(state)
                )
            }
        }
    }
    #[inline(always)]
    pub const fn is_indexable(&self) -> bool {
        matches!(self, Self::String | Self::Array(_) | Self::Unknown)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn to_c_type(
        &self,
        is_return_type: bool,
        span: Span,
        structs: &[Struct],
        file_idx: u16,
        sources: &[Source],
    ) -> Type {
        match self {
            Self::Int => libffi::middle::Type::i32(),
            Self::Float => libffi::middle::Type::f64(),
            Self::String => libffi::middle::Type::pointer(),
            Self::Array(elem_type) => {
                if let Some(elem_type) = elem_type {
                    elem_type.to_c_type(false, span, structs, file_idx, sources);
                }
                libffi::middle::Type::pointer()
            }
            Self::Bool => libffi::middle::Type::u8(),
            Self::Null if is_return_type => libffi::middle::Type::void(),
            Self::Struct(id) => {
                libffi::middle::Type::structure(structs[*id as usize].fields.iter().map(|field| {
                    field.field_type.to_c_type(
                        is_return_type,
                        field.span,
                        structs,
                        file_idx,
                        sources,
                    )
                }))
            }
            invalid_type => error_invalid_c_type(invalid_type, span, file_idx, sources),
        }
    }
}

impl PartialEq for DataType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // Array(None) is compatible with any array type
            (Self::Float, Self::Float)
            | (Self::Int, Self::Int)
            | (Self::Bool, Self::Bool)
            | (Self::String, Self::String)
            | (Self::Null, Self::Null)
            | (Self::Unknown, Self::Unknown)
            | (Self::Array(_), Self::Array(None))
            | (Self::Array(None), Self::Array(_)) => true,
            (Self::Array(Some(a)), Self::Array(Some(b))) => a == b,
            #[allow(clippy::suspicious_operation_groupings)]
            (Self::Union(a), Self::Union(b)) => {
                a == b || (a.iter().all(|x| b.contains(x)) && b.iter().all(|x| a.contains(x)))
            }
            (Self::FnSignature(a), Self::FnSignature(b)) => a == b,
            (Self::Struct(a), Self::Struct(b)) => a == b,
            (Self::Fn(fn_id), Self::Fn(fn_id_2)) => fn_id == fn_id_2,
            (Self::Map(a), Self::Map(b)) => {
                (a.0.is_none() || b.0.is_none() || a.0 == b.0)
                    && (a.1.is_none() || b.1.is_none() || a.1 == b.1)
            }
            (t, Self::Union(p)) | (Self::Union(p), t) => p.contains(t),
            _ => false,
        }
    }
}

impl core::hash::Hash for DataType {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // All Array variants hash identically, which is required because Array(None) == Array(Some(_))
        match self {
            Self::Array(_) => 0u8.hash(state),
            Self::Float => 1u8.hash(state),
            Self::Int => 2u8.hash(state),
            Self::Bool => 3u8.hash(state),
            Self::String => 4u8.hash(state),
            Self::Null => 6u8.hash(state),
            Self::Unknown => 7u8.hash(state),
            Self::Union(p) => {
                8u8.hash(state);
                p.hash(state);
            }
            Self::Fn(f) => {
                9u8.hash(state);
                f.hash(state);
            }
            Self::Struct(s) => {
                10u8.hash(state);
                s.hash(state);
            }
            Self::Map(m) => {
                11u8.hash(state);
                m.hash(state);
            }
            Self::FnSignature(sig) => {
                12u8.hash(state);
                sig.hash(state);
            }
        }
    }
}

#[inline(always)]
pub fn struct_field_type_matches(expected: &DataType, received: &DataType) -> bool {
    received == &DataType::Null || expected == received
}

pub fn c_arg_matches(inferred: &DataType, declared: &DataType) -> bool {
    match (inferred, declared) {
        (DataType::Union(_), _) => false,
        (DataType::Array(None), DataType::Array(_)) => true,
        (DataType::Array(Some(a)), DataType::Array(Some(b))) => c_arg_matches(a, b),
        _ => inferred == declared,
    }
}

pub fn fn_args_match(fn_id: usize, expected_args: &[DataType], state: &State<'_, '_>) -> bool {
    state.functions[fn_id].args.len() == expected_args.len()
        && !state.functions[fn_id]
            .args
            .iter()
            .zip(expected_args)
            .any(|((_, t), expected)| t.as_ref().is_some_and(|t| t != expected))
}

/// Checks whether the function wth index `fn_id` is compatible with the signature `expected_sig`.
pub fn fn_matches_signature(
    fn_id: usize,
    expected_sig: &[DataType],
    ctx: Ctx,
    state: &mut State<'_, '_>,
) -> bool {
    let expected_arg_types = &expected_sig[..expected_sig.len() - 1];
    let expected_return_type = &expected_sig[expected_sig.len() - 1];
    if !fn_args_match(fn_id, expected_arg_types, state) {
        return false;
    }
    let fn_name = state.functions[fn_id].name;
    resolve_function_return_type(fn_id, expected_arg_types, fn_name, ctx, state)
        == *expected_return_type
}

pub fn var_type_is_compatible(declared: &DataType, candidate: &DataType) -> bool {
    match (declared, candidate) {
        (DataType::Unknown, _) => true,
        (_, DataType::Union(types)) => types.iter().all(|t| var_type_is_compatible(declared, t)),
        (DataType::Union(types), _) => types.iter().any(|t| var_type_is_compatible(t, candidate)),
        _ => declared == candidate,
    }
}

impl DataType {
    pub fn check_poly(self) -> Self {
        if let Self::Union(ref elems) = self {
            if let Some(new) = reduce_null_struct(elems) {
                return new;
            }
            let mut concrete = elems.iter().filter(|elem_type| **elem_type != Self::Unknown);
            if let Some(first_type) = concrete.next() {
                if concrete.all(|x| x == first_type) { first_type.clone() } else { self }
            } else if !elems.is_empty() {
                Self::Unknown
            } else {
                unsafe { unreachable_unchecked() }
            }
        } else {
            unsafe { unreachable_unchecked() }
        }
    }
}

fn reduce_null_struct(types: &[DataType]) -> Option<DataType> {
    let mut struct_type = None;
    for t in types {
        match t {
            DataType::Null | DataType::Unknown => {}
            DataType::Struct(_) => {
                if let Some(struct_type) = &struct_type {
                    if struct_type != t {
                        return None;
                    }
                } else {
                    struct_type = Some(t.clone());
                }
            }
            _ => return None,
        }
    }
    struct_type
}

impl TypeExpr<'_> {
    pub fn to_datatype(self, file_idx: u16, scope: &Scope, sources: &[Source]) -> DataType {
        match self {
            Self::Null => DataType::Null,
            Self::Identifier(s, span) => match s {
                "int" => DataType::Int,
                "float" => DataType::Float,
                "bool" => DataType::Bool,
                "string" => DataType::String,
                "null" => DataType::Null,
                struct_name => {
                    if let Some(struct_id) =
                        scope.find_struct(&[], struct_name, span, file_idx, sources)
                    {
                        DataType::Struct(struct_id as u16)
                    } else {
                        error_unknown_type(span, file_idx, struct_name, sources, scope);
                    }
                }
            },
            Self::NamespacedIdentifier(s, span) => {
                if let Some(struct_id) =
                    scope.find_struct(s.get_namespace(), s.get_name(), span, file_idx, sources)
                {
                    DataType::Struct(struct_id as u16)
                } else {
                    cold_path();
                    error_unknown_type_with_namespace(
                        span,
                        file_idx,
                        s.get_name(),
                        sources,
                        scope,
                        s.get_namespace(),
                    )
                }
            }
            Self::Array(inner_t) => {
                DataType::Array(Some(Box::new(inner_t.to_datatype(file_idx, scope, sources))))
            }
            Self::Map(k_t, v_t) => DataType::Map(Box::from((
                Some(k_t.to_datatype(file_idx, scope, sources)),
                Some(v_t.to_datatype(file_idx, scope, sources)),
            ))),
            Self::Union(poly) => DataType::Union(
                poly.iter().map(|t| t.to_datatype(file_idx, scope, sources)).collect(),
            )
            .check_poly(),
            Self::Function(parts) => DataType::FnSignature(
                parts.iter().map(|t| t.to_datatype(file_idx, scope, sources)).collect(),
            ),
        }
    }
}
