#[cfg(not(target_arch = "wasm32"))]
use crate::ast::DylibFnExpr;
#[cfg(not(target_arch = "wasm32"))]
use crate::ast::DylibImportExpr;
use crate::ast::Expr;
use crate::ast::FunctionDeclarationArgumentExpr;
use crate::ast::Span;
use crate::ast::VariableDeclarationExpr;
use crate::compiler::Library;
use crate::compiler::compiler_errors;
use crate::compiler::compiler_errors::error_cannot_find_dynlib_symbol;
use crate::compiler::compiler_errors::error_cannot_load_dynlib;
use crate::compiler::compiler_errors::error_cannot_read_file;
use crate::compiler::compiler_errors::error_global_already_defined;
use crate::compiler::compiler_errors::error_struct_already_defined;
use crate::compiler::program::Dylib;
use crate::compiler::program::DylibFn;
use crate::compiler::program::FnSignature;
use crate::compiler::program::Function;
use crate::compiler::program::Struct;
use crate::compiler::program::StructField;
use crate::compiler::scope::Scope;
use crate::compiler::scope::Symbol;
use crate::compiler::state::Source;
use crate::compiler::type_system::DataType;
use crate::compiler::type_system::TypeExpr;
use crate::compiler::type_system::check_if_returns_void;
use crate::compiler::type_system::collect_direct_fn_calls;
use crate::parser;
use bumpalo::Bump;
use core::cell::LazyCell;
use core::hint::unreachable_unchecked;
use hybrid_format::hformat;
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[cfg(target_os = "macos")]
const DYLIB_EXT: &str = "dylib";
#[cfg(target_os = "linux")]
const DYLIB_EXT: &str = "so";
#[cfg(target_os = "windows")]
const DYLIB_EXT: &str = "dll";

#[cfg(target_arch = "aarch64")]
const ARCH_SUFFIX: &str = "-aarch64";
#[cfg(target_arch = "x86_64")]
const ARCH_SUFFIX: &str = "-x86_64";
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
const ARCH_SUFFIX: &str = "";

/// Recursively collects functions, dyn libs, and imported files
pub fn parse_toplevel<'a>(
    bump: &'a Bump,
    code: Vec<Expr<'a>>,
    file_path: &Path,
    src_file_idx: u16,
    fns: &mut Vec<Function<'a>>,
    structs: &mut Vec<Struct<'a>>,
    dynamic_libs: &mut Vec<Dylib<'a>>,
    sources: &mut Vec<Source<'a>>,
    scope: &mut Scope<'a>,
    files: &mut FxHashMap<PathBuf, Scope<'a>>,
    file_scopes: &mut Vec<Scope<'a>>,
    pending_structs: &mut Vec<(u16, u16, &'a [(&'a str, TypeExpr<'a>, Span)])>,
    pending_fns: &mut Vec<(u16, u16, &[FunctionDeclarationArgumentExpr<'a>])>,
    #[cfg(not(target_arch = "wasm32"))] pending_dylibs: &mut Vec<(
        u16,
        u16,
        &[DylibFnExpr<'a>],
        Rc<Library>,
        Span,
    )>,
    pending_globals: &mut Vec<(VariableDeclarationExpr<'a>, u16)>,
    keel_home_libs_path: &LazyCell<Option<PathBuf>, impl FnOnce() -> Option<PathBuf>>,
) {
    let mut imports = Vec::new();
    let mut file_globals: Vec<VariableDeclarationExpr> = Vec::new();
    for expr in code {
        match expr {
            Expr::VarDeclare(var_declaration) => file_globals.push(var_declaration),
            Expr::FunctionDecl(function_declaration) => {
                let fn_name = function_declaration.name;
                let span = function_declaration.span;
                let fn_code = function_declaration.code;
                let fn_args = function_declaration.args;
                if let Some(func_idx) =
                    scope.find_function(&[], fn_name, span, src_file_idx, sources)
                {
                    compiler_errors::error_function_already_defined(
                        &fns[func_idx],
                        span,
                        src_file_idx,
                        sources,
                    );
                }
                let returns_void = check_if_returns_void(fn_code);
                let mut callees = Vec::new();
                collect_direct_fn_calls(fn_code, &mut callees);

                let fn_id = fns.len() as u16;
                fns.push(Function {
                    name: fn_name,
                    args: Box::new([]),
                    code: fn_code,
                    impls: Vec::new(),
                    returns_null: returns_void,
                    src_file_idx,
                    return_type_cache: Vec::new(),
                    direct_calls: bump.alloc_slice_copy(&callees),
                    name_span: span,
                });
                pending_fns.push((fn_id, src_file_idx, fn_args));
                if let Some(func_idx) = scope.symbols.insert((fn_name, Symbol::Fn), fn_id) {
                    compiler_errors::error_function_already_defined(
                        &fns[func_idx as usize],
                        span,
                        src_file_idx,
                        sources,
                    );
                }
            }
            Expr::StructDeclare(struct_name, fields, span) => {
                let struct_id = structs.len() as u16;
                structs.push(Struct {
                    name: struct_name,
                    fields: Box::from([]),
                    id: struct_id,
                    name_span: span,
                    src_file_idx,
                });
                if let Some(struct_id) =
                    scope.symbols.insert((struct_name, Symbol::Struct), struct_id)
                {
                    error_struct_already_defined(
                        &structs[struct_id as usize],
                        span,
                        src_file_idx,
                        sources,
                    );
                }
                pending_structs.push((struct_id, src_file_idx, fields));
            }
            #[cfg(target_arch = "wasm32")]
            Expr::ImportDylib(..) => wasm_error("WASM does not support loading dynamic libraries"),
            #[cfg(target_arch = "wasm32")]
            Expr::ImportFile(..) => wasm_error("WASM does not support importing files"),
            import @ (Expr::ImportFile(..) | Expr::ImportDylib(..)) => imports.push(import),
            _ => {}
        }
    }

    files.insert(file_path.to_path_buf(), scope.clone());

    for import in imports {
        match import {
            #[cfg(not(target_arch = "wasm32"))]
            Expr::ImportDylib(DylibImportExpr { path, functions, span }) => {
                let base_path = if Path::new(path).is_relative() {
                    &file_path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(path)
                        .to_string_lossy()
                        .to_string()
                } else {
                    path
                };
                let dylib_name = bump.alloc_str(
                    std::path::PathBuf::from(base_path)
                        .file_prefix()
                        .and_then(|s| s.to_str())
                        .unwrap_or(base_path),
                );
                // If the extension is omitted, the extension is chosen based on the target OS.
                // An architecture-specific suffix is also tried before the extension
                let lib = Rc::new(unsafe {
                    if Path::new(base_path).extension().is_none() {
                        let path = {
                            let arch_path = hformat!("{base_path}{ARCH_SUFFIX}.{DYLIB_EXT}");
                            if Path::new(&arch_path).exists() {
                                arch_path
                            } else {
                                hformat!("{base_path}.{DYLIB_EXT}")
                            }
                        };
                        libloading::Library::new(path).unwrap_or_else(|_| {
                            error_cannot_load_dynlib(span, src_file_idx, sources);
                        })
                    } else {
                        libloading::Library::new(base_path).unwrap_or_else(|_| {
                            error_cannot_load_dynlib(span, src_file_idx, sources);
                        })
                    }
                });
                pending_dylibs.push((
                    src_file_idx,
                    dynamic_libs.len() as u16,
                    functions,
                    lib,
                    span,
                ));
                dynamic_libs.push(Dylib { name: dylib_name, fns: Box::new([]) });
            }
            Expr::ImportFile(path, alias, span) => {
                let file_path = file_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(path)
                    .canonicalize()
                    .unwrap_or_else(|_| {
                        keel_home_libs_path
                            .as_ref()
                            .unwrap_or_else(
                                #[cold]
                                || error_cannot_read_file(span, src_file_idx, sources),
                            )
                            .join(path)
                    });

                let child_name = alias.unwrap_or_else(|| {
                    bump.alloc_str(file_path.file_prefix().and_then(|s| s.to_str()).unwrap_or(path))
                });

                if let Some(cached) = files.get(&file_path) {
                    scope.children.push((child_name, cached.clone()));
                    continue;
                }

                let file_contents =
                    bump.alloc_str(&std::fs::read_to_string(&file_path).unwrap_or_else(|_| {
                        error_cannot_read_file(span, src_file_idx, sources);
                    }));
                let file_name = bump.alloc_str(file_path.to_str().unwrap_or(path));

                let child_src_idx = sources.len() as u16;

                // bump.alloc_str(&file_contents);

                let src = Source { filename: file_name, contents: file_contents };

                let file_code = parser::parse(src.contents, src, bump);

                sources.push(src);

                // Parse the imported file's contents

                let mut child_scope = Scope::default();

                parse_toplevel(
                    bump,
                    file_code,
                    &file_path,
                    child_src_idx,
                    fns,
                    structs,
                    dynamic_libs,
                    sources,
                    &mut child_scope,
                    files,
                    file_scopes,
                    pending_structs,
                    pending_fns,
                    #[cfg(not(target_arch = "wasm32"))]
                    pending_dylibs,
                    pending_globals,
                    keel_home_libs_path,
                );
                files.insert(file_path, child_scope.clone());
                scope.children.push((child_name, child_scope));
            }
            _ => unsafe { unreachable_unchecked() },
        }
    }

    for var_declaration in file_globals {
        if let Some(previous_global) = scope
            .symbols
            .insert((var_declaration.name, Symbol::Global), pending_globals.len() as u16)
        {
            error_global_already_defined(
                var_declaration.name,
                pending_globals[previous_global as usize].0.span,
                var_declaration.span,
                src_file_idx,
                sources,
            );
        }
        pending_globals.push((var_declaration, src_file_idx));
    }

    if file_scopes.len() <= src_file_idx as usize {
        file_scopes.resize_with(src_file_idx as usize + 1, Scope::default);
    }
    scope.toplevel_count = scope.symbols.len();
    file_scopes[src_file_idx as usize] = scope.clone();
}

pub fn resolve_types<'arena>(
    structs: &mut [Struct<'arena>],
    fns: &mut [Function<'arena>],
    pending_structs: Vec<(u16, u16, &'arena [(&str, TypeExpr, Span)])>,
    pending_fns: Vec<(u16, u16, &[FunctionDeclarationArgumentExpr<'arena>])>,
    #[cfg(not(target_arch = "wasm32"))] pending_dylibs: Vec<(
        u16,
        u16,
        &[DylibFnExpr<'arena>],
        Rc<Library>,
        Span,
    )>,
    file_namespaces: &[Scope],
    dynamic_libs_fns: &mut Vec<DylibFn>,
    dynamic_libs: &mut [Dylib<'arena>],
    sources: &[Source],
) {
    for (struct_id, src_file_idx, fields) in pending_structs {
        let resolved_fields = fields
            .iter()
            .map(|(field_name, field_type, field_span)| StructField {
                name: field_name,
                field_type: field_type.to_datatype(
                    src_file_idx,
                    &file_namespaces[src_file_idx as usize],
                    sources,
                ),
                span: *field_span,
            })
            .collect();
        structs[struct_id as usize].fields = resolved_fields;
    }
    for (fn_id, src_file_idx, args) in pending_fns {
        let resolved_args = args
            .iter()
            .map(|arg| {
                (
                    arg.name,
                    arg.enforced_type.map(|t_e| {
                        t_e.to_datatype(
                            src_file_idx,
                            &file_namespaces[src_file_idx as usize],
                            sources,
                        )
                    }),
                )
            })
            .collect::<Vec<(&str, Option<DataType>)>>()
            .into_boxed_slice();
        fns[fn_id as usize].args = resolved_args;
    }
    #[cfg(not(target_arch = "wasm32"))]
    for (src_file_idx, dynlib_id, fn_signatures, lib, span) in pending_dylibs {
        let namespace = &file_namespaces[src_file_idx as usize];
        let fns = fn_signatures
            .iter()
            .map(|DylibFnExpr { name, args, name_span }| {
                use crate::compiler::type_system::datatype_to_vmtype;

                let (fn_return_type, fn_return_type_span) = unsafe { args.get_unchecked(0) };
                let fn_args = args
                    .iter()
                    .skip(1)
                    .map(|(t, span)| (t.to_datatype(src_file_idx, namespace, sources), *span))
                    .collect::<Vec<(DataType, Span)>>()
                    .into_boxed_slice();
                let fn_return_type = fn_return_type.to_datatype(src_file_idx, namespace, sources);
                let return_val = FnSignature {
                    name,
                    args: fn_args.iter().map(|(t, _)| t.clone()).collect(),
                    return_type: fn_return_type.clone(),
                    id: dynamic_libs_fns.len() as u16,
                };
                let arg_types: Vec<_> = fn_args
                    .iter()
                    .map(|(t, span)| t.to_c_type(false, *span, structs, src_file_idx, sources))
                    .collect();
                let return_type = fn_return_type.to_c_type(
                    true,
                    *fn_return_type_span,
                    structs,
                    src_file_idx,
                    sources,
                );
                let cif = libffi::middle::Cif::new(arg_types, return_type);
                let ptr = unsafe {
                    libffi::middle::CodePtr(
                        lib.get::<*const ()>(name.as_bytes())
                            .unwrap_or_else(|_| {
                                error_cannot_find_dynlib_symbol(
                                    name,
                                    *name_span,
                                    span,
                                    src_file_idx,
                                    sources,
                                );
                            })
                            .try_as_raw_ptr()
                            .unwrap_unchecked(),
                    )
                };

                dynamic_libs_fns.push(DylibFn {
                    types: core::iter::once(&fn_return_type)
                        .chain(fn_args.iter().map(|(t, _)| t))
                        .map(datatype_to_vmtype)
                        .collect(),
                    _lib: Rc::clone(&lib),
                    ptr,
                    cif,
                    args_len: fn_args.len(),
                });
                return_val
            })
            .collect();
        dynamic_libs[dynlib_id as usize].fns = fns;
    }
}
