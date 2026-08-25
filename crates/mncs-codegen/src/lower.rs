//! Lower selected SSA into the portable WASM MVP subset.

use std::collections::{BTreeMap, BTreeSet};

use mncs_model::{
    ArithmeticIntent, BodyType, IntegerType, IrType, SemanticId, SsaFunction, SsaInstructionKind,
    SsaModule, SsaTerminator, SsaValue,
};

use crate::wasm::{Instr, ValType, WasmFunction, WasmModule};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringOutcome {
    pub module: Option<WasmModule>,
    pub exports: Vec<String>,
    pub unsupported: Vec<String>,
}

struct FunctionLayout {
    values: BTreeMap<SemanticId, u32>,
    types: BTreeMap<SemanticId, ValType>,
    pc: u32,
    blocks: BTreeMap<SemanticId, u32>,
    block_params: BTreeMap<SemanticId, Vec<SemanticId>>,
    functions: BTreeMap<SemanticId, u32>,
}

pub fn lower_module(ssa: &SsaModule, names: &[String]) -> LoweringOutcome {
    let mut functions = Vec::new();
    let mut exports = Vec::new();
    let mut unsupported = Vec::new();
    let function_indices = ssa
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| (function.semantic_identity.clone(), index as u32))
        .collect::<BTreeMap<_, _>>();
    for (index, function) in ssa.functions.iter().enumerate() {
        let name = names
            .get(index)
            .cloned()
            .unwrap_or_else(|| export_name(&function.semantic_identity));
        match lower_function(function, name, &function_indices) {
            Ok(wasm) => {
                exports.push(wasm.name.clone());
                functions.push(wasm);
            }
            Err(reason) => unsupported.push(format!("{}: {reason}", function.identity.0)),
        }
    }
    let module = if unsupported.is_empty() && !functions.is_empty() {
        Some(WasmModule { functions })
    } else {
        None
    };
    LoweringOutcome {
        module,
        exports,
        unsupported,
    }
}

fn lower_function(
    function: &SsaFunction,
    name: String,
    function_indices: &BTreeMap<SemanticId, u32>,
) -> Result<WasmFunction, String> {
    if function.outputs.len() != 1 {
        return Err("portable WASM MVP requires exactly one result".to_owned());
    }
    reject_record_boundaries(function)?;
    let mut layout = layout_function(function, function_indices)?;
    forward_record_values(function, &mut layout)?;
    let result_ty = *layout
        .types
        .get(&function.outputs[0].identity)
        .ok_or_else(|| "function result type is unsupported".to_owned())?;
    let params = function
        .inputs
        .iter()
        .map(|input| {
            layout
                .types
                .get(&input.identity)
                .copied()
                .ok_or_else(|| "function parameter type is unsupported".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let local_count = layout.pc as usize + 1;
    let mut locals = vec![ValType::I32; local_count.saturating_sub(params.len())];
    for (identity, index) in &layout.values {
        if *index >= params.len() as u32 {
            locals[*index as usize - params.len()] = layout.types[identity];
        }
    }
    if layout.pc as usize >= params.len() {
        locals[layout.pc as usize - params.len()] = ValType::I32;
    }
    let mut body = vec![Instr::I32Const(0), Instr::LocalSet(layout.pc), Instr::Loop];
    for (block_index, block) in function.blocks.iter().enumerate() {
        body.push(Instr::LocalGet(layout.pc));
        body.push(Instr::I32Const(block_index as i32));
        body.push(Instr::I32Eq);
        body.push(Instr::If);
        for instruction in &block.instructions {
            lower_instruction(&layout, instruction, &mut body)?;
        }
        lower_terminator(&layout, &block.terminator, &mut body)?;
        body.push(Instr::End);
    }
    body.push(Instr::Unreachable);
    body.push(Instr::End);
    Ok(WasmFunction {
        name,
        params,
        results: vec![result_ty],
        locals,
        body,
    })
}

/// Rejects record values at every point where they would have to cross the
/// scalar parameter/result/block-parameter ABI of the portable WASM envelope.
/// Records are realized by forwarding inside a single function body; anything
/// that cannot be forwarded fails closed instead of being silently redefined.
fn reject_record_boundaries(function: &SsaFunction) -> Result<(), String> {
    let record = |ty: &IrType| match ty {
        IrType::Record { name, .. } => Some(name.clone()),
        _ => None,
    };
    for input in &function.inputs {
        if let Some(name) = record(&input.ty) {
            return Err(format!(
                "record value ({name}) cannot be a function parameter of this realization"
            ));
        }
    }
    for output in &function.outputs {
        if let Some(name) = record(&output.ty) {
            return Err(format!(
                "record value ({name}) cannot be a function result of this realization"
            ));
        }
    }
    for block in &function.blocks {
        for parameter in &block.parameters {
            if let Some(name) = record(&parameter.ty) {
                return Err(format!(
                    "record value ({name}) cannot be carried through a block parameter of this realization"
                ));
            }
        }
    }
    Ok(())
}

/// Rewrites SSA values produced by record construction/projection onto the
/// local slots of their originating field operand.  Record immutability makes
/// forwarding behavior-preserving; no record is ever materialized.
fn forward_record_values(
    function: &SsaFunction,
    layout: &mut FunctionLayout,
) -> Result<(), String> {
    // record value identity -> canonical (field name, operand identity) pairs
    let mut constructed: BTreeMap<SemanticId, Vec<(String, SemanticId)>> = BTreeMap::new();
    // projected value identity -> field operand identity
    let mut aliases: BTreeMap<SemanticId, SemanticId> = BTreeMap::new();
    let resolve = |aliases: &BTreeMap<SemanticId, SemanticId>, id: &SemanticId| {
        let mut current = id;
        while let Some(next) = aliases.get(current) {
            current = next;
        }
        current.clone()
    };
    for block in &function.blocks {
        for instruction in &block.instructions {
            match &instruction.kind {
                SsaInstructionKind::RecordConstruct { field_names, .. } => {
                    let Some(output) = instruction.outputs.first() else {
                        return Err("record construction has no result".to_owned());
                    };
                    if field_names.len() != instruction.inputs.len() {
                        return Err(
                            "record construction fields do not match its operands".to_owned()
                        );
                    }
                    constructed.insert(
                        output.identity.clone(),
                        field_names
                            .iter()
                            .cloned()
                            .zip(instruction.inputs.iter().cloned())
                            .collect(),
                    );
                }
                SsaInstructionKind::RecordProject { field, .. } => {
                    let Some(output) = instruction.outputs.first() else {
                        return Err("record projection has no result".to_owned());
                    };
                    let Some(input) = instruction.inputs.first() else {
                        return Err("record projection has no operand".to_owned());
                    };
                    let source = resolve(&aliases, input);
                    match constructed
                        .get(&source)
                        .and_then(|fields| fields.iter().find(|(name, _)| name == field))
                    {
                        Some((_, operand)) => {
                            aliases.insert(output.identity.clone(), operand.clone());
                        }
                        None => {
                            return Err(format!(
                                "record projection of {field:?} does not observe a locally constructed record; cross-function and block-carried records are unsupported by this realization"
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for (projected, operand) in aliases {
        if let Some(slot) = layout.values.get(&operand).copied() {
            layout.values.insert(projected, slot);
        }
    }
    Ok(())
}

fn layout_function(
    function: &SsaFunction,
    function_indices: &BTreeMap<SemanticId, u32>,
) -> Result<FunctionLayout, String> {
    let mut values = BTreeMap::new();
    let mut types = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut next = 0_u32;
    let mut push = |value: &SsaValue| -> Result<(), String> {
        if !seen.insert(value.identity.clone()) {
            return Ok(());
        }
        let (wasm, integer) = wasm_type(&value.ty)?;
        values.insert(value.identity.clone(), next);
        types.insert(value.identity.clone(), wasm);
        let _ = integer;
        next += 1;
        Ok(())
    };
    for input in &function.inputs {
        push(input)?;
    }
    for block in &function.blocks {
        for parameter in &block.parameters {
            push(parameter)?;
        }
        for instruction in &block.instructions {
            for output in &instruction.outputs {
                push(output)?;
            }
        }
    }
    for output in &function.outputs {
        push(output)?;
    }
    let pc = next;
    let blocks = function
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.identity.clone(), index as u32))
        .collect();
    let block_params = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.identity.clone(),
                block
                    .parameters
                    .iter()
                    .map(|parameter| parameter.identity.clone())
                    .collect(),
            )
        })
        .collect();
    Ok(FunctionLayout {
        values,
        types,
        pc,
        blocks,
        block_params,
        functions: function_indices.clone(),
    })
}

fn lower_instruction(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    match &instruction.kind {
        SsaInstructionKind::Constant { value, ty } => {
            let dest = dest_local(layout, instruction)?;
            emit_const(body, *value, ty)?;
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::Integer {
            operator,
            operand_type,
            intent,
        } => {
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            let result_type = instruction
                .outputs
                .first()
                .and_then(|output| wasm_type(&output.ty).ok().and_then(|(_, integer)| integer))
                .ok_or_else(|| "integer instruction result has no integer type".to_owned())?;
            emit_integer(
                body,
                operator,
                *operand_type,
                result_type,
                *intent,
                [left, right, dest],
            )?;
        }
        SsaInstructionKind::IntegerCompare {
            predicate,
            operand_type,
        } => {
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(compare_instr(predicate, *operand_type)?);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::FiniteConstruct {
            discriminant,
            payload_fields,
            ..
        } => {
            if !payload_fields.is_empty() {
                return Err(
                    "payload-bearing finite construction is outside the current realization envelope; the reference executors realize sums fully".to_owned(),
                );
            }
            let dest = dest_local(layout, instruction)?;
            body.push(Instr::I32Const(*discriminant as i32));
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::FinitePayloadProject { .. } => {
            return Err(
                "payload projection is outside the current realization envelope; the reference executors realize sums fully".to_owned(),
            );
        }
        SsaInstructionKind::FiniteIsVariant { discriminant, .. } => {
            let dest = dest_local(layout, instruction)?;
            let value = operand_local(layout, instruction, 0)?;
            body.push(Instr::LocalGet(value));
            body.push(Instr::I32Const(*discriminant as i32));
            body.push(Instr::I32Eq);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::Call { function, .. } => {
            let dest = dest_local(layout, instruction)?;
            for input in &instruction.inputs {
                body.push(Instr::LocalGet(local(layout, input)?));
            }
            let callee = layout
                .functions
                .get(function)
                .copied()
                .ok_or_else(|| "SSA call target is not in the WASM module".to_owned())?;
            body.push(Instr::Call(callee));
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::Effect => {
            return Err("effects are unsupported on the portable WASM MVP backend".to_owned());
        }
        SsaInstructionKind::RuntimeCheck { .. } => {
            return Err(
                "runtime checks have no executable condition in the current SSA subset".to_owned(),
            );
        }
        // Record values are never materialized: construction records nothing
        // and projection was already forwarded onto its field operand's local
        // slot by `forward_record_values` before instruction lowering.
        SsaInstructionKind::RecordConstruct { .. } => {}
        SsaInstructionKind::RecordProject { .. } => {}
    }
    Ok(())
}

fn lower_terminator(
    layout: &FunctionLayout,
    terminator: &SsaTerminator,
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    match terminator {
        SsaTerminator::Return { values } => {
            let value = values
                .first()
                .ok_or_else(|| "return terminator has no value".to_owned())?;
            body.push(Instr::LocalGet(local(layout, value)?));
            body.push(Instr::Return);
        }
        SsaTerminator::Branch { target, arguments } => {
            emit_block_args(layout, target, arguments, body)?;
            emit_goto(layout, target, body)?;
            body.push(Instr::Br(1));
        }
        SsaTerminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        } => {
            body.push(Instr::LocalGet(local(layout, condition)?));
            body.push(Instr::If);
            emit_block_args(layout, then_target, then_arguments, body)?;
            emit_goto(layout, then_target, body)?;
            body.push(Instr::Else);
            emit_block_args(layout, else_target, else_arguments, body)?;
            emit_goto(layout, else_target, body)?;
            body.push(Instr::End);
            body.push(Instr::Br(1));
        }
        SsaTerminator::Failure { mode } => {
            let _ = mode;
            body.push(Instr::Unreachable);
        }
    }
    Ok(())
}

fn emit_block_args(
    layout: &FunctionLayout,
    target: &SemanticId,
    arguments: &[SemanticId],
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    let params = layout
        .block_params
        .get(target)
        .ok_or_else(|| "branch target is missing".to_owned())?;
    if params.len() != arguments.len() {
        return Err("branch arguments do not match target block parameters".to_owned());
    }
    for argument in arguments {
        body.push(Instr::LocalGet(local(layout, argument)?));
    }
    for param in params.iter().rev() {
        body.push(Instr::LocalSet(local(layout, param)?));
    }
    Ok(())
}

fn emit_goto(
    layout: &FunctionLayout,
    target: &SemanticId,
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    let index = *layout
        .blocks
        .get(target)
        .ok_or_else(|| "branch target is missing".to_owned())?;
    body.push(Instr::I32Const(index as i32));
    body.push(Instr::LocalSet(layout.pc));
    Ok(())
}

fn emit_const(body: &mut Vec<Instr>, value: i128, ty: &IrType) -> Result<(), String> {
    match wasm_type(ty)?.0 {
        ValType::I32 => body.push(Instr::I32Const(
            i32::try_from(value).map_err(|_| "constant does not fit in wasm i32".to_owned())?,
        )),
        ValType::I64 => body.push(Instr::I64Const(
            i64::try_from(value).map_err(|_| "constant does not fit in wasm i64".to_owned())?,
        )),
    }
    Ok(())
}

fn emit_integer(
    body: &mut Vec<Instr>,
    operator: &str,
    operand_type: IntegerType,
    result_type: IntegerType,
    intent: ArithmeticIntent,
    locals: [u32; 3],
) -> Result<(), String> {
    let [left, right, dest] = locals;
    let wasm = val_type(operand_type)?;
    let wrapping = match (wasm, operator) {
        (ValType::I32, "add") => Instr::I32Add,
        (ValType::I32, "sub") => Instr::I32Sub,
        (ValType::I32, "mul") => Instr::I32Mul,
        (ValType::I32, "and") => Instr::I32And,
        (ValType::I32, "or") => Instr::I32Or,
        (ValType::I32, "xor") => Instr::I32Xor,
        (ValType::I64, "add") => Instr::I64Add,
        (ValType::I64, "sub") => Instr::I64Sub,
        (ValType::I64, "mul") => Instr::I64Mul,
        (ValType::I64, "and") => Instr::I64And,
        (ValType::I64, "or") => Instr::I64Or,
        (ValType::I64, "xor") => Instr::I64Xor,
        _ => return Err(format!("unsupported integer operator {operator}")),
    };
    match intent {
        ArithmeticIntent::Wrapping => {
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(wrapping);
            mask_width(body, result_type, wasm);
            body.push(Instr::LocalSet(dest));
        }
        ArithmeticIntent::Checked | ArithmeticIntent::Trapping => {
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(wrapping);
            body.push(Instr::LocalSet(dest));
            emit_checked(body, operator, operand_type, wasm, left, right, dest)?;
            mask_width(body, result_type, wasm);
            if result_type.bits < 32 && matches!(wasm, ValType::I32) {
                body.push(Instr::LocalGet(dest));
                mask_width(body, result_type, wasm);
                body.push(Instr::LocalSet(dest));
            }
        }
        ArithmeticIntent::Saturating => {
            emit_saturating(body, operator, operand_type, result_type, left, right, dest)?
        }
        ArithmeticIntent::Widening { .. } => {
            emit_widening(body, operator, operand_type, result_type, left, right, dest)?
        }
    }
    Ok(())
}

fn emit_wide_i64_operation(
    body: &mut Vec<Instr>,
    operator: &str,
    ty: IntegerType,
    left: u32,
    right: u32,
) -> Result<(), String> {
    body.push(Instr::LocalGet(left));
    body.push(if ty.signed {
        Instr::I64ExtendI32S
    } else {
        Instr::I64ExtendI32U
    });
    body.push(Instr::LocalGet(right));
    body.push(if ty.signed {
        Instr::I64ExtendI32S
    } else {
        Instr::I64ExtendI32U
    });
    body.push(match operator {
        "add" => Instr::I64Add,
        "sub" => Instr::I64Sub,
        "mul" => Instr::I64Mul,
        _ => return Err(format!("unsupported exact arithmetic operator {operator}")),
    });
    Ok(())
}

fn emit_saturating(
    body: &mut Vec<Instr>,
    operator: &str,
    operand: IntegerType,
    result: IntegerType,
    left: u32,
    right: u32,
    dest: u32,
) -> Result<(), String> {
    if result != operand
        || operand.bits > 32
        || (!operand.signed && operand.bits > 31)
        || !matches!(operator, "add" | "sub" | "mul")
    {
        return Err("portable WASM saturating arithmetic requires signed 1..=32-bit or unsigned 1..=31-bit add/sub/mul".to_owned());
    }
    let (min, max) = if operand.signed {
        let top = 1_i64 << (operand.bits - 1);
        (-top, top - 1)
    } else {
        (0, (1_i64 << operand.bits) - 1)
    };
    emit_wide_i64_operation(body, operator, operand, left, right)?;
    body.push(Instr::I64Const(max));
    body.push(Instr::I64GtS);
    body.push(Instr::If);
    body.push(Instr::I32Const(
        i32::try_from(max).map_err(|_| "saturation maximum is not i32")?,
    ));
    body.push(Instr::LocalSet(dest));
    body.push(Instr::Else);
    emit_wide_i64_operation(body, operator, operand, left, right)?;
    body.push(Instr::I64Const(min));
    body.push(Instr::I64LtS);
    body.push(Instr::If);
    body.push(Instr::I32Const(
        i32::try_from(min).map_err(|_| "saturation minimum is not i32")?,
    ));
    body.push(Instr::LocalSet(dest));
    body.push(Instr::Else);
    emit_wide_i64_operation(body, operator, operand, left, right)?;
    body.push(Instr::I32WrapI64);
    mask_width(body, result, ValType::I32);
    body.push(Instr::LocalSet(dest));
    body.push(Instr::End);
    body.push(Instr::End);
    Ok(())
}

fn emit_widening(
    body: &mut Vec<Instr>,
    operator: &str,
    operand: IntegerType,
    result: IntegerType,
    left: u32,
    right: u32,
    dest: u32,
) -> Result<(), String> {
    if operand.bits > 32 || result.bits > 64 || result.signed != operand.signed {
        return Err(
            "portable WASM widening arithmetic requires operands <= 32 bits and results <= 64 bits"
                .to_owned(),
        );
    }
    if mncs_model::arithmetic_result_type(
        operator,
        operand,
        ArithmeticIntent::Widening { bits: result.bits },
    ) != Some(result)
    {
        return Err("widening result type is not exact for this operation".to_owned());
    }
    emit_wide_i64_operation(body, operator, operand, left, right)?;
    if matches!(val_type(result)?, ValType::I32) {
        body.push(Instr::I32WrapI64);
        mask_width(body, result, ValType::I32);
    }
    body.push(Instr::LocalSet(dest));
    Ok(())
}

fn emit_checked(
    body: &mut Vec<Instr>,
    operator: &str,
    ty: IntegerType,
    wasm: ValType,
    left: u32,
    right: u32,
    dest: u32,
) -> Result<(), String> {
    if !matches!(operator, "add" | "sub" | "mul") {
        return Err(format!(
            "checked integer operator {operator} is unsupported on the portable WASM MVP backend"
        ));
    }
    match wasm {
        ValType::I32 if ty.signed => {
            let max = max_signed(ty.bits);
            let min = -max - 1;
            emit_range_trap_i64(body, left, right, operator, min, max, true)
        }
        ValType::I32 => emit_unsigned_trap_i32(body, left, right, dest, operator, ty.bits),
        ValType::I64 if ty.signed => {
            emit_signed_i64_overflow_trap(body, left, right, dest, operator)
        }
        ValType::I64 => emit_unsigned_i64_overflow_trap(body, left, dest, operator),
    }
}

fn emit_range_trap_i64(
    body: &mut Vec<Instr>,
    left: u32,
    right: u32,
    operator: &str,
    min: i128,
    max: i128,
    signed_extend: bool,
) -> Result<(), String> {
    body.push(Instr::LocalGet(left));
    if signed_extend {
        body.push(Instr::I64ExtendI32S);
    } else {
        body.push(Instr::I64ExtendI32U);
    }
    body.push(Instr::LocalGet(right));
    if signed_extend {
        body.push(Instr::I64ExtendI32S);
    } else {
        body.push(Instr::I64ExtendI32U);
    }
    body.push(match operator {
        "add" => Instr::I64Add,
        "sub" => Instr::I64Sub,
        _ => Instr::I64Mul,
    });
    body.push(Instr::I64Const(i64::try_from(max).unwrap_or(i64::MAX)));
    body.push(Instr::I64GtS);
    body.push(Instr::LocalGet(left));
    if signed_extend {
        body.push(Instr::I64ExtendI32S);
    } else {
        body.push(Instr::I64ExtendI32U);
    }
    body.push(Instr::LocalGet(right));
    if signed_extend {
        body.push(Instr::I64ExtendI32S);
    } else {
        body.push(Instr::I64ExtendI32U);
    }
    body.push(match operator {
        "add" => Instr::I64Add,
        "sub" => Instr::I64Sub,
        _ => Instr::I64Mul,
    });
    body.push(Instr::I64Const(i64::try_from(min).unwrap_or(i64::MIN)));
    body.push(Instr::I64LtS);
    body.push(Instr::I32Or);
    body.push(Instr::If);
    body.push(Instr::Unreachable);
    body.push(Instr::End);
    Ok(())
}

fn emit_unsigned_trap_i32(
    body: &mut Vec<Instr>,
    left: u32,
    right: u32,
    dest: u32,
    operator: &str,
    bits: u16,
) -> Result<(), String> {
    let max = (1_i64 << bits) - 1;
    body.push(Instr::LocalGet(left));
    body.push(Instr::I64ExtendI32U);
    body.push(Instr::LocalGet(right));
    body.push(Instr::I64ExtendI32U);
    body.push(match operator {
        "add" => Instr::I64Add,
        "sub" => Instr::I64Sub,
        _ => Instr::I64Mul,
    });
    body.push(Instr::I64Const(max));
    body.push(Instr::I64GtU);
    body.push(Instr::If);
    body.push(Instr::Unreachable);
    body.push(Instr::End);
    let _ = dest;
    Ok(())
}

fn emit_signed_i64_overflow_trap(
    body: &mut Vec<Instr>,
    left: u32,
    right: u32,
    dest: u32,
    operator: &str,
) -> Result<(), String> {
    if operator != "add" && operator != "sub" {
        return Err(
            "checked i64 multiply is unsupported on the portable WASM MVP backend".to_owned(),
        );
    }
    // (a ^ b) >= 0 && (a ^ result) < 0 for add; invert b for sub.
    body.push(Instr::LocalGet(left));
    body.push(Instr::LocalGet(right));
    if operator == "sub" {
        body.push(Instr::I64Const(-1));
        body.push(Instr::I64Xor);
    }
    body.push(Instr::I64Xor);
    body.push(Instr::I64Const(0));
    body.push(Instr::I64GeS);
    body.push(Instr::LocalGet(left));
    body.push(Instr::LocalGet(dest));
    body.push(Instr::I64Xor);
    body.push(Instr::I64Const(0));
    body.push(Instr::I64LtS);
    body.push(Instr::I32And);
    body.push(Instr::If);
    body.push(Instr::Unreachable);
    body.push(Instr::End);
    Ok(())
}

fn emit_unsigned_i64_overflow_trap(
    body: &mut Vec<Instr>,
    left: u32,
    dest: u32,
    operator: &str,
) -> Result<(), String> {
    match operator {
        "add" => {
            body.push(Instr::LocalGet(dest));
            body.push(Instr::LocalGet(left));
            body.push(Instr::I64LtU);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
        }
        "sub" => {
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(dest));
            body.push(Instr::I64LtU);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
        }
        _ => {
            return Err(
                "checked unsigned i64 multiply is unsupported on the portable WASM MVP backend"
                    .to_owned(),
            )
        }
    }
    Ok(())
}

fn mask_width(body: &mut Vec<Instr>, ty: IntegerType, wasm: ValType) {
    if ty.bits >= 32 && matches!(wasm, ValType::I32) || ty.bits >= 64 {
        return;
    }
    if matches!(wasm, ValType::I32) {
        let mask = (1_i32.wrapping_shl(u32::from(ty.bits))) - 1;
        body.push(Instr::I32Const(mask));
        body.push(Instr::I32And);
        if ty.signed {
            // Sign-extend an N-bit value without relying on optional WASM
            // sign-extension instructions: (masked ^ sign) - sign.
            let sign = 1_i32.wrapping_shl(u32::from(ty.bits - 1));
            body.push(Instr::I32Const(sign));
            body.push(Instr::I32Xor);
            body.push(Instr::I32Const(sign));
            body.push(Instr::I32Sub);
        }
    }
}

fn compare_instr(predicate: &str, ty: IntegerType) -> Result<Instr, String> {
    let wasm = val_type(ty)?;
    Ok(match (wasm, predicate, ty.signed) {
        (ValType::I32, "eq", _) => Instr::I32Eq,
        (ValType::I32, "ne", _) => Instr::I32Ne,
        (ValType::I32, "lt", true) => Instr::I32LtS,
        (ValType::I32, "lt", false) => Instr::I32LtU,
        (ValType::I32, "le", true) => Instr::I32LeS,
        (ValType::I32, "le", false) => Instr::I32LeU,
        (ValType::I32, "gt", true) => Instr::I32GtS,
        (ValType::I32, "gt", false) => Instr::I32GtU,
        (ValType::I32, "ge", true) => Instr::I32GeS,
        (ValType::I32, "ge", false) => Instr::I32GeU,
        (ValType::I64, "eq", _) => Instr::I64Eq,
        (ValType::I64, "ne", _) => Instr::I64Ne,
        (ValType::I64, "lt", true) => Instr::I64LtS,
        (ValType::I64, "lt", false) => Instr::I64LtU,
        (ValType::I64, "le", true) => Instr::I64LeS,
        (ValType::I64, "le", false) => Instr::I64LeU,
        (ValType::I64, "gt", true) => Instr::I64GtS,
        (ValType::I64, "gt", false) => Instr::I64GtU,
        (ValType::I64, "ge", true) => Instr::I64GeS,
        (ValType::I64, "ge", false) => Instr::I64GeU,
        _ => return Err(format!("unsupported comparison predicate {predicate}")),
    })
}

fn dest_local(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
) -> Result<u32, String> {
    let output = instruction
        .outputs
        .first()
        .ok_or_else(|| "instruction has no destination".to_owned())?;
    local(layout, &output.identity)
}

fn operand_local(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
    index: usize,
) -> Result<u32, String> {
    let operand = instruction
        .inputs
        .get(index)
        .ok_or_else(|| "instruction is missing an operand".to_owned())?;
    local(layout, operand)
}

fn local(layout: &FunctionLayout, identity: &SemanticId) -> Result<u32, String> {
    layout
        .values
        .get(identity)
        .copied()
        .ok_or_else(|| format!("SSA value {} is not mapped to a WASM local", identity.0))
}

fn wasm_type(ty: &IrType) -> Result<(ValType, Option<IntegerType>), String> {
    match ty {
        IrType::Finite { .. } => Ok((ValType::I32, None)),
        // Record values are never materialized; the placeholder slot exists
        // only because every SSA value is pre-allocated a local index.
        IrType::Record { .. } => Ok((ValType::I32, None)),
        IrType::Named(name) if name == "bool" => Ok((ValType::I32, None)),
        IrType::Named(name) => match BodyType::from_semantic_name(name) {
            BodyType::Integer(integer) => Ok((val_type(integer)?, Some(integer))),
            BodyType::Named(_) | BodyType::Finite { .. } | BodyType::Record { .. } => {
                Err(format!("unsupported SSA type {name}"))
            }
        },
    }
}

fn val_type(ty: IntegerType) -> Result<ValType, String> {
    crate::wasm::val_type_for(ty)
        .ok_or_else(|| format!("integer width {} is unsupported on WASM MVP", ty.bits))
}

fn max_signed(bits: u16) -> i128 {
    (1_i128 << (bits - 1)) - 1
}

fn export_name(identity: &SemanticId) -> String {
    identity
        .0
        .rsplit(':')
        .next()
        .unwrap_or(identity.0.as_str())
        .to_owned()
}
