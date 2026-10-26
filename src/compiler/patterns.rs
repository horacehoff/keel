use crate::{
    compiler::{
        UnwrapId, compile_condition, compile_expr,
        compiler_data::{Ctx, State},
        compiler_errors::{
            error_invalid_pattern, error_invalid_type, error_struct_missing_fields,
            error_struct_unknown_field, error_unknown_struct,
        },
        expr::{
            ArrayPattern, Expr, MatchExpr, Pattern, Span, StructPattern, code_modifies_variable,
        },
        set_jmp_size,
        type_system::{DataType, var_type_is_compatible},
    },
    data::{Data, NULL},
    instr::{Instr, LibFunc},
};

struct PatternCompiler<'a, 'arena, 'compiler> {
    ctx: Ctx,
    state: &'a mut State<'arena, 'compiler>,
    output: &'a mut Vec<Instr>,
    false_jump_idxs: Vec<usize>,
    v_start: usize,
    can_never_match: bool,
}

fn join(types: Vec<DataType>) -> DataType {
    DataType::Union(types.into_boxed_slice()).check_poly()
}

fn narrow(candidate: &DataType, target: &DataType) -> Option<(bool, DataType)> {
    if var_type_is_compatible(target, candidate) {
        return Some((true, candidate.clone()));
    }
    let members = if let DataType::Union(members) = candidate {
        members
    } else {
        core::slice::from_ref(candidate)
    };
    let matching_types: Vec<DataType> =
        members.iter().filter(|member| var_type_is_compatible(target, member)).cloned().collect();

    if !matching_types.is_empty() {
        Some((false, join(matching_types)))
    } else if members.iter().any(|member| {
        member == &DataType::Unknown
            || (target == &DataType::Null && matches!(member, DataType::Struct(_)))
    }) {
        Some((false, target.clone()))
    } else {
        None
    }
}

const fn flip_jump(jump_instr: Instr) -> Option<Instr> {
    match jump_instr {
        Instr::NotEqJmp(a, b, jmp_size) => Some(Instr::EqJmp(a, b, jmp_size)),
        Instr::EqJmp(a, b, jmp_size) => Some(Instr::NotEqJmp(a, b, jmp_size)),
        Instr::ObjNotEqJmp(a, b, jmp_size) => Some(Instr::ObjEqJmp(a, b, jmp_size)),
        Instr::ObjEqJmp(a, b, jmp_size) => Some(Instr::ObjNotEqJmp(a, b, jmp_size)),
        _ => None,
    }
}

fn patch_jumps(output: &mut [Instr], jump_idxs: impl IntoIterator<Item = usize>, target: usize) {
    for jump_idx in jump_idxs {
        set_jmp_size(&mut output[jump_idx], (target - jump_idx) as u16);
    }
}

fn compile_pattern<'arena>(
    pattern: &'arena Pattern<'arena>,
    register_id: u16,
    candidate: &DataType,
    arm_code: &[Expr],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) -> Option<Vec<usize>> {
    let (v_start, output_start) = (state.v.len(), output.len());
    let mut compiler = PatternCompiler {
        ctx,
        state,
        output,
        false_jump_idxs: Vec::new(),
        v_start,
        can_never_match: false,
    };
    compiler.compile(pattern, register_id, candidate);
    let PatternCompiler { false_jump_idxs, can_never_match, .. } = compiler;
    if can_never_match {
        state.v.truncate(v_start);
        state.free_scope_registers(0, &output[output_start..]);
        output.truncate(output_start);
        return None;
    }

    // A binding shares its register with the value it was bound from.
    // Therefore if the arm reassigns the register, it needs to be copied.
    for idx in v_start..state.v.len() {
        let var_reg = state.v[idx].register_id;
        let mut shared =
            state.v.iter().chain(state.globals.iter()).filter(|var| var.register_id == var_reg);
        if shared.any(|var| code_modifies_variable(var.name, arm_code)) {
            let new = state.alloc_reg();
            output.push(Instr::Mov(var_reg, new));
            state.v[idx].register_id = new;
        }
    }
    Some(false_jump_idxs)
}

impl<'arena> PatternCompiler<'_, 'arena, '_> {
    fn error(&self, span: Span, msg: core::fmt::Arguments) -> ! {
        error_invalid_pattern(msg, None, span, self.ctx.file_idx, self.state.sources)
    }
    fn false_arm(&mut self, jump_instr: Instr) {
        self.false_jump_idxs.push(self.output.len());
        self.output.push(jump_instr);
    }
    fn bind_var(&mut self, name: &'arena str, register_id: u16, var_type: DataType, span: Span) {
        if self.state.v[self.v_start..].iter().any(|var| var.name == name) {
            self.error(span, format_args!("{name} is bound more than once"));
        }
        self.state.new_var(name, register_id, var_type);
    }
    /// Checks that `r[register_id]` is of type `target`
    fn check_type(
        &mut self,
        register_id: u16,
        candidate: &DataType,
        target: &DataType,
    ) -> (bool, DataType) {
        let Some((is_constant, narrowed_type)) = narrow(candidate, target) else {
            self.can_never_match = true;
            return (false, target.clone());
        };
        if !is_constant {
            let type_idx = self.state.compile_type(target.clone());
            self.false_arm(Instr::IsNotTypeJmp { src_reg_id: register_id, type_idx, jump_size: 0 });
        }
        (is_constant, narrowed_type)
    }

    fn compile(
        &mut self,
        pattern: &'arena Pattern<'arena>,
        register_id: u16,
        candidate: &DataType,
    ) -> DataType {
        match pattern {
            Pattern::Wildcard => candidate.clone(),
            Pattern::Binding(name, sub_pattern, span) => {
                let var_type = if let Some(sub) = sub_pattern {
                    self.compile(sub, register_id, candidate)
                } else {
                    candidate.clone()
                };
                self.bind_var(name, register_id, var_type.clone(), *span);
                var_type
            }
            Pattern::Constant(cst) => {
                let constant_type = cst.infer_type(self.ctx, self.state);
                self.can_never_match |= narrow(candidate, &constant_type).is_none();
                let constant_reg =
                    cst.compile(self.ctx, self.state, self.output, None, false, true).unwrap_id();
                self.false_arm(if constant_type == DataType::String {
                    Instr::ObjNotEqJmp(register_id, constant_reg, 0)
                } else {
                    Instr::NotEqJmp(register_id, constant_reg, 0)
                });
                constant_type
            }
            Pattern::Range { lower_bound, upper_bound, is_inclusive } => {
                let bound_type = lower_bound.infer_type(self.ctx, self.state);
                self.check_type(register_id, candidate, &bound_type);
                let lower_bound_reg_id = lower_bound
                    .compile(self.ctx, self.state, self.output, None, false, true)
                    .unwrap_id();
                self.false_arm(if bound_type == DataType::Int {
                    Instr::InfIntJmp(register_id, lower_bound_reg_id, 0)
                } else {
                    Instr::NotInfEqFloatJmp(lower_bound_reg_id, register_id, 0)
                });
                if let Some(upper_bound) = upper_bound {
                    let upper_bound_reg_id = upper_bound
                        .compile(self.ctx, self.state, self.output, None, false, true)
                        .unwrap_id();
                    self.false_arm(match (bound_type == DataType::Int, is_inclusive) {
                        (true, true) => Instr::InfIntJmp(upper_bound_reg_id, register_id, 0),
                        (true, false) => Instr::InfEqIntJmp(upper_bound_reg_id, register_id, 0),
                        (false, true) => {
                            Instr::NotInfEqFloatJmp(register_id, upper_bound_reg_id, 0)
                        }
                        (false, false) => Instr::NotInfFloatJmp(register_id, upper_bound_reg_id, 0),
                    });
                }
                bound_type
            }
            Pattern::Struct(struct_pattern) => {
                self.compile_struct_pattern(struct_pattern, register_id, candidate)
            }
            Pattern::Array(array_pattern) => {
                self.compile_array_pattern(array_pattern, register_id, candidate)
            }
            Pattern::Typed(sub_pattern, type_expr) => {
                let target = type_expr.to_datatype(
                    self.ctx.file_idx,
                    self.state.scope(self.ctx.file_idx),
                    self.state.sources,
                );
                let (_, narrowed_type) = self.check_type(register_id, candidate, &target);
                self.compile(sub_pattern, register_id, &narrowed_type)
            }
            Pattern::Or(sub_patterns, span) => {
                self.compile_or_pattern(sub_patterns, *span, register_id, candidate)
            }
        }
    }

    fn compile_struct_pattern(
        &mut self,
        StructPattern { qualified_name, fields, fill_the_rest, span }: &'arena StructPattern<
            'arena,
        >,
        register_id: u16,
        candidate: &DataType,
    ) -> DataType {
        let struct_idx = self
            .state
            .scope(self.ctx.file_idx)
            .find_struct(
                qualified_name.get_namespace(),
                qualified_name.get_name(),
                *span,
                self.ctx.file_idx,
                self.state.sources,
            )
            .unwrap_or_else(|| {
                error_unknown_struct(
                    qualified_name.get_name(),
                    *span,
                    self.state.sources,
                    self.ctx.file_idx,
                )
            });
        let struct_type = DataType::Struct(struct_idx as u16);
        if self.check_type(register_id, candidate, &struct_type).0 {
            let null_reg = self.state.new_const_reg(NULL);
            self.false_arm(Instr::EqJmp(register_id, null_reg, 0));
        }

        let mut matched_fields = vec![false; self.state.structs[struct_idx].fields.len()];
        for field in *fields {
            let s = &self.state.structs[struct_idx];
            let Some(field_idx) = s.fields.iter().position(|f| f.name == field.name) else {
                error_struct_unknown_field(
                    self.ctx.file_idx,
                    field.span,
                    field.name,
                    s.name,
                    &s.fields,
                    self.state.sources,
                )
            };
            if core::mem::replace(&mut matched_fields[field_idx], true) {
                self.error(
                    field.span,
                    format_args!("Field {} is matched more than once", field.name),
                )
            }
            if matches!(field.pattern, Pattern::Wildcard) {
                continue;
            }
            let field_type = s.fields[field_idx].field_type.clone();
            let field_reg_id = self.state.alloc_reg();
            self.output.push(Instr::GetFieldStruct(register_id, field_idx as u16, field_reg_id));
            self.compile(&field.pattern, field_reg_id, &field_type);
            // todo!("Pattern func call!");
        }

        // let s = &self.state.structs[struct_idx];
        if !fill_the_rest && matched_fields.contains(&false) {
            let s = &self.state.structs[struct_idx];
            let missing_fields: Vec<&str> = s
                .fields
                .iter()
                .zip(matched_fields)
                .filter(|(_, matched)| !*matched)
                .map(|(f, _)| f.name)
                .collect();
            error_struct_missing_fields(
                self.ctx.file_idx,
                s.name_span,
                *span,
                self.state.sources,
                &missing_fields,
            );
        }

        struct_type
    }

    fn compile_array_pattern(
        &mut self,
        ArrayPattern { prefix, rest, suffix }: &'arena ArrayPattern,
        register_id: u16,
        candidate: &DataType,
    ) -> DataType {
        let (_, array_type) = self.check_type(register_id, candidate, &DataType::Array(None));
        if prefix.is_empty() && suffix.is_empty() && matches!(rest, Some(Pattern::Wildcard)) {
            return array_type;
        }
        let elem_type: DataType = match &array_type {
            DataType::Array(Some(elem_type)) => (**elem_type).clone(),
            _ => DataType::Unknown,
        };

        let array_length_reg = self.state.alloc_reg();
        self.output.push(Instr::CallLibFunc(LibFunc::Len, register_id, array_length_reg));
        let min_match_len = prefix.len() + suffix.len();
        let min_match_len_reg = self.state.new_const_reg(Data::int(min_match_len as i32));
        if rest.is_none() {
            self.false_arm(Instr::NotEqJmp(array_length_reg, min_match_len_reg, 0));
        } else if min_match_len > 0 {
            self.false_arm(Instr::InfIntJmp(array_length_reg, min_match_len_reg, 0));
        }

        let mut suffix_start = suffix.is_empty().then_some(array_length_reg);
        for (i, elem) in prefix.iter().chain(*suffix).enumerate() {
            if matches!(elem, Pattern::Wildcard) {
                continue;
            }
            let idx_reg_id = if i < prefix.len() {
                self.state.new_const_reg(Data::int(i as i32))
            } else {
                let dest_reg = self.state.alloc_reg();
                self.output.push(Instr::SubInt(
                    array_length_reg,
                    self.state.new_const_reg(Data::int((min_match_len - i) as i32)),
                    dest_reg,
                ));
                dest_reg
            };
            if i == prefix.len() {
                suffix_start = Some(idx_reg_id);
            }
            let elem_reg_id = self.state.alloc_reg();
            self.output.push(Instr::GetIndexArray(register_id, idx_reg_id, elem_reg_id));
            self.compile(elem, elem_reg_id, &elem_type);
        }
        if let Some(rest @ Pattern::Binding(..)) = rest {
            let start_reg_id = self.state.new_const_reg(Data::int(prefix.len() as i32));
            let end_reg_id = suffix_start.unwrap_or_else(|| {
                let dest_reg_id = self.state.alloc_reg();
                self.output.push(Instr::SubInt(
                    array_length_reg,
                    self.state.new_const_reg(Data::int(suffix.len() as i32)),
                    dest_reg_id,
                ));
                dest_reg_id
            });
            self.output.push(Instr::StoreFuncArg(end_reg_id));
            self.state.add_arg_hint(1);
            let rest_reg_id = self.state.alloc_reg();
            self.output.push(Instr::GetSliceArray(register_id, start_reg_id, rest_reg_id));
            self.state.sub_arg_hint(1);
            self.compile(rest, rest_reg_id, &array_type);
        }
        array_type
    }
    fn compile_or_pattern(
        &mut self,
        sub_patterns: &'arena [Pattern<'arena>],
        span: Span,
        register_id: u16,
        candidate: &DataType,
    ) -> DataType {
        let (false_jump_start, v_start) = (self.false_jump_idxs.len(), self.state.v.len());
        let outer_can_never_match = core::mem::take(&mut self.can_never_match);

        let mut slots: Vec<(&str, u16, Vec<DataType>)> = Vec::new();
        let mut types = Vec::with_capacity(sub_patterns.len());
        let mut true_jumps = Vec::with_capacity(sub_patterns.len());
        let mut false_jumps = Vec::new();
        for pattern in sub_patterns {
            let pattern_start = self.output.len();
            let pattern_type = self.compile(pattern, register_id, candidate);
            let vars = self.state.v.split_off(v_start);
            // if it can never match just remove everything added since
            if core::mem::take(&mut self.can_never_match) {
                self.state.free_scope_registers(0, &self.output[pattern_start..]);
                self.output.truncate(pattern_start);
                self.false_jump_idxs.truncate(false_jump_start);
                continue;
            }
            patch_jumps(self.output, false_jumps, pattern_start);
            false_jumps = self.false_jump_idxs.split_off(false_jump_start);
            if types.is_empty() {
                slots = vars
                    .iter()
                    .map(|v| {
                        let slot = if v.register_id == register_id {
                            self.state.alloc_reg()
                        } else {
                            v.register_id
                        };
                        (v.name, slot, Vec::new())
                    })
                    .collect();
            }
            types.push(pattern_type);
            if vars.len() != slots.len()
                || !vars.iter().all(|var| slots.iter().any(|slot| slot.0 == var.name))
            {
                self.error(span, format_args!("Every sub-pattern must bind the same variables"));
            }
            for var in vars {
                let slot = slots.iter_mut().find(|slot| slot.0 == var.name).unwrap();
                slot.2.push(var.var_type);
                if var.register_id != slot.1 {
                    self.output.push(Instr::Mov(var.register_id, slot.1));
                }
            }

            if false_jumps.is_empty() {
                true_jumps.push(self.output.len());
                self.output.push(Instr::jump(0));
                break;
            }

            if false_jumps.len() == 1
                && (false_jumps[0] == self.output.len() - 1)
                && let Some(success) = flip_jump(*self.output.last().unwrap())
            {
                *self.output.last_mut().unwrap() = success;
                false_jumps.clear();
            } else {
                self.output.push(Instr::jump(0));
            }
            true_jumps.push(self.output.len() - 1);
        }
        if types.is_empty() {
            self.can_never_match = true;
            return candidate.clone();
        }

        // Last option falls through
        let last = true_jumps.pop().unwrap();
        if let Some(false_jump) = flip_jump(self.output[last]) {
            self.output[last] = false_jump;
            false_jumps.push(last);
        } else {
            self.output.pop();
        }

        let or_pattern_end = self.output.len();
        patch_jumps(self.output, true_jumps, or_pattern_end);
        self.false_jump_idxs.extend(false_jumps);
        self.can_never_match = outer_can_never_match;
        for (name, register_id, slot_types) in slots {
            self.bind_var(name, register_id, join(slot_types), span);
        }
        join(types)
    }
}

pub fn type_after_mismatch(
    pattern: &Pattern,
    guard: Option<&Expr>,
    candidate: &DataType,
    ctx: Ctx,
    state: &State,
) -> DataType {
    if let Pattern::Typed(Pattern::Wildcard | Pattern::Binding(_, None, _), type_expr) = pattern
        && guard.is_none()
        && let DataType::Union(members) = candidate
    {
        let target = type_expr.to_datatype(ctx.file_idx, state.scope(ctx.file_idx), state.sources);
        let remaining_members = members
            .iter()
            .filter(|m| matches!(m, DataType::Struct(_)) || !var_type_is_compatible(&target, m))
            .cloned()
            .collect();
        return join(remaining_members);
    }
    candidate.clone()
}

/// Declares the bindings in the variables for pre-codegen passes
/// Returns whether the match always matches (None if it never can)
pub fn declare_pattern_bindings<'arena>(
    pattern: &'arena Pattern<'arena>,
    candidate: &DataType,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
) -> Option<bool> {
    let v_len = state.v.len();
    let mut output = Vec::new();
    let false_jump_idxs = compile_pattern(pattern, 0, candidate, &[], ctx, state, &mut output)?;
    for var in &mut state.v[v_len..] {
        var.register_id = 0;
    }
    state.free_scope_registers(0, &output);
    Some(false_jump_idxs.is_empty())
}

pub fn compile_match_block<'arena>(
    MatchExpr { obj, arms, .. }: &'arena MatchExpr,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let mut candidate = obj.infer_type(ctx, state);
    let obj_reg_id = obj.compile(ctx, state, output, None, false, true).unwrap_id();

    let mut end_jumps = Vec::with_capacity(arms.len());
    let mut pending_false_jmps = Vec::new();

    for arm in *arms {
        let (arm_start, v_len) = (output.len(), state.v.len());
        let Some(mut false_jump_idxs) =
            compile_pattern(&arm.pattern, obj_reg_id, &candidate, arm.code, ctx, state, output)
        else {
            continue;
        };
        let pattern_end = output.len();

        if let Some(guard) = arm.guard {
            let guard_type = guard.infer_type(ctx, state);
            if guard_type != DataType::Bool {
                error_invalid_type(
                    &DataType::Bool,
                    &guard_type,
                    arm.guard_span,
                    None,
                    None,
                    ctx.file_idx,
                    state.sources,
                )
            }
            let (guard_b, guard_true_jump_idxs, guard_false_jump_idxs) =
                compile_condition(guard, ctx, state, output);
            if guard_b == Some(false) {
                // if it's a constant and it's false, just remove the now-dead arm
                state.v.truncate(v_len);
                state.free_scope_registers(0, &output[arm_start..]);
                output.truncate(arm_start);
                continue;
            }
            let body_start = output.len();
            patch_jumps(output, guard_true_jump_idxs, body_start);
            false_jump_idxs.extend(guard_false_jump_idxs);
        }
        patch_jumps(output, pending_false_jmps, arm_start);
        output.extend(compile_expr(arm.code, ctx.advance_offset(output.len() as u32), state));
        state.v.truncate(v_len);
        state.free_scope_registers(0, &output[arm_start..pattern_end]);
        end_jumps.push(output.len());
        output.push(Instr::jump(0));
        pending_false_jmps = false_jump_idxs;
        if pending_false_jmps.is_empty() {
            break;
        }
        candidate = type_after_mismatch(&arm.pattern, arm.guard, &candidate, ctx, state);
    }
    // the last just falls through, pop the Jmp(0)
    if end_jumps.pop().is_some() {
        output.pop();
    }
    let match_end = output.len();
    patch_jumps(output, pending_false_jmps, match_end);
    patch_jumps(output, end_jumps, match_end);
    state.free_reg(obj_reg_id);
}
