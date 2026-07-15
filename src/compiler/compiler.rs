// use crate::compiler_data::*;
use crate::data::NULL;
use crate::errors::BLUE;
use crate::errors::BOLD;
use crate::errors::ErrType;
use crate::errors::RED;
use crate::errors::RESET;
use crate::errors::blue;
use crate::errors::red;
use crate::errors::throw_compiler_error;
use crate::errors::throw_compiler_error_exp;
#[cfg(target_arch = "wasm32")]
use crate::errors::wasm_error;
use crate::instr::LibFunc;
use crate::parser;
use crate::parser::TypeExpr;
use crate::util::parse_keel_type;
use crate::vm::Pool;
use crate::vm::UncheckedVecOps;
use crate::{data::Data, instr::Instr};
use ariadne::Label;
use ariadne::Report;
use compiler_data::Ctx;
use compiler_data::DynamicLibFn;
use compiler_data::Dynamiclib;
use compiler_data::FnSignature;
use compiler_data::Function;
use compiler_data::Pools;
use compiler_data::State;
use compiler_data::Struct;
use compiler_data::Variable;
use expr::Expr;
use expr::Span;
use expr::contains_var_reassign;
use functions::handle_functions;
use methods::handle_method_calls;
use registers::move_reg_to_reg;
use registers::move_to_id;
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use smol_strc::SmolStr;
use smol_strc::ToSmolStr;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::hint::cold_path;
use std::hint::unreachable_unchecked;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use type_system::DataType;
use type_system::check_if_returns_void;
use type_system::collect_direct_fn_calls;
use type_system::struct_field_type_matches;

#[cfg(not(target_arch = "wasm32"))]
use type_system::datatype_to_c_type;

pub mod compiler_data;
pub mod type_system;

pub mod expr;

#[path = "functions/functions.rs"]
mod functions;
#[path = "functions/methods.rs"]
mod methods;

mod registers;

pub trait UnwrapId {
    fn unwrap_id(self) -> u16;
}

impl UnwrapId for Option<u16> {
    #[inline(always)]
    fn unwrap_id(self) -> u16 {
        debug_assert!(self.is_some());
        unsafe { self.unwrap_unchecked() }
    }
}

/// Fuses the last comparison instruction into a jump instruction (jumps when condition is false)
fn add_cmp_false(condition_id: u16, len: &mut u16, output: &mut Vec<Instr>, jmp_backwards: bool) {
    if output.is_empty() {
        return output.push(Instr::IsFalseJmp(condition_id, *len));
    }
    *output.last_mut().unwrap() = match *output.last().unwrap() {
        Instr::InfFloat(o1, o2, o3) if o3 == condition_id => Instr::SupEqFloatJmp(o1, o2, *len),
        Instr::InfInt(o1, o2, o3) if o3 == condition_id => Instr::SupEqIntJmp(o1, o2, *len),
        Instr::InfEqFloat(o1, o2, o3) if o3 == condition_id => Instr::SupFloatJmp(o1, o2, *len),
        Instr::InfEqInt(o1, o2, o3) if o3 == condition_id => Instr::SupIntJmp(o1, o2, *len),
        Instr::SupFloat(o1, o2, o3) if o3 == condition_id => Instr::InfEqFloatJmp(o1, o2, *len),
        Instr::SupInt(o1, o2, o3) if o3 == condition_id => Instr::InfEqIntJmp(o1, o2, *len),
        Instr::SupEqFloat(o1, o2, o3) if o3 == condition_id => Instr::InfFloatJmp(o1, o2, *len),
        Instr::SupEqInt(o1, o2, o3) if o3 == condition_id => Instr::InfIntJmp(o1, o2, *len),
        Instr::Eq(o1, o2, o3) if o3 == condition_id => Instr::NotEqJmp(o1, o2, *len),
        Instr::ObjEq(o1, o2, o3) if o3 == condition_id => Instr::ObjNotEqJmp(o1, o2, *len),
        Instr::StrEq(o1, o2, o3) if o3 == condition_id => Instr::StrNotEqJmp(o1, o2, *len),
        Instr::NotEq(o1, o2, o3) if o3 == condition_id => Instr::EqJmp(o1, o2, *len),
        Instr::ObjNotEq(o1, o2, o3) if o3 == condition_id => Instr::ObjEqJmp(o1, o2, *len),
        Instr::StrNotEq(o1, o2, o3) if o3 == condition_id => Instr::StrEqJmp(o1, o2, *len),
        _ => {
            output.push(Instr::IsFalseJmp(condition_id, *len));
            return;
        }
    };
    if jmp_backwards {
        *len -= 1;
    }
}

/// Fuses the last comparison instruction into a jump instruction (jumps when condition is true)
#[inline(always)]
fn add_cmp_true(condition_id: u16, output: &mut Vec<Instr>) {
    if output.is_empty() {
        return output.push(Instr::IsTrueJmp(condition_id, 0));
    }
    let new_instr = match *output.last().unwrap() {
        Instr::InfFloat(o1, o2, o3) if o3 == condition_id => Instr::InfFloatJmp(o1, o2, 0),
        Instr::InfInt(o1, o2, o3) if o3 == condition_id => Instr::InfIntJmp(o1, o2, 0),
        Instr::InfEqFloat(o1, o2, o3) if o3 == condition_id => Instr::InfEqFloatJmp(o1, o2, 0),
        Instr::InfEqInt(o1, o2, o3) if o3 == condition_id => Instr::InfEqIntJmp(o1, o2, 0),
        Instr::SupFloat(o1, o2, o3) if o3 == condition_id => Instr::SupFloatJmp(o1, o2, 0),
        Instr::SupInt(o1, o2, o3) if o3 == condition_id => Instr::SupIntJmp(o1, o2, 0),
        Instr::SupEqFloat(o1, o2, o3) if o3 == condition_id => Instr::SupEqFloatJmp(o1, o2, 0),
        Instr::SupEqInt(o1, o2, o3) if o3 == condition_id => Instr::SupEqIntJmp(o1, o2, 0),
        Instr::Eq(o1, o2, o3) if o3 == condition_id => Instr::EqJmp(o1, o2, 0),
        Instr::ObjEq(o1, o2, o3) if o3 == condition_id => Instr::ObjEqJmp(o1, o2, 0),
        Instr::StrEq(o1, o2, o3) if o3 == condition_id => Instr::StrEqJmp(o1, o2, 0),
        Instr::NotEq(o1, o2, o3) if o3 == condition_id => Instr::NotEqJmp(o1, o2, 0),
        Instr::ObjNotEq(o1, o2, o3) if o3 == condition_id => Instr::ObjNotEqJmp(o1, o2, 0),
        Instr::StrNotEq(o1, o2, o3) if o3 == condition_id => Instr::StrNotEqJmp(o1, o2, 0),
        _ => {
            output.push(Instr::IsTrueJmp(condition_id, 0));
            return;
        }
    };
    *output.last_mut().unwrap() = new_instr;
}

/// Sets the jump size field of a jump instruction
#[inline(always)]
fn set_jmp_size(instr: &mut Instr, size: u16) {
    match instr {
        Instr::IsFalseJmp(_, jump_size)
        | Instr::IsTrueJmp(_, jump_size)
        | Instr::Jmp(jump_size)
        | Instr::SupEqFloatJmp(_, _, jump_size)
        | Instr::SupEqIntJmp(_, _, jump_size)
        | Instr::SupFloatJmp(_, _, jump_size)
        | Instr::SupIntJmp(_, _, jump_size)
        | Instr::InfEqFloatJmp(_, _, jump_size)
        | Instr::InfEqIntJmp(_, _, jump_size)
        | Instr::InfFloatJmp(_, _, jump_size)
        | Instr::InfIntJmp(_, _, jump_size)
        | Instr::InfIntJmpBack(_, _, jump_size)
        | Instr::NotEqJmp(_, _, jump_size)
        | Instr::EqJmp(_, _, jump_size)
        | Instr::ObjNotEqJmp(_, _, jump_size)
        | Instr::ObjEqJmp(_, _, jump_size)
        | Instr::StrNotEqJmp(_, _, jump_size)
        | Instr::StrEqJmp(_, _, jump_size) => *jump_size = size,
        _ => unreachable!(),
    }
}

/// Compiles short-circuit && and || conditions
/// bool_or_mode true indicates left side of ||, emits true jumps
/// bool_or_mode false emits false jumps
/// Returns (true_jump_idxs, false_jump_idxs)
#[allow(clippy::too_many_arguments)]
fn compile_short_circuit_condition(
    expr: &Expr,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
    bool_or_mode: bool,
) -> (Vec<usize>, Vec<usize>) {
    match expr {
        Expr::BoolOr(left, right, _) => {
            // left side of || always uses true jump mode
            let (mut true_jumps, _) =
                compile_short_circuit_condition(left, v, ctx, state, output, true);
            let (right_true, right_false) =
                compile_short_circuit_condition(right, v, ctx, state, output, bool_or_mode);
            true_jumps.extend(right_true);
            (true_jumps, right_false)
        }
        Expr::BoolAnd(left, right, _) => {
            if bool_or_mode {
                // && inside left side of ||
                let id_l = left
                    .compile(v, ctx, state, output, None, false, true)
                    .unwrap_id();
                let id_r = right
                    .compile(v, ctx, state, output, None, false, true)
                    .unwrap_id();
                state.free_reg(id_l, v);
                state.free_reg(id_r, v);
                let id = state.alloc_reg();
                output.push(Instr::BoolAnd(id_l, id_r, id));
                add_cmp_true(id, output);
                state.free_reg(id, v);
                (vec![output.len() - 1], Vec::new())
            } else {
                // normal && -> if either side is false, jump past the body
                let (_, mut false_jumps) =
                    compile_short_circuit_condition(left, v, ctx, state, output, false);
                let (_, right_false) =
                    compile_short_circuit_condition(right, v, ctx, state, output, false);
                false_jumps.extend(right_false);
                (Vec::new(), false_jumps)
            }
        }
        expr => {
            let cond_id = expr
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            if bool_or_mode {
                add_cmp_true(cond_id, output);
                state.free_reg(cond_id, v);
                (vec![output.len() - 1], Vec::new())
            } else {
                add_cmp_false(cond_id, &mut 0, output, false);
                state.free_reg(cond_id, v);
                (Vec::new(), vec![output.len() - 1])
            }
        }
    }
}

fn parse_loop_flow_control(
    loop_code: &mut [Instr],
    loop_id: u16,
    code_length: u16,
    for_loop: bool,
    indefinite: bool,
) {
    loop_code.iter_mut().enumerate().for_each(|(i, x)| {
        if let Instr::NotEqJmp(break_id, 0, 0) = x
            && *break_id == loop_id
        {
            if for_loop && !indefinite {
                *x = Instr::Jmp(code_length - i as u16 - 1);
            } else {
                *x = Instr::Jmp(code_length - i as u16);
            }
        } else if let Instr::EqJmp(continue_id, 0, 0) = x
            && *continue_id == loop_id
        {
            if for_loop {
                *x = Instr::Jmp(code_length - i as u16 - 3);
            } else {
                // loop blocks and while loops only have 1 trailing instruction
                *x = Instr::Jmp(code_length - i as u16 - 1);
            }
        }
    });
}

pub fn walk_namespace_struct(root: &Namespace, path: &[SmolStr], fn_name: &str) -> Option<usize> {
    let mut current = root;
    for sub in path {
        current = current.children.iter().find(|n| n.name == *sub)?;
    }
    current
        .structs
        .iter()
        .find(|(n, _)| n.as_str() == fn_name)
        .map(|(_, id)| *id as usize)
}

pub fn find_struct(
    root: &Namespace,
    structs: &[Struct],
    namespace: &[SmolStr],
    name: &str,
) -> Option<usize> {
    if let Some(idx) = walk_namespace_struct(root, namespace, name) {
        Some(idx)
    } else if namespace.is_empty() {
        structs.iter().rposition(|s| s.name == name)
    } else {
        None
    }
}

#[inline(never)]
#[cold]
fn error_array_diff_types(
    src: (&str, &str),
    sources: &[(SmolStr, Rc<String>)],
    array_span: Span,
    array_elem_type: &DataType,
    failing_elem_span: Span,
    failing_elem_type: &DataType,
) -> ! {
    throw_compiler_error_exp(
        || {
            Report::build(ariadne::ReportKind::Error, (src.0, array_span.into()))
                .with_message("Invalid array types")
                .with_label(
                    Label::new((src.0, array_span.into()))
                        .with_message(format_args!(
                            "This expression is of type {}",
                            blue(array_elem_type)
                        ))
                        .with_color(ariadne::Color::Blue),
                )
                .with_label(
                    Label::new((src.0, failing_elem_span.into()))
                        .with_message(format_args!(
                            "This expression is of type {}",
                            red(failing_elem_type),
                        ))
                        .with_color(ariadne::Color::Red),
                )
                .with_note("Arrays are homogeneous and can only hold elements of a single type")
                .finish()
        },
        sources,
    );
}

#[inline(always)]
fn compile_array_literal(
    array_items: &[Expr],
    spans: &[Span],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    if let Some(first) = array_items.first() {
        let first_type = first.infer_type(v, ctx, state);
        if let Some(failing_elem_idx) = array_items
            .iter()
            .skip(1)
            .position(|x| x.infer_type(v, ctx, state) != first_type)
        {
            let failing_elem_type = array_items[failing_elem_idx + 1].infer_type(v, ctx, state);
            let failing_elem_span = spans[failing_elem_idx + 2];
            error_array_diff_types(
                ctx.src,
                state.sources,
                spans[1],
                &first_type,
                failing_elem_span,
                &failing_elem_type,
            )
        }
    }
    let array_id = {
        state.pools.objs.push(Vec::with_capacity(array_items.len()));
        state.pools.objs.len() - 1
    };
    if array_items.is_empty() && !ctx.single_run {
        let array_reg = {
            state.registers.push(Data::array(array_id as u32));
            state.registers.len() - 1
        } as u16;
        output.push(Instr::EmptyArray(array_reg));
        return array_reg;
    }
    if ctx.single_run {
        for elem in array_items {
            let id = elem
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            if elem.is_constant_literal() {
                state
                    .pools
                    .objs
                    .get_mut(array_id)
                    .push(state.registers[id as usize]);
            } else {
                output.push(Instr::ObjElemMov(
                    id,
                    array_id as u16,
                    state.pools.objs[array_id].len() as u16,
                ));
                state.pools.objs.get_mut(array_id).push(NULL);
            }
        }
        state.registers.push(Data::array(array_id as u32));
        (state.registers.len() - 1) as u16
    } else {
        // Check if all elements are constant (no instructions emitted)
        let mut constant_array = true;
        let mut elem_ids: Vec<u16> = Vec::with_capacity(array_items.len());
        for elem in array_items {
            let id = elem
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            if elem.is_constant_literal() {
                state
                    .pools
                    .objs
                    .get_mut(array_id)
                    .push(state.registers[id as usize]);
            } else {
                constant_array = false;
                state.pools.objs.get_mut(array_id).push(NULL);
            }
            elem_ids.push(id);
        }

        if constant_array {
            // The template array is held by a register to prevent it from being freed by the GC
            let template_reg = {
                state.registers.push(Data::array(array_id as u32));
                (state.registers.len() - 1) as u16
            };
            let dest_reg = {
                state.registers.push(Data::array(0)); // 0 is a placeholder that's overwritten by EmptyArray
                (state.registers.len() - 1) as u16
            };
            output.push(Instr::CloneArray(
                template_reg,
                dest_reg,
                state.pools.objs[array_id].len() as u16,
            ));
            dest_reg
        } else {
            let dest_reg = {
                state.registers.push(Data::array(0)); // 0 is a placeholder that's overwritten by EmptyArray
                (state.registers.len() - 1) as u16
            };
            output.push(Instr::EmptyArray(dest_reg));
            for elem_reg in elem_ids {
                output.push(Instr::Push(dest_reg, elem_reg));
            }
            dest_reg
        }
    }
}

#[inline(never)]
#[cold]
pub fn error_unknown_struct(
    struct_name: &SmolStr,
    struct_span: Span,
    sources: &[(SmolStr, Rc<String>)],
    src: (&str, &str),
) -> ! {
    throw_compiler_error_exp(
        || {
            Report::build(ariadne::ReportKind::Error, (src.0, struct_span.into()))
                .with_message("Unknown struct")
                .with_label(
                    Label::new((src.0, struct_span.into()))
                        .with_message(format_args!("Unknown struct {}", red(struct_name)))
                        .with_color(ariadne::Color::Red),
                )
                .finish()
        },
        sources,
    );
}

fn error_struct_no_such_field(
    src: (&str, &str),
    struct_name: &SmolStr,
    struct_span: Span,
    struct_field_span: Span,
    struct_field_name: &SmolStr,
    sources: &[(SmolStr, Rc<String>)],
) -> ! {
    throw_compiler_error_exp(
        || {
            let report = Report::build(
                ariadne::ReportKind::Error,
                (src.0, struct_field_span.into()),
            )
            .with_message("Unknown struct field")
            .with_label(
                Label::new((src.0, struct_span.into()))
                    .with_message(format_args!("Struct defined here"))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.0, struct_field_span.into()))
                    .with_message(format_args!(
                        "There is no field {} in {}",
                        red(struct_field_name),
                        blue(struct_name)
                    ))
                    .with_color(ariadne::Color::Red),
            );

            report.finish()
        },
        sources,
    );
}

fn error_struct_missing_fields(
    src: (&str, &str),
    struct_span: Span,
    struct_literal_span: Span,
    sources: &[(SmolStr, Rc<String>)],
    missing_fields: &[&SmolStr],
) -> ! {
    throw_compiler_error_exp(
        || {
            let report = Report::build(
                ariadne::ReportKind::Error,
                (src.0, struct_literal_span.into()),
            )
            .with_message("Missing struct fields")
            .with_label(
                Label::new((src.0, struct_span.into()))
                    .with_message(format_args!("Struct defined here"))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.0, struct_literal_span.into()))
                    .with_message(format_args!(
                        "This is missing field{} {}",
                        if missing_fields.len() > 1 { "s" } else { "" },
                        missing_fields
                            .iter()
                            .map(|f| blue(f).to_smolstr())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .with_color(ariadne::Color::Red),
            );

            report.finish()
        },
        sources,
    );
}

fn compile_struct_literal(
    namespace: &[SmolStr],
    fields: &[(SmolStr, Expr, Span, Span)],
    span: Span,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let name = &namespace[namespace.len() - 1];
    let namespace = &namespace[..(namespace.len() - 1)];
    let Some(expected_struct_idx) = find_struct(state.namespace, state.structs, namespace, name)
    else {
        error_unknown_struct(name, span, state.sources, ctx.src);
    };
    let type_id = state.structs[expected_struct_idx].id;
    let expected_fields_len = state.structs[expected_struct_idx].fields.len();
    if expected_fields_len < fields.len() {
        let unexpected_field = &fields[expected_fields_len];
        error_struct_no_such_field(
            ctx.src,
            name,
            state.structs[expected_struct_idx].name_span,
            unexpected_field.2,
            &unexpected_field.0,
            state.sources,
        )
    }
    let struct_id = {
        state.pools.objs.push(Vec::with_capacity(fields.len()));
        state.pools.objs.len() - 1
    };
    if ctx.single_run {
        for field_idx in 0..expected_fields_len {
            if let Some((_, field_expr, _, field_value_span)) = fields
                .iter()
                .find(|(f, _, _, _)| f == &state.structs[expected_struct_idx].fields[field_idx].0)
            {
                let field_type = field_expr.infer_type(v, ctx, state);
                let field = &state.structs[expected_struct_idx].fields[field_idx];
                if !struct_field_type_matches(&field.1, &field_type) {
                    error_struct_field_invalid_type(
                        ctx.src,
                        name,
                        field.2,
                        &field.0,
                        &field.1,
                        *field_value_span,
                        &field_type,
                        state.sources,
                    );
                }
                let id = field_expr
                    .compile(v, ctx, state, output, None, false, true)
                    .unwrap_id();
                if field_expr.is_constant_literal() {
                    state
                        .pools
                        .objs
                        .get_mut(struct_id)
                        .push(state.registers[id as usize]);
                } else {
                    output.push(Instr::ObjElemMov(
                        id,
                        struct_id as u16,
                        state.pools.objs[struct_id].len() as u16,
                    ));
                    state.pools.objs.get_mut(struct_id).push(NULL);
                }
            } else {
                let missing_elems = (0..expected_fields_len)
                    .into_iter()
                    .filter(|i| {
                        !fields.iter().any(|(f, _, _, _)| {
                            f == &state.structs[expected_struct_idx].fields[*i].0
                        })
                    })
                    .map(|i| &state.structs[struct_id].fields[i].0)
                    .collect::<Vec<&SmolStr>>();
                error_struct_missing_fields(
                    ctx.src,
                    state.structs[expected_struct_idx].name_span,
                    span,
                    state.sources,
                    &missing_elems,
                )
            }
        }

        state
            .registers
            .push(Data::struct_instance(type_id, struct_id as u32));
        (state.registers.len() - 1) as u16
    } else {
        let mut dynamic: Vec<(u16, u16)> = Vec::with_capacity(expected_fields_len);
        for field_idx in 0..expected_fields_len {
            if let Some((_, field_expr, _, field_value_span)) = fields
                .iter()
                .find(|(f, _, _, _)| f == &state.structs[expected_struct_idx].fields[field_idx].0)
            {
                let field_type = field_expr.infer_type(v, ctx, state);
                let field = &state.structs[expected_struct_idx].fields[field_idx];
                if !struct_field_type_matches(&field.1, &field_type) {
                    error_struct_field_invalid_type(
                        ctx.src,
                        name,
                        field.2,
                        &field.0,
                        &field.1,
                        *field_value_span,
                        &field_type,
                        state.sources,
                    );
                }
                let id = field_expr
                    .compile(v, ctx, state, output, None, false, true)
                    .unwrap_id();
                if field_expr.is_constant_literal() {
                    state
                        .pools
                        .objs
                        .get_mut(struct_id)
                        .push(state.registers[id as usize]);
                } else {
                    state.pools.objs.get_mut(struct_id).push(NULL);
                    dynamic.push((id, field_idx as u16));
                }
            } else {
                let missing_elems = (0..expected_fields_len)
                    .into_iter()
                    .filter(|i| {
                        !fields.iter().any(|(f, _, _, _)| {
                            f == &state.structs[expected_struct_idx].fields[*i].0
                        })
                    })
                    .map(|i| &state.structs[struct_id].fields[i].0)
                    .collect::<Vec<&SmolStr>>();
                error_struct_missing_fields(
                    ctx.src,
                    state.structs[expected_struct_idx].name_span,
                    span,
                    state.sources,
                    &missing_elems,
                );
            }
        }

        let template_reg = {
            state
                .registers
                .push(Data::struct_instance(type_id, struct_id as u32));
            (state.registers.len() - 1) as u16
        };
        let dest_reg = {
            state.registers.push(Data::struct_instance(type_id, 0));
            (state.registers.len() - 1) as u16
        };
        output.push(Instr::CloneStruct(template_reg, dest_reg));
        for (val_reg, slot) in dynamic {
            output.push(Instr::SetFieldStruct(dest_reg, val_reg, slot));
        }
        dest_reg
    }
}

fn compile_map_literal(
    kv_pairs: &[(Expr, Expr)],
    span: Span,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let mut global_key_type: DataType = DataType::Unknown;
    let mut global_val_type: DataType = DataType::Unknown;
    let map_id = state.pools.maps.len();
    state.pools.maps.push(HashMap::with_capacity_and_hasher(
        kv_pairs.len(),
        BuildHasherDefault::default(),
    ));
    if ctx.single_run {
        for (i, (key, val)) in kv_pairs.iter().enumerate() {
            if kv_pairs.iter().skip(i + 1).any(|(k, _)| k == key) {
                throw_compiler_error(ctx.src, span, ErrType::DuplicateMapKey);
            }
            let key_t = key.infer_type(v, ctx, state);
            let val_t = val.infer_type(v, ctx, state);
            if i == 0 {
                global_key_type = key_t;
                global_val_type = val_t;
            } else {
                key_t.expect(&global_key_type, ctx.src, span);
                val_t.expect(&global_val_type, ctx.src, span);
            }
            let output_len = output.len();
            let key_val_id = key
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            if !(key.is_constant_literal()
                || matches!(key, Expr::Array(_, _)) && output_len == output.len())
            {
                throw_compiler_error(ctx.src, span, ErrType::NotLiteralMapKey);
            }
            let key_val = state.registers[key_val_id as usize];
            let id = val
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            if val.is_constant_literal() {
                state.pools.maps[map_id].insert(key_val, state.registers[id as usize]);
            } else {
                state.pools.maps[map_id].insert(key_val, NULL);
                output.push(Instr::MapInsert(
                    map_id as u16,
                    state.registers.len() as u16,
                    id,
                ));
                state.registers.push(key_val);
            }
        }
        let dest_id = state.registers.len();
        state.registers.push(Data::map(map_id as u32));
        dest_id as u16
    } else {
        let mut dynamic: Vec<(Data, u16)> = Vec::with_capacity(kv_pairs.len());
        for (i, (key, val)) in kv_pairs.iter().enumerate() {
            if kv_pairs.iter().skip(i + 1).any(|(k, _)| k == key) {
                throw_compiler_error(ctx.src, span, ErrType::DuplicateMapKey);
            }
            let key_t = key.infer_type(v, ctx, state);
            let val_t = val.infer_type(v, ctx, state);
            if i == 0 {
                global_key_type = key_t;
                global_val_type = val_t;
            } else {
                key_t.expect(&global_key_type, ctx.src, span);
                val_t.expect(&global_val_type, ctx.src, span);
            }
            let output_len = output.len();
            let key_val_id = key
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            if !(key.is_constant_literal()
                || matches!(key, Expr::Array(_, _)) && output_len == output.len())
            {
                throw_compiler_error(ctx.src, span, ErrType::NotLiteralMapKey);
            }
            let key_val = state.registers[key_val_id as usize];
            let val_id = val
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            if val.is_constant_literal() {
                state.pools.maps[map_id].insert(key_val, state.registers[val_id as usize]);
            } else {
                state.pools.maps[map_id].insert(key_val, NULL);
                dynamic.push((key_val, val_id));
            }
        }

        let template_reg = {
            state.registers.push(Data::map(map_id as u32));
            (state.registers.len() - 1) as u16
        };
        let dest_reg = {
            state.registers.push(Data::map(0));
            (state.registers.len() - 1) as u16
        };
        output.push(Instr::CloneMap(template_reg, dest_reg));
        for (key_val, val_id) in dynamic {
            let key_reg = if let Some(&id) = state.const_registers.get(&key_val) {
                id
            } else {
                let id = state.registers.len() as u16;
                state.const_registers.insert(key_val, id);
                state.registers.push(key_val);
                id
            };
            output.push(Instr::MapInsertReg(dest_reg, key_reg, val_id));
        }
        dest_reg
    }
}

fn compile_struct_field_access(
    struct_expr: &Expr,
    field: &SmolStr,
    struct_span: Span,
    field_span: Span,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let t = struct_expr.infer_type(v, ctx, state);
    if let DataType::Struct(s_id) = t {
        let s = &state.structs[s_id as usize];
        let idx = s
            .fields
            .iter()
            .position(|f| &f.0 == field)
            .unwrap_or_else(|| {
                error_struct_unknown_field(
                    ctx.src,
                    field_span,
                    field,
                    &s.name,
                    &s.fields,
                    state.sources,
                );
            });
        let id = struct_expr
            .compile(v, ctx, state, output, None, false, true)
            .unwrap_id();
        let dest_reg_id = state.alloc_reg();
        output.push(Instr::GetFieldStruct(id, idx as u16, dest_reg_id));
        dest_reg_id
    } else {
        throw_compiler_error(
            ctx.src,
            struct_span,
            ErrType::InvalidType(&DataType::Struct(0), &t),
        );
    }
}

#[inline(never)]
#[cold]
fn error_struct_unknown_field(
    src: (&str, &str),
    field_span: Span,
    field: &SmolStr,
    struct_name: &SmolStr,
    fields: &[(SmolStr, DataType, Span)],
    sources: &[(SmolStr, Rc<String>)],
) -> ! {
    throw_compiler_error_exp(
        || {
            Report::build(ariadne::ReportKind::Error, (src.0, field_span.into()))
                .with_message("Unknown field")
                .with_label(
                    Label::new((src.0, field_span.into()))
                        .with_message(format_args!(
                            "The field {} isn't defined in struct {}",
                            red(field),
                            blue(struct_name)
                        ))
                        .with_color(ariadne::Color::Red),
                )
                .with_help(format_args!(
                    "The available fields are: {}",
                    fields
                        .iter()
                        .map(|(field, _, _)| blue(field))
                        .collect::<Vec<_>>()
                        .join(", "),
                ))
                .finish()
        },
        sources,
    );
}

#[cold]
#[inline(never)]
fn error_struct_field_invalid_type(
    src: (&str, &str),
    struct_name: &SmolStr,
    struct_field_span: Span,
    struct_field_name: &SmolStr,
    struct_field_type: &DataType,
    value_span: Span,
    value_type: &DataType,
    sources: &[(SmolStr, Rc<String>)],
) -> ! {
    throw_compiler_error_exp(
        || {
            let mut report = Report::build(
                ariadne::ReportKind::Error,
                (src.0, struct_field_span.into()),
            )
            .with_message("Incompatible types")
            .with_label(
                Label::new((src.0, struct_field_span.into()))
                    .with_message(format_args!(
                        "Field {} in struct {} expects type {}",
                        blue(struct_field_name),
                        blue(struct_name),
                        blue(struct_field_type)
                    ))
                    .with_color(ariadne::Color::Blue),
            )
            .with_label(
                Label::new((src.0, value_span.into()))
                    .with_message(format_args!(
                        "This expression is of type {}",
                        red(value_type)
                    ))
                    .with_color(ariadne::Color::Red),
            );

            if struct_field_type == &DataType::Int
                && (value_type == &DataType::Float || value_type == &DataType::String)
            {
                report = report.with_help(format_args!("Try using the {} function", blue("int()")));
            } else if struct_field_type == &DataType::Float
                && (value_type == &DataType::Int || value_type == &DataType::String)
            {
                report =
                    report.with_help(format_args!("Try using the {} function", blue("float()")));
            } else if struct_field_type == &DataType::Bool && value_type == &DataType::String {
                report =
                    report.with_help(format_args!("Try using the {} function", blue("bool()")));
            } else if struct_field_type == &DataType::String {
                report = report.with_help(format_args!("Try using the {} function", blue("str()")));
            }

            report.finish()
        },
        sources,
    );
}

fn compile_array_indexing(
    array: &Expr,
    index: &Expr,
    span: Span,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let infered = array.infer_type(v, ctx, state);
    if !infered.is_indexable() {
        throw_compiler_error(ctx.src, span, ErrType::NotIndexable(&infered));
    }

    let id = array
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();

    let index_inferred = index.infer_type(v, ctx, state);
    if index_inferred != DataType::Int {
        throw_compiler_error(ctx.src, span, ErrType::InvalidIndexType(&index_inferred));
    }
    let index_id = index
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    state.free_reg(index_id, v);
    let dest_reg_id = state.alloc_reg();

    let to_push = if infered == DataType::String {
        Instr::GetIndexString(id, index_id, dest_reg_id)
    } else {
        Instr::GetIndexArray(id, index_id, dest_reg_id)
    };
    state.instr_src.push((to_push, span, ctx.current_src_file));
    output.push(to_push);
    dest_reg_id
}

fn compile_array_slice(
    array: &Expr,
    idx_start: &Expr,
    idx_end: &Expr,
    span: Span,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let infered = array.infer_type(v, ctx, state);
    if !infered.is_indexable() {
        throw_compiler_error(ctx.src, span, ErrType::NotIndexable(&infered));
    }
    let id = array
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    let idx_start_inferred = idx_start.infer_type(v, ctx, state);
    if idx_start_inferred != DataType::Int {
        throw_compiler_error(
            ctx.src,
            span,
            ErrType::InvalidIndexType(&idx_start_inferred),
        );
    }
    let idx_start_id = idx_start
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    let idx_end_inferred = idx_end.infer_type(v, ctx, state);
    if idx_end_inferred != DataType::Int {
        throw_compiler_error(ctx.src, span, ErrType::InvalidIndexType(&idx_end_inferred));
    }
    let idx_end_id = idx_end
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    output.push(Instr::StoreFuncArg(idx_end_id));
    state.free_reg(idx_start_id, v);
    state.free_reg(idx_end_id, v);
    let dest_reg_id = state.alloc_reg();
    let to_push = if infered == DataType::String {
        Instr::GetSliceString(id, idx_start_id, dest_reg_id)
    } else {
        Instr::GetSliceArray(id, idx_start_id, dest_reg_id)
    };
    state.instr_src.push((to_push, span, ctx.current_src_file));
    output.push(to_push);
    dest_reg_id
}

fn uniform_op(
    instr: fn(u16, u16, u16) -> Instr,
    symbol: &'static str,
    l: &Expr,
    r: &Expr,
    span: Span,
    t: &DataType,
    tgt_id: Option<u16>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let (t_l, t_r) = (l.infer_type(v, ctx, state), r.infer_type(v, ctx, state));
    if &t_l != t || &t_r != t {
        throw_compiler_error(ctx.src, span, ErrType::OpError(&t_l, &t_r, symbol))
    }

    let id_l = l
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    let id_r = r
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    state.free_reg(id_l, v);
    state.free_reg(id_r, v);
    let id = state.alloc_reg_tgt(tgt_id);
    output.push(instr(id_l, id_r, id));
    id
}

#[inline]
fn uniform_op2(
    instr: fn(u16, u16, u16) -> Instr,
    t_1: &'static DataType,
    instr2: fn(u16, u16, u16) -> Instr,
    t_2: &'static DataType,
    symbol: &'static str,
    l: &Expr,
    r: &Expr,
    span: Span,
    tgt_id: Option<u16>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let (t_l, t_r) = (l.infer_type(v, ctx, state), r.infer_type(v, ctx, state));
    if !((&t_l == t_1 && &t_r == t_1) || (&t_l == t_2 && &t_r == t_2)) {
        throw_compiler_error(ctx.src, span, ErrType::OpError(&t_l, &t_r, symbol))
    }
    let id_l = l
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    let id_r = r
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    state.free_reg(id_l, v);
    state.free_reg(id_r, v);
    let id = state.alloc_reg_tgt(tgt_id);
    output.push(if &t_l == t_1 {
        instr(id_l, id_r, id)
    } else {
        instr2(id_l, id_r, id)
    });
    id
}

fn compile_div_op(
    l: &Expr,
    r: &Expr,
    span: Span,
    tgt_id: Option<u16>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    if let Expr::Int(n) = r
        && *n == 0
    {
        throw_compiler_error(ctx.src, span, ErrType::DivisionByZero);
    }
    let id = uniform_op2(
        Instr::DivFloat,
        &DataType::Float,
        Instr::DivInt,
        &DataType::Int,
        "/",
        l,
        r,
        span,
        tgt_id,
        v,
        ctx,
        state,
        output,
    );
    if matches!(output.last(), Some(Instr::DivInt(..))) {
        state
            .instr_src
            .push((*output.last().unwrap(), span, ctx.current_src_file));
    }
    id
}

fn compile_add_op(
    l: &Expr,
    r: &Expr,
    span: Span,
    tgt_id: Option<u16>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let t_l = l.infer_type(v, ctx, state);
    let t_r = r.infer_type(v, ctx, state);
    if t_l != t_r
        || !matches!(
            t_l,
            DataType::String | DataType::Array(_) | DataType::Float | DataType::Int
        )
    {
        throw_compiler_error(ctx.src, span, ErrType::OpError(&t_l, &t_r, "+"));
    }
    // var+1 or 1+var use the dedicated IncInt/IncIntTo instructions
    if t_l == DataType::Int
        && let Some(Expr::Var(src_name, _)) = {
            if matches!(r, Expr::Int(1)) {
                Some(l)
            } else if matches!(l, Expr::Int(1)) {
                Some(r)
            } else {
                None
            }
        }
        && let Some(src_var) = v.iter().rfind(|x| x.name == *src_name)
    {
        let src_id = src_var.register_id;
        let id = tgt_id.unwrap_or_else(|| state.alloc_reg());
        output.push(if src_id == id {
            Instr::IncInt(id)
        } else {
            Instr::IncIntTo(src_id, id)
        });
        return id;
    }
    let id_l = l
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    let id_r = r
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    state.free_reg(id_l, v);
    state.free_reg(id_r, v);
    let id = state.alloc_reg_tgt(tgt_id);
    if matches!(t_l, DataType::Array(_)) {
        output.push(Instr::AddArray(id_l, id_r, id));
    } else if t_l == DataType::String {
        output.push(Instr::AddStr(id_l, id_r, id));
    } else if t_l == DataType::Float {
        output.push(Instr::AddFloat(id_l, id_r, id));
    } else {
        output.push(Instr::AddInt(id_l, id_r, id));
    }
    id
}

fn compile_sub_op(
    l: &Expr,
    r: &Expr,
    span: Span,
    tgt_id: Option<u16>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let t_l = l.infer_type(v, ctx, state);
    let t_r = r.infer_type(v, ctx, state);
    if !((t_l == DataType::Float && t_r == DataType::Float)
        || (t_l == DataType::Int && t_r == DataType::Int))
    {
        throw_compiler_error(ctx.src, span, ErrType::OpError(&t_l, &t_r, "-"));
    }
    // var-1 uses the dedicated DecInt/DecIntTo instructions
    if t_l == DataType::Int
        && matches!(r, Expr::Int(1))
        && let Expr::Var(src_name, _) = l
        && let Some(src_var) = v.iter().rfind(|x| x.name == *src_name)
    {
        let src_id = src_var.register_id;
        let id = tgt_id.unwrap_or_else(|| state.alloc_reg());
        output.push(if src_id == id {
            Instr::DecInt(id)
        } else {
            Instr::DecIntTo(src_id, id)
        });
        return id;
    }
    let id_l = l
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    let id_r = r
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    state.free_reg(id_l, v);
    state.free_reg(id_r, v);
    let id = state.alloc_reg_tgt(tgt_id);
    output.push(if t_l == DataType::Float {
        Instr::SubFloat(id_l, id_r, id)
    } else {
        Instr::SubInt(id_l, id_r, id)
    });
    id
}

fn compile_mod_op(
    l: &Expr,
    r: &Expr,
    span: Span,
    tgt_id: Option<u16>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    if let Expr::Int(n) = r
        && *n == 0
    {
        throw_compiler_error(ctx.src, span, ErrType::ModuloByZero);
    }
    let id = uniform_op2(
        Instr::ModFloat,
        &DataType::Float,
        Instr::ModInt,
        &DataType::Int,
        "%",
        l,
        r,
        span,
        tgt_id,
        v,
        ctx,
        state,
        output,
    );
    if matches!(output.last(), Some(Instr::ModInt(..))) {
        state
            .instr_src
            .push((*output.last().unwrap(), span, ctx.current_src_file));
    }
    id
}

fn compile_eq_op(
    l: &Expr,
    r: &Expr,
    tgt_id: Option<u16>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let l_type = l.infer_type(v, ctx, state);
    let r_type = r.infer_type(v, ctx, state);
    let is_array = matches!(l_type, DataType::Array(_) | DataType::Struct(_))
        && matches!(r_type, DataType::Array(_) | DataType::Struct(_));
    let is_string = l_type == DataType::String || r_type == DataType::String;
    let id_l = l
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    let id_r = r
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    state.free_reg(id_l, v);
    state.free_reg(id_r, v);
    let id = state.alloc_reg_tgt(tgt_id);
    output.push(if is_array {
        Instr::ObjEq(id_l, id_r, id)
    } else if is_string {
        Instr::StrEq(id_l, id_r, id)
    } else {
        Instr::Eq(id_l, id_r, id)
    });
    id
}

fn compile_neq_op(
    l: &Expr,
    r: &Expr,
    tgt_id: Option<u16>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let l_type = l.infer_type(v, ctx, state);
    let r_type = r.infer_type(v, ctx, state);
    let is_array = matches!(l_type, DataType::Array(_) | DataType::Struct(_))
        && matches!(r_type, DataType::Array(_) | DataType::Struct(_));
    let is_string = l_type == DataType::String || r_type == DataType::String;
    let id_l = l
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    let id_r = r
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    state.free_reg(id_l, v);
    state.free_reg(id_r, v);
    let id = state.alloc_reg_tgt(tgt_id);
    if is_array {
        output.push(Instr::ObjNotEq(id_l, id_r, id));
    } else if is_string {
        output.push(Instr::StrNotEq(id_l, id_r, id));
    } else {
        output.push(Instr::NotEq(id_l, id_r, id));
    }
    id
}

fn compile_neg_op(
    l: &Expr,
    span: Span,
    tgt_id: Option<u16>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let operand_type = l.infer_type(v, ctx, state);
    let id_l = l
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    state.free_reg(id_l, v);
    let id = state.alloc_reg_tgt(tgt_id);
    if operand_type == DataType::Float {
        output.push(Instr::NegFloat(id_l, id));
    } else if operand_type == DataType::Int {
        output.push(Instr::NegInt(id_l, id));
    } else {
        throw_compiler_error(ctx.src, span, ErrType::InvalidOp(&operand_type, "-"));
    }
    id
}

fn compile_bool_neg_op(
    l: &Expr,
    span: Span,
    tgt_id: Option<u16>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) -> u16 {
    let operand_type = l.infer_type(v, ctx, state);
    let id_l = l
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    state.free_reg(id_l, v);
    let id = state.alloc_reg_tgt(tgt_id);
    if operand_type != DataType::Bool {
        throw_compiler_error(ctx.src, span, ErrType::InvalidOp(&operand_type, "!"));
    }
    output.push(Instr::NegBool(id_l, id));
    id
}

fn compile_inline_condition_branch(
    branch: &[Expr],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
    tgt_id: u16,
) {
    let regs_before = state.registers.len() as u16;
    let output_len = output.len();
    output.extend(compile_expr(
        &branch[..branch.len() - 1],
        v,
        ctx.advance_offset(output.len() as u16),
        state,
    ));
    let val_id = branch[branch.len() - 1]
        .compile(
            v,
            ctx.advance_offset(output.len() as u16),
            state,
            output,
            Some(tgt_id),
            false,
            true,
        )
        .unwrap_id();
    state.free_scope_registers(regs_before, &output[output_len..], v);
    if val_id != tgt_id {
        output.push(Instr::Mov(val_id, tgt_id));
    }
}

fn compile_inline_condition(
    main_condition: &Expr,
    code: &[Expr],
    span: Span,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
    tgt_id: Option<u16>,
) -> u16 {
    let return_id = state.alloc_reg_tgt(tgt_id);

    // get first code limit (after which there are only else(if) blocks)
    let main_code_limit = code
        .iter()
        .position(|x| matches!(x, Expr::ElseIfBlock(_, _) | Expr::ElseBlock(_)))
        .unwrap_or(code.len());

    let condition_blocks_count = code.len() - main_code_limit;
    let mut cmp_markers: Vec<usize> = Vec::with_capacity(condition_blocks_count);
    let mut jmp_markers: Vec<usize> = Vec::with_capacity(condition_blocks_count);
    let mut condition_markers: Vec<usize> = Vec::with_capacity(condition_blocks_count);

    // parse the main condition
    let condition_id = main_condition
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    add_cmp_false(condition_id, &mut 0, output, false);
    cmp_markers.push(output.len() - 1);

    compile_inline_condition_branch(&code[..main_code_limit], v, ctx, state, output, return_id);
    if main_code_limit != code.len() {
        output.push(Instr::Jmp(0));
        jmp_markers.push(output.len() - 1);
    }

    let mut else_exists = false;
    for elem in &code[main_code_limit..] {
        if let Expr::ElseIfBlock(condition, code) = elem {
            condition_markers.push(output.len());
            let condition_id = condition
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            add_cmp_false(condition_id, &mut 0, output, false);
            state.free_reg(condition_id, v);
            cmp_markers.push(output.len() - 1);
            compile_inline_condition_branch(code, v, ctx, state, output, return_id);
            output.push(Instr::Jmp(0));
            jmp_markers.push(output.len() - 1);
        } else if let Expr::ElseBlock(code) = elem {
            else_exists = true;
            condition_markers.push(output.len());
            compile_inline_condition_branch(code, v, ctx, state, output, return_id);
        }
    }
    if !else_exists {
        throw_compiler_error(ctx.src, span, ErrType::InvalidConditionalExpression);
    }

    for y in jmp_markers {
        let diff = output.len() - y;
        output[y] = Instr::Jmp(diff as u16);
    }
    for (i, y) in cmp_markers.iter().enumerate() {
        let diff = if i >= condition_markers.len() {
            output.len() - 1 - y
        } else {
            condition_markers[i] - y
        };
        if let Some(
            Instr::IsFalseJmp(_, jump_size)
            | Instr::SupEqFloatJmp(_, _, jump_size)
            | Instr::SupEqIntJmp(_, _, jump_size)
            | Instr::SupFloatJmp(_, _, jump_size)
            | Instr::SupIntJmp(_, _, jump_size)
            | Instr::InfEqFloatJmp(_, _, jump_size)
            | Instr::InfEqIntJmp(_, _, jump_size)
            | Instr::InfFloatJmp(_, _, jump_size)
            | Instr::InfIntJmp(_, _, jump_size)
            | Instr::NotEqJmp(_, _, jump_size)
            | Instr::ObjNotEqJmp(_, _, jump_size)
            | Instr::EqJmp(_, _, jump_size)
            | Instr::ObjEqJmp(_, _, jump_size),
        ) = output.get_mut(*y)
        {
            *jump_size = diff as u16;
        }
    }
    state.free_reg(condition_id, v);
    return_id
}

fn compile_array_index_assignment(
    array: &Expr,
    index: &Expr,
    value: &Expr,
    index_markers: Span,
    elem_markers: Span,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) {
    let array_type = array.infer_type(v, ctx, state);
    if !array_type.is_indexable() {
        throw_compiler_error(ctx.src, index_markers, ErrType::NotIndexable(&array_type));
    }
    // Get the id of the source array/string (may be a nested GetIndex)
    let id = array
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();

    let final_id = index
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();

    let elem_type = value.infer_type(v, ctx, state);
    let elem_id = value
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    state.free_reg(elem_id, v);
    if {
        if let DataType::Array(Some(array_type)) = &array_type
            && array_type.as_ref() != &elem_type
        {
            true
        } else {
            false
        }
    } || (array_type == DataType::String && elem_type != DataType::String)
    {
        throw_compiler_error(
            ctx.src,
            elem_markers,
            ErrType::CannotPushTypeToArray(&elem_type, &array_type),
        );
    }

    let to_push = if array_type == DataType::String {
        Instr::SetElementString(id, elem_id, final_id)
    } else {
        Instr::SetElementObj(id, elem_id, final_id)
    };
    state
        .instr_src
        .push((to_push, index_markers, ctx.current_src_file));
    output.push(to_push);
    state.free_reg(id, v);
}

fn compile_struct_field_assignment(
    struct_expr: &Expr,
    field: &SmolStr,
    new_val: &Expr,
    struct_span: Span,
    field_span: Span,
    value_span: Span,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) {
    let t = struct_expr.infer_type(v, ctx, state);
    let new_val_type = new_val.infer_type(v, ctx, state);
    let DataType::Struct(struct_id) = t else {
        throw_compiler_error(
            ctx.src,
            struct_span,
            ErrType::InvalidType(&DataType::Struct(0), &t),
        );
    };
    let mut field_index: Option<u16> = None;
    let field_struct = &state.structs[struct_id as usize];
    let struct_name = &field_struct.name;
    for (i, (expected_field_name, expected_field_type, expected_field_span)) in
        field_struct.fields.iter().enumerate()
    {
        if expected_field_name == field {
            if !struct_field_type_matches(expected_field_type, &new_val_type) {
                error_struct_field_invalid_type(
                    ctx.src,
                    struct_name,
                    *expected_field_span,
                    expected_field_name,
                    expected_field_type,
                    value_span,
                    &new_val_type,
                    state.sources,
                );
            }
            field_index = Some(i as u16);
            break;
        }
    }
    let Some(field_index) = field_index else {
        error_struct_unknown_field(
            ctx.src,
            field_span,
            field,
            struct_name,
            &field_struct.fields,
            state.sources,
        );
    };
    let id = struct_expr
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    let new_elem_reg_id = new_val
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();
    output.push(Instr::SetFieldStruct(id, new_elem_reg_id, field_index));
}

fn compile_condition(
    main_condition: &Expr,
    code: &[Expr],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) {
    // get first code limit (after which there are only else(if) blocks)
    let main_code_limit = code
        .iter()
        .position(|x| matches!(x, Expr::ElseIfBlock(_, _) | Expr::ElseBlock(_)))
        .unwrap_or(code.len());

    let condition_blocks_count = code.len() - main_code_limit;
    // Each entry is the list of false-jump instruction indices for one condition block.
    let mut conditional_false_jmp_idxs: Vec<Vec<usize>> =
        Vec::with_capacity(condition_blocks_count + 1);
    let mut jmp_instr_idx: Vec<usize> = Vec::with_capacity(condition_blocks_count);
    let mut condition_markers: Vec<usize> = Vec::with_capacity(condition_blocks_count);

    // Compile the main condition
    let (true_jump_idxs, false_jump_idxs) =
        compile_short_circuit_condition(main_condition, v, ctx, state, output, false);
    conditional_false_jmp_idxs.push(false_jump_idxs);

    // Modify true jump instructions to point to body_start
    let body_start = output.len();
    for j in true_jump_idxs {
        set_jmp_size(&mut output[j], (body_start - j) as u16);
    }

    // parse the main code block
    let cond_code = compile_expr(
        &code[0..main_code_limit],
        v,
        ctx.advance_offset(output.len() as u16),
        state,
    );
    output.extend(cond_code);
    if main_code_limit != code.len() {
        output.push(Instr::Jmp(0));
        jmp_instr_idx.push(output.len() - 1);
    }

    for elem in &code[main_code_limit..] {
        if let Expr::ElseIfBlock(condition, code) = elem {
            condition_markers.push(output.len());
            let condition_id = condition
                .compile(v, ctx, state, output, None, false, true)
                .unwrap_id();
            state.free_reg(condition_id, v);
            add_cmp_false(condition_id, &mut 0, output, false);
            conditional_false_jmp_idxs.push(vec![output.len() - 1]);
            let cond_code = compile_expr(code, v, ctx.advance_offset(output.len() as u16), state);
            output.extend(cond_code);
            output.push(Instr::Jmp(0));
            jmp_instr_idx.push(output.len() - 1);
        } else if let Expr::ElseBlock(code) = elem {
            condition_markers.push(output.len());
            let cond_code = compile_expr(code, v, ctx.advance_offset(output.len() as u16), state);
            output.extend(cond_code);
        }
    }

    for y in jmp_instr_idx {
        let diff = output.len() - y;
        output[y] = Instr::Jmp(diff as u16);
    }
    // Fix all false-jump instructions for each condition block
    for (cm_idx, false_idxs) in conditional_false_jmp_idxs.iter().enumerate() {
        let target = if cm_idx < condition_markers.len() {
            condition_markers[cm_idx]
        } else {
            output.len()
        };
        for &y in false_idxs {
            set_jmp_size(&mut output[y], (target - y) as u16);
        }
    }
}

fn compile_while_loop(
    condition: &Expr,
    code: &[Expr],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) {
    let output_len_before = output.len();

    let (true_jump_idxs, false_jump_idxs) =
        compile_short_circuit_condition(condition, v, ctx, state, output, false);

    let body_start = output.len();
    for j in true_jump_idxs {
        set_jmp_size(&mut output[j], (body_start - j) as u16);
    }

    // parse the code block, clone the vars to avoid overriding anything
    let loop_id = ctx.block_id + 1;

    let mut cond_code = compile_expr(
        code,
        v,
        ctx.no_single_run().advance_offset(output.len() as u16),
        state,
    );

    let exit = output.len() + cond_code.len() + 1;
    for j in false_jump_idxs {
        set_jmp_size(&mut output[j], (exit - j) as u16);
    }

    let cond_len = (output.len() - output_len_before) as u16;
    let body_len = cond_code.len() as u16;
    let len = cond_len + body_len; // full span used by JmpBack
    // Break/Continue offsets are relative to cond_code, so pass body_len+1 (body remaining + JmpBack)
    parse_loop_flow_control(&mut cond_code, loop_id, body_len + 1, false, false);
    output.extend(cond_code);
    output.push(Instr::JmpBack(len));
}

fn compile_for_loop(
    var_name: &SmolStr,
    array: &Expr,
    code: &[Expr],
    span: Span,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) {
    let real_var = var_name.as_str() != "_";

    // parse the array, get its id (the target array is the first Expr in array_code)
    let array_type = array.infer_type(v, ctx, state);
    let array = array
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();

    let array_len_id = state.alloc_reg();

    output.push(Instr::CallLibFunc(LibFunc::Len, array, array_len_id));

    // set up the id of the index variable (0..len)
    let index_id = if ctx.single_run {
        state.registers.push(0.into());
        (state.registers.len() - 1) as u16
    } else {
        let id = state.alloc_reg();
        output.push(Instr::SetInt(id, 0));
        id
    };

    // do the 'i < len' condition, set up the condition's id (true/false)
    let condition_id = state.alloc_reg();

    output.push(Instr::InfInt(index_id, array_len_id, condition_id));

    // set up the variable for the current element (for current_element_id in ... {}) => current_element_id = array[index]
    let current_element_id = if real_var { state.alloc_reg() } else { 0 };

    let v_len = v.len();

    let is_str = array_type == DataType::String;

    if real_var {
        v.push(Variable {
            name: var_name.clone(),
            register_id: current_element_id,
            var_type: match array_type {
                DataType::String => DataType::String,
                DataType::Array(a_type) => a_type.map_or(DataType::Null, |t| *t),
                t => throw_compiler_error(ctx.src, span, ErrType::IsNotAnIterator(&t)),
            },
        });
    }
    let loop_id = ctx.block_id + 1;

    // accounts for the GetIndexArray/GetIndexString instruction
    let pending = real_var as u16;

    let regs_before = state.registers.len() as u16;
    let mut cond_code = compile_expr(
        code,
        v,
        ctx.no_single_run()
            .advance_offset(output.len() as u16 + pending),
        state,
    );
    // Clean up variables
    v.truncate(v_len);
    state.free_loop_scope_registers(regs_before, &cond_code, v);

    // add the condition ('i < len') jumping logic
    let mut len = (cond_code.len() + 3) as u16 + pending;
    add_cmp_false(condition_id, &mut len, output, true);

    // make the current_element_id register actually hold the element's value
    if real_var {
        if is_str {
            output.push(Instr::GetIndexString(array, index_id, current_element_id));
        } else {
            output.push(Instr::GetIndexArray(array, index_id, current_element_id));
        }
    }
    parse_loop_flow_control(&mut cond_code, loop_id, len, true, false);
    // then add the condition code
    output.extend(cond_code);
    // add 1 to the index (i+=1) so that the next loop iteration will have the next element in the array
    output.push(Instr::IncInt(index_id));

    // jump back to the loop if still inside of it
    output.push(Instr::JmpBack(len));

    if ctx.single_run {
        state.free_reg(array_len_id, v);
        state.free_reg(index_id, v);
        state.free_reg(condition_id, v);
        if real_var {
            state.free_reg(current_element_id, v);
        }
    }
}

fn compile_int_for_loop(
    var_name: &SmolStr,
    start_elem: &Expr,
    end_elem: &Expr,
    code: &[Expr],
    span1: Span,
    span2: Span,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) {
    // IntForLoop is compiled to:
    // ----
    // (1) if i >= end_elem jump out
    // (2) loop_body
    // (3) i += 1
    // (4) if i < end_elem jump back to body
    // ----
    //
    //
    // Check start and elem type
    let t1 = start_elem.infer_type(v, ctx, state);
    let t2 = end_elem.infer_type(v, ctx, state);
    t1.expect(&DataType::Int, ctx.src, span1);
    t2.expect(&DataType::Int, ctx.src, span2);
    let elem_id = if ctx.single_run {
        start_elem
            .compile(v, ctx, state, output, None, false, true)
            .unwrap_id()
    } else {
        let start_elem_id = start_elem
            .compile(v, ctx, state, output, None, false, true)
            .unwrap_id();
        let start_val = state.registers[start_elem_id as usize];
        let elem_id = state.alloc_reg();
        if state.const_registers.values().any(|&v| v == start_elem_id) && start_val.is_int() {
            output.push(Instr::SetInt(elem_id, start_val.as_int()));
        } else {
            output.push(Instr::Mov(start_elem_id, elem_id));
        }
        elem_id
    };
    let end_elem_id = end_elem
        .compile(v, ctx, state, output, None, false, true)
        .unwrap_id();

    // elem_id is a fresh mutable register -> remove from const_registers just in case
    state.const_registers.retain(|_, &mut v| v != elem_id);

    let v_len = v.len();
    v.push(Variable {
        name: var_name.clone(),
        register_id: elem_id,
        var_type: DataType::Int,
    });
    let loop_id = ctx.block_id + 1;

    // (1) if i >= end_elem jump out -> push placeholder first so that compile_expr sees the correct offset
    let jmp_idx = output.len();
    output.push(Instr::SupEqIntJmp(elem_id, end_elem_id, 0));

    let regs_before = state.registers.len() as u16;
    let compiled_loop_code = compile_expr(
        code,
        v,
        ctx.no_single_run().advance_offset(output.len() as u16),
        state,
    );
    state.free_loop_scope_registers(regs_before, &compiled_loop_code, v);
    let compiled_loop_code_len = compiled_loop_code.len() as u16;

    // (2) loop_body
    output.extend(compiled_loop_code);

    // (3) i+= 1
    output.push(Instr::IncInt(elem_id));

    // (4) if i < end_elem jump back to body
    output.push(Instr::InfIntJmpBack(
        elem_id,
        end_elem_id,
        compiled_loop_code_len + 1,
    ));

    let exit_size = (output.len() - jmp_idx) as u16;
    output[jmp_idx] = Instr::SupEqIntJmp(elem_id, end_elem_id, exit_size);

    parse_loop_flow_control(&mut output[jmp_idx + 1..], loop_id, exit_size, true, false);
    v.truncate(v_len);

    if ctx.single_run {
        state.free_reg(end_elem_id, v);
        state.free_reg(elem_id, v);
    }
}

fn compile_loop_block(
    code: &[Expr],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) {
    let loop_id = ctx.block_id + 1;
    let regs_before = state.registers.len() as u16;
    let mut compiled = compile_expr(
        code,
        v,
        ctx.no_single_run().advance_offset(output.len() as u16),
        state,
    );
    state.free_loop_scope_registers(regs_before, &compiled, v);
    let code_length = compiled.len() as u16;
    parse_loop_flow_control(&mut compiled, loop_id, code_length + 1, false, true);
    output.extend(compiled);
    output.push(Instr::JmpBack(code_length));
}

fn compile_try_catch_block(
    e: &[Expr],
    err_var: &SmolStr,
    catch_code: &[Expr],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) {
    output.push(Instr::StartErrorCatch(0, 0)); // patched later on
    let err_catch_instr = output.len() - 1;
    let main_code = compile_expr(e, v, ctx, state);
    output.extend(main_code);
    output.push(Instr::StopErrorCatch);
    output.push(Instr::Jmp(0)); // jumps over the catch handler if no error arises
    let jmp_catch_instr = output.len() - 1;

    let v_len = v.len();
    let err_reg_id = state.alloc_reg();
    v.push(Variable {
        name: err_var.clone(),
        register_id: err_reg_id,
        var_type: DataType::String,
    });
    output[err_catch_instr] =
        Instr::StartErrorCatch((output.len() - err_catch_instr) as u16, err_reg_id);
    let catch_code = compile_expr(catch_code, v, ctx, state);
    v.truncate(v_len);
    output.extend(catch_code);
    output[jmp_catch_instr] = Instr::Jmp((output.len() - jmp_catch_instr) as u16);
    state.free_reg(err_reg_id, v);
}

fn compile_var_declaration(
    name: &SmolStr,
    value: &Expr,
    remaining_code: &[Expr],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) {
    let var_type = value.infer_type(v, ctx, state);

    let var_id = if ctx.single_run {
        value
            .compile(v, ctx, state, output, None, true, true)
            .unwrap_id()
    } else {
        let src_id = value
            .compile(v, ctx, state, output, None, false, true)
            .unwrap_id();
        if contains_var_reassign(name, remaining_code) {
            let mutable_id = state.alloc_reg();
            move_reg_to_reg(output, src_id, mutable_id, state.registers[src_id as usize]);
            mutable_id
        } else {
            src_id
        }
    };

    if let DataType::Fn(fn_id) = &var_type {
        state.namespace.fns.push((name.clone(), *fn_id));
    }
    v.push(Variable {
        name: name.clone(),
        register_id: var_id,
        var_type,
    });
}

fn compile_var_assignment(
    name: &SmolStr,
    value: &Expr,
    span: Span,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) {
    let var_type = value.infer_type(v, ctx, state);
    let var_pos = v.iter().rposition(|x| x.name == *name).unwrap_or_else(|| {
        throw_compiler_error(ctx.src, span, ErrType::UnknownVariable(name));
    });
    let id = v[var_pos].register_id;

    if var_type == DataType::Int {
        // (is_inc, src_var_name)
        let inc_dec: Option<(bool, &str)> = match value {
            // var+1/1+var use the dedicated IncInt/IncIntTo instructions
            Expr::Add(l, r, _) => {
                let src = if matches!(r.as_ref(), Expr::Int(1)) {
                    Some(l.as_ref())
                } else if matches!(l.as_ref(), Expr::Int(1)) {
                    Some(r.as_ref())
                } else {
                    None
                };
                src.and_then(|e| {
                    if let Expr::Var(src_name, _) = e {
                        v.iter()
                            .rfind(|x| x.name == *src_name)
                            .filter(|x| x.var_type == DataType::Int)
                            .map(|_| (true, src_name.as_str()))
                    } else {
                        None
                    }
                })
            }
            // var-1 uses the dedicated DecInt/DecIntTo instructions
            Expr::Sub(l, r, _) => {
                if matches!(r.as_ref(), Expr::Int(1)) {
                    if let Expr::Var(src_name, _) = l.as_ref() {
                        v.iter()
                            .rfind(|x| x.name == *src_name)
                            .filter(|x| x.var_type == DataType::Int)
                            .map(|_| (false, src_name.as_str()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some((is_inc, src_name)) = inc_dec {
            let src_id = v.iter().rfind(|x| x.name == src_name).unwrap().register_id;
            output.push(if src_id == id {
                if is_inc {
                    Instr::IncInt(id)
                } else {
                    Instr::DecInt(id)
                }
            } else {
                if is_inc {
                    Instr::IncIntTo(src_id, id)
                } else {
                    Instr::DecIntTo(src_id, id)
                }
            });
            return;
        }
    }

    let output_len = output.len();
    let obj_id = value
        .compile(v, ctx, state, output, Some(id), false, true)
        .unwrap_id();
    if output.len() != output_len {
        move_to_id(output, id);
    } else if state.const_registers.values().any(|&v| v == obj_id) {
        move_reg_to_reg(output, obj_id, id, state.registers[obj_id as usize]);
    } else {
        output.push(Instr::Mov(obj_id, id));
    }
    if !v
        .iter()
        .any(|var| &var.name != name && var.register_id == obj_id)
    {
        state.free_reg(obj_id, v);
    }
    v[var_pos].var_type = var_type;
}

fn compile_struct_definition(
    name: &SmolStr,
    fields: &[(SmolStr, TypeExpr, Span)],
    span: Span,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    _output: &mut Vec<Instr>,
) {
    let struct_id = state.structs.len() as u16;
    state.structs.push(Struct {
        // pushing it first allows structs to be recursive
        name: name.clone(),
        fields: Box::from([]),
        id: struct_id,
        name_span: span,
    });
    let parsed_fields = fields
        .iter()
        .map(|(f, f_t, f_span)| {
            (
                f.clone(),
                parse_keel_type(f_t, state.structs, span, ctx.src),
                *f_span,
            )
        })
        .collect();
    state.structs[struct_id as usize].fields = parsed_fields;
    state.struct_fields.push((
        name.clone(),
        fields
            .iter()
            .map(|(n, _, _)| n.clone())
            .collect::<Vec<SmolStr>>(),
    ));
    state
        .namespace
        .structs
        .push((name.clone(), (state.structs.len() - 1) as u16));
}

fn compile_function_definition(
    fn_name: &SmolStr,
    fn_args: &[SmolStr],
    fn_code: &Rc<[Expr]>,
    span: Span,
    _v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    _output: &mut Vec<Instr>,
) {
    if state.fns.iter().any(|func| &func.name == fn_name) {
        throw_compiler_error(ctx.src, span, ErrType::FunctionAlreadyExists(fn_name));
    }
    let mut callees = Vec::new();
    collect_direct_fn_calls(fn_code, &mut callees);
    state.fns.push(Function {
        name: fn_name.clone(),
        args: Box::from(fn_args),
        code: fn_code.clone(),
        impls: Vec::new(),
        is_recursive: None,
        returns_null: check_if_returns_void(fn_code),
        src_file: ctx.current_src_file,
        return_type_cache: Vec::new(),
        direct_calls: callees.into_boxed_slice(),
        name_span: span,
    });
    state.fn_registers.push(Vec::new());
    state
        .namespace
        .fns
        .push((fn_name.clone(), (state.fns.len() - 1) as u16));
}

fn compile_return(
    return_value: Option<&Expr>,
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) {
    if let Some(x) = return_value {
        let id = x
            .compile(v, ctx, state, output, None, false, true)
            .unwrap_id();
        if ctx.is_parsing_recursive {
            output.push(Instr::RecursiveReturn(id));
        } else {
            output.push(Instr::Return(id));
        }
    }
}

#[inline]
fn compile_loop_break(ctx: Ctx<'_>, output: &mut Vec<Instr>) {
    output.push(Instr::NotEqJmp(ctx.block_id + 1, 0, 0));
}

#[inline]
fn compile_loop_continue(ctx: Ctx<'_>, output: &mut Vec<Instr>) {
    output.push(Instr::EqJmp(ctx.block_id + 1, 0, 0));
}

#[inline]
fn compile_eval_block(
    code: &[Expr],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
    output: &mut Vec<Instr>,
) {
    output.extend(compile_expr(
        code,
        v,
        ctx.set_offset(output.len() as u16),
        state,
    ));
}

pub fn compile_expr(
    input: &[Expr],
    v: &mut Vec<Variable>,
    ctx: Ctx<'_>,
    state: &mut State<'_>,
) -> Vec<Instr> {
    let v_len = v.len();
    let fn_len = state.fns.len();
    let mut output: Vec<Instr> = Vec::with_capacity(input.len());
    for (idx, x) in input.iter().enumerate() {
        if let Some(id) = x.compile_with_code_context(
            v,
            ctx,
            state,
            &mut output,
            None,
            false,
            &input[idx + 1..],
            false,
        ) {
            state.free_reg(id, v);
        }
    }
    v.truncate(v_len);
    state.fns.truncate(fn_len);
    output
}

impl Expr {
    pub const fn is_constant_literal(&self) -> bool {
        matches!(
            self,
            Self::Int(_) | Self::Float(_) | Self::String(_) | Self::Bool(_) | Self::Null
        )
    }
    #[inline(always)]
    pub fn compile(
        &self,
        v: &mut Vec<Variable>,
        ctx: Ctx<'_>,
        state: &mut State<'_>,
        output: &mut Vec<Instr>,
        tgt_id: Option<u16>,
        var_assignment: bool,
        uses_id: bool,
    ) -> Option<u16> {
        self.compile_with_code_context(v, ctx, state, output, tgt_id, var_assignment, &[], uses_id)
    }
    pub fn compile_with_code_context(
        &self,
        v: &mut Vec<Variable>,
        ctx: Ctx<'_>,
        state: &mut State<'_>,
        output: &mut Vec<Instr>,
        tgt_id: Option<u16>,
        var_assignment: bool,
        remaining_code: &[Self],
        uses_id: bool,
    ) -> Option<u16> {
        match self {
            Self::Int(num) => {
                debug_assert!(uses_id);
                if var_assignment {
                    state.registers.push((*num).into());
                    return Some((state.registers.len() - 1) as u16);
                }
                let data = (*num).into();
                if let Some(&id) = state.const_registers.get(&data) {
                    Some(id)
                } else {
                    let id = state.registers.len() as u16;
                    state.const_registers.insert(data, id);
                    state.registers.push(data);
                    Some(id)
                }
            }
            Self::Float(num) => {
                debug_assert!(uses_id);
                if var_assignment {
                    state.registers.push((*num).into());
                    return Some((state.registers.len() - 1) as u16);
                }
                let data = (*num).into();
                if let Some(&id) = state.const_registers.get(&data) {
                    Some(id)
                } else {
                    state.registers.push(data);
                    let id = (state.registers.len() - 1) as u16;
                    state.const_registers.insert(data, id);
                    Some(id)
                }
            }
            Self::String(str) => {
                debug_assert!(uses_id);
                if var_assignment {
                    state
                        .registers
                        .push(Data::p_str(str, &mut state.pools.strings));
                    return Some((state.registers.len() - 1) as u16);
                }
                let data = Data::p_str(str, &mut state.pools.strings);
                if let Some(&id) = state.const_registers.get(&data) {
                    Some(id)
                } else {
                    let id = state.registers.len() as u16;
                    state.const_registers.insert(data, id);
                    state.registers.push(data);
                    Some(id)
                }
            }
            Self::Null => {
                debug_assert!(uses_id);
                if var_assignment {
                    state.registers.push(NULL);
                    return Some((state.registers.len() - 1) as u16);
                }
                if let Some(&id) = state.const_registers.get(&NULL) {
                    Some(id)
                } else {
                    let id = state.registers.len() as u16;
                    state.const_registers.insert(NULL, id);
                    state.registers.push(NULL);
                    Some(id)
                }
            }
            Self::Bool(bool) => {
                debug_assert!(uses_id);
                if var_assignment {
                    state.registers.push((*bool).into());
                    return Some((state.registers.len() - 1) as u16);
                }
                let data: Data = (*bool).into();
                if let Some(&id) = state.const_registers.get(&data) {
                    Some(id)
                } else {
                    let id = state.registers.len() as u16;
                    state.const_registers.insert(data, id);
                    state.registers.push(data);
                    Some(id)
                }
            }
            Self::Var(name, markers) => {
                debug_assert!(uses_id);
                if let Some(Variable {
                    name: _,
                    register_id,
                    var_type: _,
                }) = v.iter().rfind(|v_temp| *name == v_temp.name)
                {
                    Some(*register_id)
                } else {
                    cold_path();
                    throw_compiler_error(ctx.src, *markers, ErrType::UnknownVariable(name))
                }
            }
            Self::Array(array_items, spans) => {
                debug_assert!(uses_id);
                Some(compile_array_literal(
                    array_items,
                    spans,
                    v,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::Struct(namespace, fields, span) => {
                debug_assert!(uses_id);
                Some(compile_struct_literal(
                    namespace, fields, *span, v, ctx, state, output,
                ))
            }
            Self::Map(kv_pairs, span) => {
                debug_assert!(uses_id);
                Some(compile_map_literal(kv_pairs, *span, v, ctx, state, output))
            }
            Self::GetStructField(struct_expr, field, struct_span, field_span) => {
                debug_assert!(uses_id);
                Some(compile_struct_field_access(
                    struct_expr,
                    field,
                    *struct_span,
                    *field_span,
                    v,
                    ctx,
                    state,
                    output,
                ))
            }
            // array[index]
            Self::ArrayGetIndex(array, index, span) => {
                debug_assert!(uses_id);
                Some(compile_array_indexing(
                    array, index, *span, v, ctx, state, output,
                ))
            }
            // array[start..end]
            Self::ArrayGetSlice(array, idx_start, idx_end, span) => {
                debug_assert!(uses_id);
                Some(compile_array_slice(
                    array, idx_start, idx_end, *span, v, ctx, state, output,
                ))
            }
            Self::Mul(l, r, span) => {
                debug_assert!(uses_id);
                Some(uniform_op2(
                    Instr::MulFloat,
                    &DataType::Float,
                    Instr::MulInt,
                    &DataType::Int,
                    "*",
                    l,
                    r,
                    *span,
                    tgt_id,
                    v,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::Div(l, r, span) => {
                debug_assert!(uses_id);
                Some(compile_div_op(l, r, *span, tgt_id, v, ctx, state, output))
            }
            Self::Add(l, r, span) => {
                debug_assert!(uses_id);
                Some(compile_add_op(l, r, *span, tgt_id, v, ctx, state, output))
            }
            Self::Sub(l, r, span) => {
                debug_assert!(uses_id);
                Some(compile_sub_op(l, r, *span, tgt_id, v, ctx, state, output))
            }
            Self::Mod(l, r, span) => {
                debug_assert!(uses_id);
                Some(compile_mod_op(l, r, *span, tgt_id, v, ctx, state, output))
            }
            Self::Pow(l, r, span) => {
                debug_assert!(uses_id);
                Some(uniform_op2(
                    Instr::PowFloat,
                    &DataType::Float,
                    Instr::PowInt,
                    &DataType::Int,
                    "^",
                    l,
                    r,
                    *span,
                    tgt_id,
                    v,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::Eq(l, r) => {
                debug_assert!(uses_id);
                Some(compile_eq_op(l, r, tgt_id, v, ctx, state, output))
            }
            Self::NotEq(l, r) => {
                debug_assert!(uses_id);
                Some(compile_neq_op(l, r, tgt_id, v, ctx, state, output))
            }
            Self::Sup(l, r, span) => {
                debug_assert!(uses_id);
                Some(uniform_op2(
                    Instr::SupFloat,
                    &DataType::Float,
                    Instr::SupInt,
                    &DataType::Int,
                    ">",
                    l,
                    r,
                    *span,
                    tgt_id,
                    v,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::SupEq(l, r, span) => {
                debug_assert!(uses_id);
                Some(uniform_op2(
                    Instr::SupEqFloat,
                    &DataType::Float,
                    Instr::SupEqInt,
                    &DataType::Int,
                    ">=",
                    l,
                    r,
                    *span,
                    tgt_id,
                    v,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::Inf(l, r, span) => {
                debug_assert!(uses_id);
                Some(uniform_op2(
                    Instr::InfFloat,
                    &DataType::Float,
                    Instr::InfInt,
                    &DataType::Int,
                    "<",
                    l,
                    r,
                    *span,
                    tgt_id,
                    v,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::InfEq(l, r, span) => {
                debug_assert!(uses_id);
                Some(uniform_op2(
                    Instr::InfEqFloat,
                    &DataType::Float,
                    Instr::InfEqInt,
                    &DataType::Int,
                    "<=",
                    l,
                    r,
                    *span,
                    tgt_id,
                    v,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::BoolAnd(l, r, markers) => {
                debug_assert!(uses_id);
                Some(uniform_op(
                    Instr::BoolAnd,
                    "&&",
                    l,
                    r,
                    *markers,
                    &DataType::Bool,
                    tgt_id,
                    v,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::BoolOr(l, r, span) => {
                debug_assert!(uses_id);
                Some(uniform_op(
                    Instr::BoolOr,
                    "||",
                    l,
                    r,
                    *span,
                    &DataType::Bool,
                    tgt_id,
                    v,
                    ctx,
                    state,
                    output,
                ))
            }
            Self::Neg(l, span) => {
                debug_assert!(uses_id);
                Some(compile_neg_op(l, *span, tgt_id, v, ctx, state, output))
            }
            Self::BoolNeg(l, span) => {
                debug_assert!(uses_id);
                Some(compile_bool_neg_op(l, *span, tgt_id, v, ctx, state, output))
            }
            Self::InlineCondition(main_condition, code, span) => {
                debug_assert!(uses_id);
                Some(compile_inline_condition(
                    main_condition,
                    code,
                    *span,
                    v,
                    ctx,
                    state,
                    output,
                    tgt_id,
                ))
            }
            Self::FunctionCall(args, namespace, markers, args_indexes) if uses_id => Some(
                handle_functions(
                    output,
                    v,
                    ctx,
                    state,
                    tgt_id,
                    args,
                    namespace,
                    *markers,
                    args_indexes,
                )
                .unwrap_or_else(|| {
                    if let Some(&id) = state.const_registers.get(&NULL) {
                        id
                    } else {
                        let id = state.registers.len() as u16;
                        state.const_registers.insert(NULL, id);
                        state.registers.push(NULL);
                        id
                    }
                }),
            ),
            Self::AnonymousFunction(_, _, _) => {
                debug_assert!(uses_id);
                if let Some(&id) = state.const_registers.get(&NULL) {
                    Some(id)
                } else {
                    let id = state.registers.len() as u16;
                    state.const_registers.insert(NULL, id);
                    state.registers.push(NULL);
                    Some(id)
                }
            }

            // ------------------
            // --- STATEMENTS ---
            // ------------------

            // x[y] = z;
            Self::ArrayModify(array, index, value, index_markers, elem_markers) => {
                debug_assert!(!uses_id);
                compile_array_index_assignment(
                    array,
                    index,
                    value,
                    *index_markers,
                    *elem_markers,
                    v,
                    ctx,
                    state,
                    output,
                );
                None
            }
            Self::SetStructField(
                struct_expr,
                field,
                new_val,
                struct_span,
                field_span,
                value_span,
            ) => {
                debug_assert!(!uses_id);
                compile_struct_field_assignment(
                    struct_expr,
                    field,
                    new_val,
                    *struct_span,
                    *field_span,
                    *value_span,
                    v,
                    ctx,
                    state,
                    output,
                );
                None
            }
            Self::Condition(main_condition, code, _) => {
                debug_assert!(!uses_id);
                compile_condition(main_condition, code, v, ctx, state, output);
                None
            }
            Self::WhileBlock(condition, code) => {
                debug_assert!(!uses_id);
                compile_while_loop(condition, code, v, ctx, state, output);
                None
            }
            Self::ForLoop(var_name, array, code, span) => {
                debug_assert!(!uses_id);
                compile_for_loop(var_name, array, code, *span, v, ctx, state, output);
                None
            }
            Self::IntForLoop(var_name, start_elem, end_elem, code, span1, span2) => {
                debug_assert!(!uses_id);
                compile_int_for_loop(
                    var_name, start_elem, end_elem, code, *span1, *span2, v, ctx, state, output,
                );
                None
            }
            Self::LoopBlock(code) => {
                debug_assert!(!uses_id);
                compile_loop_block(code, v, ctx, state, output);
                None
            }
            Self::TryCatchBlock(e, err_var, catch_code) => {
                debug_assert!(!uses_id);
                compile_try_catch_block(e, err_var, catch_code, v, ctx, state, output);
                None
            }
            Self::VarDeclare(name, value) => {
                debug_assert!(!uses_id);
                compile_var_declaration(name, value, remaining_code, v, ctx, state, output);
                None
            }
            Self::VarAssign(name, value, span) => {
                debug_assert!(!uses_id);
                compile_var_assignment(name, value, *span, v, ctx, state, output);
                None
            }
            Self::StructDeclare(name, fields, span) => {
                debug_assert!(!uses_id);
                compile_struct_definition(name, fields, *span, ctx, state, output);
                None
            }
            Self::FunctionCall(args, namespace, markers, args_indexes) if !uses_id => {
                let output_id = handle_functions(
                    output,
                    v,
                    ctx,
                    state,
                    tgt_id,
                    args,
                    namespace,
                    *markers,
                    args_indexes,
                );
                if let Some(id) = output_id {
                    state.free_reg(id, v);
                }
                None
            }
            Self::ObjFunctionCall(obj, args, namespace, obj_markers, fn_markers, args_indexes)
                if !uses_id =>
            {
                let output_id = handle_method_calls(
                    output,
                    v,
                    ctx,
                    state,
                    tgt_id,
                    obj,
                    args,
                    namespace,
                    *obj_markers,
                    *fn_markers,
                    args_indexes,
                );
                if let Some(id) = output_id {
                    state.free_reg(id, v);
                }
                None
            }
            Self::ObjFunctionCall(obj, args, namespace, obj_markers, fn_markers, args_indexes)
                if uses_id =>
            {
                Some(
                    handle_method_calls(
                        output,
                        v,
                        ctx,
                        state,
                        tgt_id,
                        obj,
                        args,
                        namespace,
                        *obj_markers,
                        *fn_markers,
                        args_indexes,
                    )
                    .unwrap_or_else(|| {
                        if let Some(&id) = state.const_registers.get(&NULL) {
                            id
                        } else {
                            let id = state.registers.len() as u16;
                            state.const_registers.insert(NULL, id);
                            state.registers.push(NULL);
                            id
                        }
                    }),
                )
            }
            Self::FunctionDecl(fn_name, fn_args, fn_code, span) => {
                debug_assert!(!uses_id);
                compile_function_definition(
                    fn_name, fn_args, fn_code, *span, v, ctx, state, output,
                );
                None
            }
            Self::ReturnVal(return_value) => {
                debug_assert!(!uses_id);
                compile_return(return_value.as_ref().as_ref(), v, ctx, state, output);
                None
            }
            Self::Break => {
                debug_assert!(!uses_id);
                compile_loop_break(ctx, output);
                None
            }
            Self::Continue => {
                debug_assert!(!uses_id);
                compile_loop_continue(ctx, output);
                None
            }
            Self::EvalBlock(code) => {
                debug_assert!(!uses_id);
                compile_eval_block(code, v, ctx, state, output);
                None
            }
            _ => unsafe { unreachable_unchecked() },
        }
    }
}

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

#[derive(Debug)]
pub struct Namespace {
    pub fns: Vec<(SmolStr, u16)>,
    pub structs: Vec<(SmolStr, u16)>,
    pub name: SmolStr,
    pub children: Vec<Self>,
}

/// Recursively collects functions, dyn libs, and imported files
fn parse_toplevel(
    code: Vec<Expr>,
    file_path: &Path,
    src_file_idx: u16,
    use_line_markers: (&str, &str),
    fns: &mut Vec<Function>,
    structs: &mut Vec<Struct>,
    struct_fields: &mut Vec<(SmolStr, Vec<SmolStr>)>,
    fn_registers: &mut Vec<Vec<u16>>,
    dyn_libs: &mut Vec<Dynamiclib>,
    dyn_lib_fns: &mut Vec<DynamicLibFn>,
    sources: &mut Vec<(SmolStr, Rc<String>)>,
    visited_files: &mut Vec<PathBuf>,
    dyn_fn_id: &mut u16,
    namespace: &mut Namespace,
) {
    for expr in code {
        match expr {
            Expr::FunctionDecl(fn_name, fn_args, fn_code, markers) => {
                if let Some((_, func_id)) = namespace.fns.iter().find(|(f, _)| f == &fn_name) {
                    let func = &fns[*func_id as usize];
                    let func_file = &sources[func.src_file as usize].0;
                    throw_compiler_error(
                        use_line_markers,
                        markers,
                        ErrType::DuplicateFunctionInImport(&fn_name, func_file.as_str()),
                    );
                }
                fn_registers.push(Vec::new());
                let returns_void = check_if_returns_void(&fn_code);
                let mut callees = Vec::new();
                collect_direct_fn_calls(&fn_code, &mut callees);

                fns.push(Function {
                    name: fn_name.clone(),
                    args: fn_args,
                    code: fn_code,
                    impls: Vec::new(),
                    is_recursive: None,
                    returns_null: returns_void,
                    src_file: src_file_idx,
                    return_type_cache: Vec::new(),
                    direct_calls: callees.into_boxed_slice(),
                    name_span: markers,
                });
                namespace.fns.push((fn_name, (fns.len() - 1) as u16));
            }
            Expr::StructDeclare(name, fields, span) => {
                let struct_id = structs.len() as u16;
                structs.push(Struct {
                    name: name.clone(),
                    fields: Box::from([]),
                    id: struct_id,
                    name_span: span,
                });
                let parsed_fields = fields
                    .iter()
                    .map(|(f, f_t, f_span)| {
                        (
                            f.clone(),
                            parse_keel_type(f_t, structs, span, use_line_markers),
                            *f_span,
                        )
                    })
                    .collect();
                structs[struct_id as usize].fields = parsed_fields;
                struct_fields.push((
                    name.clone(),
                    fields
                        .iter()
                        .map(|(n, _, _)| n.clone())
                        .collect::<Vec<SmolStr>>(),
                ));
                namespace.structs.push((name, struct_id));
            }
            #[cfg(target_arch = "wasm32")]
            Expr::ImportDylib(_, _, _) => {
                wasm_error("WASM does not support loading dynamic libraries")
            }
            #[cfg(not(target_arch = "wasm32"))]
            Expr::ImportDylib(path, fn_signatures, markers) => {
                let base_path = if Path::new(path.as_str()).is_relative() {
                    file_path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(path.as_str())
                        .to_string_lossy()
                        .to_smolstr()
                } else {
                    path.clone()
                };
                let dylib_name = std::path::PathBuf::from(base_path.as_str())
                    .file_prefix()
                    .and_then(|s| s.to_str())
                    .unwrap_or(base_path.as_str())
                    .to_smolstr();
                // If the extension is omitted, the extension is chosen based on the target OS.
                // An architecture-specific suffix is also tried before the extension
                let path = if Path::new(base_path.as_str()).extension().is_none() {
                    let arch_path = format!("{base_path}{ARCH_SUFFIX}.{DYLIB_EXT}");
                    if Path::new(&arch_path).exists() {
                        SmolStr::from(arch_path)
                    } else {
                        format_args!("{base_path}.{DYLIB_EXT}").to_smolstr()
                    }
                } else {
                    base_path
                };
                let lib = Rc::new(unsafe {
                    libloading::Library::new(path.as_str()).unwrap_or_else(|e| {
                        throw_compiler_error(
                            use_line_markers,
                            markers,
                            ErrType::Custom(
                                format_args!("Cannot load dynamic library \"{path}\": {e}")
                                    .to_smolstr(),
                            ),
                        )
                    })
                });
                let fns = fn_signatures
                    .iter()
                    .map(|(fn_name, fn_args, fn_return_type)| {
                        let fn_args = fn_args
                            .iter()
                            .map(|t| parse_keel_type(t, structs, markers, use_line_markers))
                            .collect::<Vec<DataType>>()
                            .into_boxed_slice();
                        let fn_return_type =
                            parse_keel_type(fn_return_type, structs, markers, use_line_markers);
                        let return_val = FnSignature {
                            name: fn_name.clone(),
                            args: fn_args.clone(),
                            return_type: fn_return_type.clone(),
                            id: *dyn_fn_id,
                        };
                        let arg_types: Vec<_> = fn_args.iter().map(datatype_to_c_type).collect();
                        let return_type = datatype_to_c_type(&fn_return_type);
                        let cif = libffi::middle::Cif::new(arg_types, return_type);
                        let ptr = unsafe {
                            libffi::middle::CodePtr(
                                lib.get::<*const ()>(fn_name.as_bytes())
                                    .unwrap_or_else(|e| {
                                        throw_compiler_error(
                                            use_line_markers,
                                            markers,
                                            ErrType::Custom(
                                                format_args!(
                                                    "Cannot find symbol \"{fn_name}\" in \"{path}\": {e}"
                                                )
                                                .to_smolstr(),
                                            ),
                                        )
                                    })
                                    .try_as_raw_ptr()
                                    .unwrap_unchecked(),
                            )
                        };

                        let mut types = vec![fn_return_type];
                        types.extend(fn_args);

                        dyn_lib_fns.push(DynamicLibFn {
                            types: Box::from(types),
                            _lib: Rc::clone(&lib),
                            ptr,
                            cif,
                        });
                        *dyn_fn_id += 1;
                        return_val
                    })
                    .collect();
                dyn_libs.push(Dynamiclib {
                    name: dylib_name,
                    fns,
                });
            }
            #[cfg(target_arch = "wasm32")]
            Expr::ImportFile(_, _, _) => wasm_error("WASM does not support importing files"),
            #[cfg(not(target_arch = "wasm32"))]
            Expr::ImportFile(path, alias, markers) => {
                let file_path = file_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(path.as_str())
                    .canonicalize()
                    .unwrap_or_else(|_| {
                        std::env::current_exe().map_or_else(
                            |_| {
                                let current_src = &sources[src_file_idx as usize];
                                throw_compiler_error(
                                    (current_src.0.as_str(), current_src.1.as_str()),
                                    markers,
                                    ErrType::CannotReadImportedFile(path.as_str()),
                                );
                            },
                            |p| {
                                p.canonicalize()
                                    .unwrap_or(p)
                                    .parent()
                                    .unwrap_or_else(|| Path::new("."))
                                    .join("libs/")
                                    .join(path.clone())
                            },
                        )
                    });
                if visited_files.contains(&file_path) {
                    let current_src = &sources[src_file_idx as usize];
                    throw_compiler_error(
                        (current_src.0.as_str(), current_src.1.as_str()),
                        markers,
                        ErrType::CircularImport(file_path.to_str().unwrap_or(path.as_str())),
                    );
                }
                visited_files.push(file_path.clone());
                let file_contents =
                    Rc::new(std::fs::read_to_string(&file_path).unwrap_or_else(|_| {
                        let current_src = &sources[src_file_idx as usize];
                        throw_compiler_error(
                            (current_src.0.as_str(), current_src.1.as_str()),
                            markers,
                            ErrType::CannotReadImportedFile(path.as_str()),
                        );
                    }));
                let file_name: SmolStr = file_path.to_str().unwrap_or(path.as_str()).into();
                sources.push((file_name.clone(), file_contents.clone()));

                // Parse the imported file's contents
                let file_code = parser::parse(
                    file_contents.as_str(),
                    (file_name.as_str(), file_contents.as_str()),
                );

                let import_src: (&str, &str) = (file_name.as_str(), file_contents.as_str());
                let child_name = alias.unwrap_or_else(|| {
                    file_path
                        .file_prefix()
                        .and_then(|s| s.to_str())
                        .unwrap_or(path.as_str())
                        .to_smolstr()
                });
                let mut child_namespace = Namespace {
                    name: child_name,
                    fns: Vec::new(),
                    structs: Vec::new(),
                    children: Vec::new(),
                };
                parse_toplevel(
                    file_code,
                    &file_path,
                    (sources.len() - 1) as u16,
                    import_src,
                    fns,
                    structs,
                    struct_fields,
                    fn_registers,
                    dyn_libs,
                    dyn_lib_fns,
                    sources,
                    visited_files,
                    dyn_fn_id,
                    &mut child_namespace,
                );
                visited_files.pop_unchecked();
                namespace.children.push(child_namespace);
            }
            _ => {}
        }
    }
}

pub fn compile(
    contents: String,
    filename: &str,
    debug: bool,
) -> (
    Vec<Instr>,
    Vec<Data>,
    Pools,
    Vec<(Instr, Span, u16)>,
    Vec<Vec<u16>>,
    Vec<DynamicLibFn>,
    usize,
    usize,
    Vec<(SmolStr, Rc<String>)>,
    Vec<(SmolStr, Vec<SmolStr>)>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    let now = std::time::Instant::now();

    // let code: Vec<Expr> = grammar::FileParser::new()
    //     .parse((filename, &contents), &contents)
    //     .unwrap_or_else(|x| {
    //         crate::errors::lalrpop_error::<lalrpop_util::lexer::Token<'_>>(x, &contents, filename)
    //     });
    let code = parser::parse(&contents, (filename, &contents));

    #[cfg(not(target_arch = "wasm32"))]
    if debug {
        println!("PARSING TIME: {:.2?}", now.elapsed());
    }

    let mut variables: Vec<Variable> = Vec::new();
    let mut registers: Vec<Data> = Vec::new();
    let mut pools: Pools = Pools {
        objs: Pool::with_capacity(10),
        maps: Pool::with_capacity(2),
        strings: Pool::with_capacity(10),
    };
    let mut instr_src: Vec<(Instr, Span, u16)> = Vec::new();
    let mut fn_registers: Vec<Vec<u16>> = Vec::new();
    let mut functions: Vec<Function> = Vec::new();
    let mut structs: Vec<Struct> = Vec::new();
    let mut struct_fields: Vec<(SmolStr, Vec<SmolStr>)> = Vec::new();
    let mut dyn_libs: Vec<Dynamiclib> = Vec::new();
    let mut dyn_fn_id: u16 = 0;
    let mut dyn_lib_fns: Vec<DynamicLibFn> = Vec::new();
    let mut allocated_arg_count = 0;
    let mut allocated_call_depth = 0;
    let mut const_registers: FxHashMap<Data, u16> = FxHashMap::default();
    let mut free_registers = Vec::new();

    // sources[0] = main file
    let contents = Rc::new(contents);
    let mut sources: Vec<(SmolStr, Rc<String>)> = vec![(SmolStr::from(filename), contents.clone())];
    let main_path = PathBuf::from(filename)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(filename));
    let mut visited: Vec<PathBuf> = Vec::with_capacity(1);
    visited.push(main_path.clone());
    let mut namespace = Namespace {
        name: SmolStr::new_static(""),
        children: Vec::new(),
        fns: Vec::new(),
        structs: Vec::new(),
    };
    let main_src: (&str, &str) = (filename, &contents);
    parse_toplevel(
        code,
        &main_path,
        0,
        main_src,
        &mut functions,
        &mut structs,
        &mut struct_fields,
        &mut fn_registers,
        &mut dyn_libs,
        &mut dyn_lib_fns,
        &mut sources,
        &mut visited,
        &mut dyn_fn_id,
        &mut namespace,
    );
    // dbg!(&namespace);
    // dbg!(&functions);

    let ctx = Ctx {
        block_id: 0,
        src: (filename, &contents),
        is_parsing_recursive: false,
        current_src_file: 0,
        single_run: true,
        offset: 0,
    };
    let mut state = State {
        registers: &mut registers,
        fns: &mut functions,
        structs: &mut structs,
        struct_fields: &mut struct_fields,
        pools: &mut pools,
        instr_src: &mut instr_src,
        fn_registers: &mut fn_registers,
        dyn_libs: &mut dyn_libs,
        allocated_arg_count: &mut allocated_arg_count,
        allocated_call_depth: &mut allocated_call_depth,
        const_registers: &mut const_registers,
        free_registers: &mut free_registers,
        sources: &mut sources,
        reserved_registers: FxHashSet::default(),
        namespace: &mut namespace,
    };
    let mut instructions = compile_expr(
        &state.fns
            .iter()
            .find(|func| func.name == "main")
            .unwrap_or_else(|| {
                #[cfg(target_arch = "wasm32")]
                wasm_error("Cannot find main function");

                eprintln!(
                    "--------------\n{RED}KEEL RUNTIME ERROR:{RESET}\nCannot find {BLUE}{BOLD}main{RESET} function\n--------------",
                );
                std::process::exit(1);
            })
            .code
            .clone(),
        &mut variables,
        ctx,
        &mut state,
    );
    instructions.push(Instr::Halt(0));
    for x in &mut fn_registers {
        x.sort_unstable();
        x.dedup();
    }
    if debug {
        println!("---- DEBUG ----");
        if !pools.objs.is_empty() {
            println!("---  ARRAYS  ---");
            for (i, data) in pools.objs.iter().enumerate() {
                println!(" {i} {data:?}");
            }
        }
        println!("-- REGISTERS --");
        for (i, data) in registers.iter().enumerate() {
            println!(
                " [{i}] {}({})",
                data.type_name(),
                data.format(
                    &pools.objs,
                    &pools.strings,
                    &pools.maps,
                    &struct_fields,
                    true
                )
            );
        }
        if !instructions.is_empty() {
            println!("-- INSTRUCTIONS --");
            for (i, instr) in instructions.iter().enumerate() {
                println!(" {i}: {instr:?}");
            }
        }
        println!("------------------");
    }
    (
        instructions,
        registers,
        pools,
        instr_src,
        fn_registers,
        dyn_lib_fns,
        allocated_arg_count,
        allocated_call_depth,
        sources,
        struct_fields,
    )
}
