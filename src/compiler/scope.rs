use crate::compiler::Source;
use crate::compiler::Span;
use crate::compiler::error_unknown_namespace;
use indexmap::IndexMap;
use rustc_hash::FxBuildHasher;

#[derive(Clone, Hash, Eq, PartialEq, Copy)]
pub enum Symbol {
    Fn,
    Struct,
    Global,
}

#[derive(Clone, Default)]
pub struct Scope<'arena> {
    pub symbols: IndexMap<(&'arena str, Symbol), u16, FxBuildHasher>,
    pub children: Vec<(&'arena str, Self)>,
    /// Number of top-level symbols
    pub toplevel_count: usize,
}

impl Scope<'_> {
    #[cold]
    pub fn fns(&self) -> impl Iterator<Item = (&str, u16)> {
        self.symbols.iter().filter(|((_, kind), _)| *kind == Symbol::Fn).map(|(&(k, _), &v)| (k, v))
    }
    #[cold]
    pub fn structs(&self) -> impl Iterator<Item = (&str, u16)> {
        self.symbols
            .iter()
            .filter(|((_, kind), _)| *kind == Symbol::Struct)
            .map(|(&(k, _), &v)| (k, v))
    }
    #[must_use]
    pub fn find_function(
        &self,
        path: &[&str],
        function_name: &str,
        span: Span,
        file_idx: u16,
        sources: &[Source],
    ) -> Option<usize> {
        self.walk_to_namespace(path)
            .unwrap_or_else(|| error_unknown_namespace(path, span, file_idx, sources))
            .symbols
            .get(&(function_name, Symbol::Fn))
            .map(|symbol| *symbol as usize)
    }
    #[must_use]
    pub fn find_function_fallible(&self, path: &[&str], function_name: &str) -> Option<usize> {
        self.walk_to_namespace(path)?
            .symbols
            .get(&(function_name, Symbol::Fn))
            .map(|symbol| *symbol as usize)
    }
    #[must_use]
    pub fn find_struct(
        &self,
        path: &[&str],
        struct_name: &str,
        span: Span,
        file_idx: u16,
        sources: &[Source],
    ) -> Option<usize> {
        self.walk_to_namespace(path)
            .unwrap_or_else(|| error_unknown_namespace(path, span, file_idx, sources))
            .symbols
            .get(&(struct_name, Symbol::Struct))
            .map(|symbol| *symbol as usize)
    }
    #[must_use]
    pub fn find_global(
        &self,
        path: &[&str],
        global_var_name: &str,
        span: Span,
        file_idx: u16,
        sources: &[Source],
    ) -> Option<usize> {
        self.walk_to_namespace(path)
            .unwrap_or_else(|| error_unknown_namespace(path, span, file_idx, sources))
            .symbols
            .get(&(global_var_name, Symbol::Global))
            .map(|symbol| *symbol as usize)
    }
    #[must_use]
    pub fn walk_to_namespace(&self, path: &[&str]) -> Option<&Self> {
        let mut current = self;
        for sub in path {
            current = if let Some((_, child_namespace)) =
                current.children.iter().find(|(name, _)| name == sub)
            {
                child_namespace
            } else {
                return None;
            };
        }
        Some(current)
    }
}
