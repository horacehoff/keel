use super::conditions::compile_condition;
use super::conditions::compile_short_circuit_condition;
use super::conditions::patch_jumps;
use crate::ast::MatchExpr;
use crate::compiler::Ctx;
use crate::compiler::DataType;
use crate::compiler::Expr;
use crate::compiler::IfBlockExpr;
use crate::compiler::IntForLoopExpr;
use crate::compiler::Span;
use crate::compiler::State;
use crate::compiler::codegen::patterns::compile_pattern;
use crate::compiler::codegen::patterns::type_after_mismatch;
use crate::compiler::compile_expr;
use crate::compiler::error_invalid_type;
use crate::compiler::error_range_invalid_type;
use crate::compiler::error_type_not_indexable;
use crate::compiler::move_reg_to_reg;
use crate::compiler::move_value_to;
use crate::compiler::registers::UnwrapId;
use crate::data::Data;
use crate::instr::Instr;

const LOOP_MARKER: u16 = u16::MAX;
const LOOP_MARKER_REG: u16 = 1;

fn parse_loop_flow_control(loop_code: &mut [Instr]) {
    let body_len = loop_code.len() as u16;
    loop_code.iter_mut().enumerate().for_each(|(i, x)| {
        if *x == Instr::NotEqJmp(LOOP_MARKER_REG, 0, LOOP_MARKER) {
            *x = Instr::Jmp(body_len + 1 - i as u16);
        } else if *x == Instr::EqJmp(LOOP_MARKER_REG, 0, LOOP_MARKER) {
            // loop blocks and while loops only have 1 trailing instruction
            *x = Instr::Jmp(body_len - i as u16);
        }
    });
}

fn compile_if_block_branch<'arena>(
    branch: &'arena [Expr],
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    if let Some(tgt_id) = tgt_id {
        let regs_len = state.regs.temp_count;
        let output_len = output.len();
        output.extend(compile_expr(
            &branch[..branch.len() - 1],
            ctx.advance_offset(output.len() as u32),
            state,
        ));
        let val_id = branch[branch.len() - 1]
            .compile(
                ctx.advance_offset(output.len() as u32),
                state,
                output,
                Some(tgt_id),
                false,
                true,
            )
            .unwrap_id();
        state.free_scope_registers(regs_len, &output[output_len..]);
        if val_id != tgt_id {
            output.push(Instr::Mov(val_id, tgt_id));
        }
    } else {
        output.extend(compile_expr(branch, ctx.advance_offset(output.len() as u32), state));
    }
}

pub(super) fn compile_if_block<'arena>(
    IfBlockExpr { condition, then, otherwise, condition_span }: &'arena IfBlockExpr,
    previous_jumps: Vec<usize>,
    tgt_id: Option<u16>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let condition_start = output.len();
    patch_jumps(output, previous_jumps, condition_start);

    let condition_type = condition.infer_type(ctx, state);
    if condition_type != DataType::Bool {
        error_invalid_type(
            &DataType::Bool,
            &condition_type,
            *condition_span,
            None,
            None,
            ctx.file_idx,
            state.sources,
        )
    }
    let (b, true_jump_idxs, false_jump_idxs) = compile_condition(condition, ctx, state, output);
    if let Some(const_bool) = b {
        if const_bool {
            compile_if_block_branch(then, tgt_id, ctx, state, output);
            // This is just to make it so that the branch that's thrown away is still type-checked
            compile_if_block_branch(otherwise, tgt_id, ctx, state, &mut Vec::new());
        } else if let [Expr::IfBlock(if_block)] = otherwise {
            compile_if_block(if_block, Vec::new(), tgt_id, ctx, state, output);
        } else if !otherwise.is_empty() {
            compile_if_block_branch(otherwise, tgt_id, ctx, state, output);
        }
        return;
    }

    // Modify true jump instructions to point to body_start
    let body_start = output.len();
    patch_jumps(output, true_jump_idxs, body_start);

    compile_if_block_branch(then, tgt_id, ctx, state, output);

    if otherwise.is_empty() {
        let body_end = output.len();
        patch_jumps(output, false_jump_idxs, body_end);
        return;
    }

    let jump_over_idx = output.len();
    output.push(Instr::Jmp(0));
    if let [Expr::IfBlock(if_block)] = otherwise {
        compile_if_block(if_block, false_jump_idxs, tgt_id, ctx, state, output);
    } else {
        let branch_start = output.len();
        patch_jumps(output, false_jump_idxs, branch_start);
        compile_if_block_branch(otherwise, tgt_id, ctx, state, output);
    }

    let condition_end = output.len();
    patch_jumps(output, [jump_over_idx], condition_end);
}

pub(super) fn compile_while_loop<'arena>(
    condition: &'arena Expr,
    code: &'arena [Expr],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let output_len_before = output.len();

    let (true_jump_idxs, false_jump_idxs) =
        compile_short_circuit_condition(condition, ctx, state, output, false);

    let body_start = output.len();
    patch_jumps(output, true_jump_idxs, body_start);

    // parse the code block, clone the vars to avoid overriding anything
    let mut cond_code =
        compile_expr(code, ctx.no_single_run().advance_offset(output.len() as u32), state);

    let exit = output.len() + cond_code.len() + 1;
    patch_jumps(output, false_jump_idxs, exit);

    let cond_len = (output.len() - output_len_before) as u16;
    let body_len = cond_code.len() as u16;
    let len = cond_len + body_len; // full span used by JmpBack
    // Break/Continue offsets are relative to cond_code, so pass body_len+1 (body remaining + JmpBack)
    parse_loop_flow_control(&mut cond_code);
    output.extend(cond_code);
    output.push(Instr::Jmp(len.wrapping_neg()));
}

pub(super) fn compile_for_loop<'arena>(
    var_name: &'arena str,
    array: &'arena Expr,
    code: &'arena [Expr],
    span: Span,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let real_var = var_name != "_";

    // parse the array, get its id (the target array is the first Expr in array_code)
    let array_type = array.infer_type(ctx, state);
    let array = array.compile(ctx, state, output, None, false, true).unwrap_id();

    let array_len_id = state.alloc_reg();

    output.push(Instr::Len(array, array_len_id));

    // set up the id of the index variable (0..len)
    let index_id = if ctx.single_run {
        state.init_reg(Data::int(0), ctx)
    } else {
        let id = state.alloc_reg();
        output.push(Instr::SetInt(id, 0));
        id
    };

    let jmp_idx = output.len();
    output.push(Instr::InfEqIntJmp(array_len_id, index_id, 0));

    // set up the variable for the current element (for current_element_id in ... {}) => current_element_id = array[index]
    let current_element_id = if real_var { state.alloc_reg() } else { 0 };

    let v_len = state.v.len();

    let is_str = array_type == DataType::String;

    if real_var {
        state.new_var(
            var_name,
            current_element_id,
            match array_type {
                DataType::String => DataType::String,
                DataType::Array(a_type) => a_type.map_or(DataType::Null, |t| *t),
                t => {
                    error_type_not_indexable(&t, span, true, ctx.file_idx, state.sources);
                }
            },
        );
    }

    // accounts for the GetIndexArray/GetIndexString instruction
    let pending = u32::from(real_var);

    let regs_before = state.regs.temp_count;
    let mut loop_code: Vec<Instr> = compile_expr(
        code,
        ctx.no_single_run().advance_offset(output.len() as u32 + pending),
        state,
    );
    // Clean up variables
    state.v.truncate(v_len);
    state.free_scope_registers(regs_before, &loop_code);

    // make the current_element_id register actually hold the element's value
    if real_var {
        if is_str {
            output.push(Instr::GetIndexString(array, index_id, current_element_id));
        } else {
            output.push(Instr::GetIndexArray(array, index_id, current_element_id));
        }
    }
    parse_loop_flow_control(&mut loop_code);
    output.extend(loop_code);

    let body_len = (output.len() - jmp_idx - 1) as u16;
    output.push(Instr::IncIntInfJmp {
        counter_reg: index_id,
        limit_reg: array_len_id,
        jump_size: body_len.wrapping_neg(),
    });

    let exit_size = (output.len() - jmp_idx) as u16;
    output[jmp_idx] = Instr::InfEqIntJmp(array_len_id, index_id, exit_size);

    if ctx.single_run {
        state.free_reg(array_len_id);
        state.free_reg(index_id);
        if real_var {
            state.free_reg(current_element_id);
        }
    }
}

pub(super) fn compile_int_for_loop<'arena>(
    int_for_loop: &'arena IntForLoopExpr,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let lower_bound = int_for_loop.get_lower_bound();
    let upper_bound = int_for_loop.get_upper_bound();
    let code = int_for_loop.get_loop_code();

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
    let t1 = lower_bound.infer_type(ctx, state);
    let t2 = upper_bound.infer_type(ctx, state);
    if t1 != DataType::Int {
        error_range_invalid_type(int_for_loop.lower_bound_span, &t1, ctx.file_idx, state.sources);
    }
    if t2 != DataType::Int {
        error_range_invalid_type(int_for_loop.upper_bound_span, &t2, ctx.file_idx, state.sources);
    }
    let elem_id = state.alloc_reg();
    let output_len = output.len();
    let start_elem_id =
        lower_bound.compile(ctx, state, output, Some(elem_id), false, true).unwrap_id();
    if state.is_register_const(start_elem_id) {
        move_reg_to_reg(output, start_elem_id, elem_id, state.register_value(start_elem_id));
    } else {
        move_value_to(output, output_len, start_elem_id, elem_id);
    }
    let end_elem_id = upper_bound.compile(ctx, state, output, None, false, true).unwrap_id();

    let v_len = state.v.len();
    state.new_var(int_for_loop.var_name, elem_id, DataType::Int);

    // (1) if i >= end_elem jump out -> push placeholder first so that compile_expr sees the correct offset
    let jmp_idx = output.len();
    output.push(Instr::InfEqIntJmp(end_elem_id, elem_id, 0));

    let regs_before = state.regs.temp_count;
    let compiled_loop_code =
        compile_expr(code, ctx.no_single_run().advance_offset(output.len() as u32), state);
    state.free_scope_registers(regs_before, &compiled_loop_code);
    let compiled_loop_code_len = compiled_loop_code.len() as u16;

    // (2) loop_body
    output.extend(compiled_loop_code);

    // (3) i+= 1
    output.push(Instr::IncIntInfJmp {
        counter_reg: elem_id,
        limit_reg: end_elem_id,
        jump_size: compiled_loop_code_len.wrapping_neg(),
    });

    let exit_size = (output.len() - jmp_idx) as u16;
    output[jmp_idx] = Instr::InfEqIntJmp(end_elem_id, elem_id, exit_size);

    parse_loop_flow_control(
        &mut output[jmp_idx + 1..jmp_idx + 1 + compiled_loop_code_len as usize],
    );
    state.v.truncate(v_len);

    if ctx.single_run {
        state.free_reg(end_elem_id);
        state.free_reg(elem_id);
    }
}

pub(super) fn compile_loop_block<'arena>(
    code: &'arena [Expr],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let regs_before = state.regs.temp_count;
    let mut compiled =
        compile_expr(code, ctx.no_single_run().advance_offset(output.len() as u32), state);
    state.free_scope_registers(regs_before, &compiled);
    let code_length = compiled.len() as u16;
    parse_loop_flow_control(&mut compiled);
    output.extend(compiled);
    output.push(Instr::Jmp(code_length.wrapping_neg()));
}

pub(super) fn compile_try_catch_block<'arena>(
    e: &'arena [Expr],
    err_var: &'arena str,
    catch_code: &'arena [Expr],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    output.push(Instr::StartErrorCatch(0, 0)); // patched later on
    let err_catch_instr = output.len() - 1;
    let main_code = compile_expr(
        e,
        Ctx { try_depth: ctx.try_depth + 1, ..ctx }.advance_offset(output.len() as u32),
        state,
    );
    output.extend(main_code);
    output.push(Instr::StopErrorCatch);
    output.push(Instr::Jmp(0)); // jumps over the catch handler if no error arises
    let jmp_catch_instr = output.len() - 1;

    let v_len = state.v.len();
    let err_reg_id = state.alloc_reg();
    state.new_var(err_var, err_reg_id, DataType::String);
    output[err_catch_instr] =
        Instr::StartErrorCatch((output.len() - err_catch_instr) as u16, err_reg_id);
    let catch_code = compile_expr(catch_code, ctx.advance_offset(output.len() as u32), state);
    state.v.truncate(v_len);
    output.extend(catch_code);
    output[jmp_catch_instr] = Instr::Jmp((output.len() - jmp_catch_instr) as u16);
    state.free_reg(err_reg_id);
}

pub(super) fn compile_return<'arena>(
    return_value: Option<&'arena Expr<'arena>>,
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    let id = return_value.map(|x| match x {
        Expr::Int(n) if ctx.in_function => {
            let reg_id = state.alloc_reg();
            output.push(Instr::SetInt(reg_id, *n));
            state.free_reg(reg_id);
            reg_id
        }
        _ => x.compile(ctx, state, output, None, false, true).unwrap_id(),
    });
    if ctx.in_function {
        output.extend(core::iter::repeat_n(Instr::StopErrorCatch, ctx.try_depth as usize));
    }
    output.push(match (ctx.in_function, id) {
        (false, ..) => Instr::Halt(u16::MAX),
        (true, Some(id)) => Instr::Return(id),
        (true, None) => Instr::Return(0),
    });
}

#[inline(always)]
pub(super) fn compile_loop_break(ctx: Ctx, output: &mut Vec<Instr>) {
    output.extend(core::iter::repeat_n(
        Instr::StopErrorCatch,
        (ctx.try_depth - ctx.loop_try_depth) as usize,
    ));
    output.push(Instr::NotEqJmp(LOOP_MARKER_REG, 0, LOOP_MARKER));
}

#[inline(always)]
pub(super) fn compile_loop_continue(ctx: Ctx, output: &mut Vec<Instr>) {
    output.extend(core::iter::repeat_n(
        Instr::StopErrorCatch,
        (ctx.try_depth - ctx.loop_try_depth) as usize,
    ));
    output.push(Instr::EqJmp(LOOP_MARKER_REG, 0, LOOP_MARKER));
}

#[inline]
pub(super) fn compile_eval_block<'arena>(
    code: &'arena [Expr<'arena>],
    ctx: Ctx,
    state: &mut State<'arena, '_>,
    output: &mut Vec<Instr>,
) {
    output.extend(compile_expr(code, ctx.advance_offset(output.len() as u32), state));
}

pub(super) fn compile_match_block<'arena>(
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
        output.push(Instr::Jmp(0));
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
