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

pub fn lower_module(
    program: &mncs_model::Program,
    ssa: &SsaModule,
    names: &[String],
) -> LoweringOutcome {
    let composites = CompositeInfo::from_program(program);
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
        match lower_function(function, name, &function_indices, &composites) {
            Ok(wasm) => {
                exports.push(wasm.name.clone());
                functions.push(wasm);
            }
            Err(reason) => unsupported.push(format!("{}: {reason}", function.identity.0)),
        }
    }
    let module = if unsupported.is_empty() && !functions.is_empty() {
        Some(if composites.uses_composites {
            WasmModule {
                globals: vec![crate::wasm::WasmGlobal {
                    valtype: ValType::I32,
                    mutable: true,
                    init: 8,
                }],
                // Composite values are immutable cells. A bounded streaming
                // consumer may retain one state cell per input step, so the
                // old 1 MiB arena was too small for realistic byte streams
                // even though every individual sequence bound was finite.
                // Keep the artifact bounded while giving host-buffer-based
                // applications a documented 32 MiB arena budget.
                memory: Some(crate::wasm::WasmMemory { min_pages: 512 }),
                functions,
            }
        } else {
            WasmModule {
                globals: Vec::new(),
                memory: None,
                functions,
            }
        })
    } else {
        None
    };
    LoweringOutcome {
        module,
        exports,
        unsupported,
    }
}

/// Logical slot width of one composite field cell in linear memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotWidth {
    W32,
    W64,
}

/// Per-type realization facts derived from the language-owned program.
#[derive(Debug, Clone, Default)]
struct CompositeInfo {
    /// record type identity -> canonical (field name, slot width)
    records: BTreeMap<SemanticId, Vec<(String, SlotWidth)>>,
    /// payload-bearing finite type identity -> discriminant -> payload fields
    boxed_finites: BTreeMap<SemanticId, BTreeMap<u32, Vec<(String, SlotWidth)>>>,
    uses_composites: bool,
}

impl CompositeInfo {
    fn from_program(program: &mncs_model::Program) -> Self {
        // Records and boxed finite variants occupy referenced cells; bare
        // tags and narrow scalars stay in 32-bit slots.
        let width_of = |semantic_type: &str| -> SlotWidth {
            // Canonical cells reserve eight bytes per field, but a nested
            // record/sequence/vector value is an i32 cell reference in the
            // portable WASM locals. The store/load width below is therefore
            // W32; the remaining slot bytes stay padding.
            if program
                .record_types
                .iter()
                .any(|record| record.name == semantic_type)
            {
                return SlotWidth::W32;
            }
            if program
                .finite_types
                .iter()
                .any(|finite| finite.name == semantic_type)
            {
                return SlotWidth::W32;
            }
            match mncs_model::BodyType::from_semantic_name(semantic_type) {
                mncs_model::BodyType::Integer(ty) if ty.bits == 64 => SlotWidth::W64,
                // Packed view descriptors and masks are full i64 values.
                // Exact sequences and vectors are i32 cell references, like
                // nested records above.
                mncs_model::BodyType::Sequence {
                    bound: mncs_model::SequenceBound::UpTo(_),
                    ..
                }
                | mncs_model::BodyType::Mask { .. } => SlotWidth::W64,
                _ => SlotWidth::W32,
            }
        };
        let mut info = Self::default();
        for record in &program.record_types {
            let fields = record
                .fields
                .iter()
                .map(|field| (field.name.clone(), width_of(&field.field_type)))
                .collect::<Vec<_>>();
            info.records.insert(record.identity.clone(), fields);
        }
        for finite in &program.finite_types {
            // A type is boxed when ANY variant carries a payload; every
            // variant of that type gets a layout entry (possibly empty).
            let boxed = finite
                .variants
                .iter()
                .any(|variant| !variant.payload.is_empty());
            if boxed {
                let variants = finite
                    .variants
                    .iter()
                    .map(|variant| {
                        (
                            variant.discriminant,
                            variant
                                .payload
                                .iter()
                                .map(|field| (field.name.clone(), width_of(&field.field_type)))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                info.boxed_finites.insert(finite.identity.clone(), variants);
            }
        }
        info.uses_composites = !info.records.is_empty()
            || !info.boxed_finites.is_empty()
            || program_uses_sequences(program);
        info
    }

    fn is_boxed_finite(&self, type_identity: &SemanticId) -> bool {
        self.boxed_finites.contains_key(type_identity)
    }
}

/// Whether any executable body materializes bounded sequences; such programs
/// need linear-memory arena support even without records or payload sums.
fn program_uses_sequences(program: &mncs_model::Program) -> bool {
    let ty_uses_arena =
        |ty: &BodyType| matches!(ty, BodyType::Sequence { .. } | BodyType::Vector { .. });
    let mut found = false;
    for function in &program.functions {
        if let Some(body) = &function.body {
            found |= body.parameters.iter().any(|param| ty_uses_arena(&param.ty));
            for block in &body.blocks {
                for operation in &block.operations {
                    found |= matches!(
                        operation.kind,
                        mncs_model::BodyOperationKind::SequenceConstruct { .. }
                            | mncs_model::BodyOperationKind::VectorConstruct { .. }
                            | mncs_model::BodyOperationKind::VectorSplat { .. }
                            | mncs_model::BodyOperationKind::VectorReplace { .. }
                            | mncs_model::BodyOperationKind::VectorBinary { .. }
                    );
                    found |= operation
                        .results
                        .iter()
                        .any(|result| ty_uses_arena(&result.ty));
                }
            }
        }
    }
    found
}

fn lower_function(
    function: &SsaFunction,
    name: String,
    function_indices: &BTreeMap<SemanticId, u32>,
    composites: &CompositeInfo,
) -> Result<WasmFunction, String> {
    if function.outputs.len() != 1 {
        return Err("portable WASM MVP requires exactly one result".to_owned());
    }
    let layout = layout_function(function, function_indices, composites)?;
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
            lower_instruction(&layout, instruction, &mut body, composites)?;
        }
        lower_terminator(&layout, &block.terminator, &mut body)?;
        body.push(Instr::End);
    }
    // Close the dispatcher loop before the terminal fallback. Keeping the
    // unreachable outside the loop makes the function-level fallthrough
    // unreachable to a standard WASM validator while preserving the
    // embedded interpreter's fail-closed behavior.
    body.push(Instr::End);
    body.push(Instr::Unreachable);
    Ok(WasmFunction {
        name,
        params,
        results: vec![result_ty],
        locals,
        body,
    })
}

fn layout_function(
    function: &SsaFunction,
    function_indices: &BTreeMap<SemanticId, u32>,
    _composites: &CompositeInfo,
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
    composites: &CompositeInfo,
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
            type_identity,
            discriminant,
            payload_fields,
            ..
        } => {
            let dest = dest_local(layout, instruction)?;
            match composites.boxed_finites.get(type_identity) {
                Some(variants) => {
                    // Boxed representation: allocate a tag cell plus one
                    // 8-byte slot per payload field of THIS variant.
                    let fields = variants.get(discriminant).ok_or_else(|| {
                        format!("finite variant {discriminant} has no declared payload layout")
                    })?;
                    if fields.len() != payload_fields.len() {
                        return Err(
                            "payload field count does not match the declared variant layout"
                                .to_owned(),
                        );
                    }
                    emit_alloc(body, dest, (fields.len() as u32 + 1) * 8)?;
                    body.push(Instr::LocalGet(dest));
                    body.push(Instr::I32Const(*discriminant as i32));
                    body.push(Instr::I32Store);
                    for (index, _) in fields.iter().enumerate() {
                        let operand = operand_local(layout, instruction, index)?;
                        let offset = ((index + 1) * 8) as i32;
                        body.push(Instr::LocalGet(dest));
                        emit_offset(body, offset);
                        body.push(Instr::LocalGet(operand));
                        store_instr(layout, instruction, index, body)?;
                    }
                }
                None => {
                    if !payload_fields.is_empty() {
                        return Err(
                            "payload-bearing construction on an unboxed finite type".to_owned()
                        );
                    }
                    body.push(Instr::I32Const(*discriminant as i32));
                    body.push(Instr::LocalSet(dest));
                }
            }
        }
        SsaInstructionKind::BooleanOp { operator } => {
            // Bools are normalized 0/1 i32 locals in this realization, so
            // strict conjunction/disjunction is a bitwise op on them.
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(match operator.as_str() {
                "and" => Instr::I32And,
                "or" => Instr::I32Or,
                other => return Err(format!("unsupported boolean operator {other}")),
            });
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::ByteBitwise { operator } => {
            // Bytes ride zero-extended in i32 cells; bitwise ops are exact.
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(match operator.as_str() {
                "and" => Instr::I32And,
                "or" => Instr::I32Or,
                "xor" => Instr::I32Xor,
                other => return Err(format!("unsupported byte bitwise operator {other}")),
            });
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::ByteShift { operator } => {
            // Total semantics: the u64 count is taken modulo 8 (count rides
            // i64), shifts are logical, and the result is masked to a byte.
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(Instr::I64Const(8));
            body.push(Instr::I64RemU);
            body.push(Instr::I32WrapI64);
            if operator == "shl" {
                body.push(Instr::I32Shl);
                body.push(Instr::I32Const(255));
                body.push(Instr::I32And);
            } else if operator == "shr" {
                body.push(Instr::I32ShrU);
            } else {
                return Err(format!("unsupported byte shift operator {operator}"));
            }
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::ByteCompare { predicate } => {
            // Bytes compare as unsigned 8-bit values in i32 cells.
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            body.push(Instr::LocalGet(left));
            body.push(Instr::LocalGet(right));
            body.push(compare_instr(
                predicate,
                IntegerType {
                    bits: 8,
                    signed: false,
                },
            )?);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::Convert { from, to } => {
            emit_convert(layout, instruction, body, from.clone(), to.clone())?
        }
        SsaInstructionKind::SequenceConstruct {
            element_type: _,
            length,
        } => {
            // Exact sequences materialize as canonical cells with one
            // 8-byte slot per element, exactly like record layouts.
            let dest = dest_local(layout, instruction)?;
            emit_alloc(body, dest, *length * 8)?;
            for index in 0..*length as usize {
                let operand = operand_local(layout, instruction, index)?;
                body.push(Instr::LocalGet(dest));
                emit_offset(body, (index * 8) as i32);
                body.push(Instr::LocalGet(operand));
                store_element_width(layout, instruction, index, body)?;
            }
        }
        SsaInstructionKind::Select { operand_type } => {
            let dest = dest_local(layout, instruction)?;
            let condition = operand_local(layout, instruction, 0)?;
            let when_true = operand_local(layout, instruction, 1)?;
            let when_false = operand_local(layout, instruction, 2)?;
            if let BodyType::Vector { element, lanes } = operand_type {
                let (lane_type, lane_bytes, integer) = vector_lane_realization(element)?;
                emit_alloc(
                    body,
                    dest,
                    lanes
                        .checked_mul(lane_bytes)
                        .ok_or_else(|| "vector allocation size overflow".to_owned())?,
                )?;
                for lane in 0..*lanes {
                    let offset = lane
                        .checked_mul(lane_bytes)
                        .ok_or_else(|| "vector lane offset overflow".to_owned())?
                        as i32;
                    body.push(Instr::LocalGet(dest));
                    emit_offset(body, offset);
                    body.push(Instr::LocalGet(when_true));
                    emit_offset(body, offset);
                    load_valtype(lane_type, body);
                    body.push(Instr::LocalGet(when_false));
                    emit_offset(body, offset);
                    load_valtype(lane_type, body);
                    body.push(Instr::LocalGet(condition));
                    body.push(Instr::I64Const(i64::from(lane)));
                    body.push(Instr::I64ShrU);
                    body.push(Instr::I32WrapI64);
                    body.push(Instr::I32Const(1));
                    body.push(Instr::I32And);
                    body.push(Instr::Select);
                    store_valtype(lane_type, body);
                }
                let _ = integer;
                return Ok(());
            }
            // WebAssembly `select` consumes true, false, condition and does
            // not introduce a candidate-dependent control-flow edge.
            body.push(Instr::LocalGet(when_true));
            body.push(Instr::LocalGet(when_false));
            body.push(Instr::LocalGet(condition));
            body.push(Instr::Select);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::SequenceReplace {
            bound, evidence, ..
        } => {
            let mncs_model::SequenceBound::Exact(length) = bound else {
                return Err("functional sequence update requires an exact bound".to_owned());
            };
            let dest = dest_local(layout, instruction)?;
            let source = operand_local(layout, instruction, 0)?;
            let index = operand_local(layout, instruction, 1)?;
            let element = operand_local(layout, instruction, 2)?;
            if matches!(evidence, mncs_model::BoundsEvidence::RuntimeChecked { .. }) {
                body.push(Instr::LocalGet(index));
                body.push(Instr::I64Const(i64::from(*length)));
                body.push(Instr::I64GeU);
                body.push(Instr::If);
                body.push(Instr::Unreachable);
                body.push(Instr::End);
            }
            // Allocate a fresh canonical cell and copy every fixed slot. The
            // source is immutable, so replacement preserves value semantics.
            emit_alloc(body, dest, *length * 8)?;
            let element_valtype = instruction
                .inputs
                .get(2)
                .and_then(|identity| layout.types.get(identity))
                .copied()
                .unwrap_or(ValType::I32);
            for lane in 0..*length {
                body.push(Instr::LocalGet(dest));
                emit_offset(body, (lane * 8) as i32);
                body.push(Instr::LocalGet(source));
                emit_offset(body, (lane * 8) as i32);
                match element_valtype {
                    ValType::I32 => body.push(Instr::I32Load),
                    ValType::I64 => body.push(Instr::I64Load),
                }
                store_element_width(layout, instruction, 2, body)?;
            }
            body.push(Instr::LocalGet(dest));
            body.push(Instr::LocalGet(index));
            body.push(Instr::I64Const(3));
            body.push(Instr::I64Shl);
            body.push(Instr::I32WrapI64);
            body.push(Instr::I32Add);
            body.push(Instr::LocalGet(element));
            store_element_width(layout, instruction, 2, body)?;
        }
        SsaInstructionKind::VectorConstruct {
            element_type,
            lanes,
        } => {
            let dest = dest_local(layout, instruction)?;
            let (lane_type, lane_bytes, _) = vector_lane_realization(element_type)?;
            if instruction.inputs.len() != *lanes as usize {
                return Err(
                    "vector construction operand count does not match lane count".to_owned(),
                );
            }
            emit_alloc(
                body,
                dest,
                lanes
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector allocation size overflow".to_owned())?,
            )?;
            for lane in 0..*lanes {
                body.push(Instr::LocalGet(dest));
                emit_offset(
                    body,
                    lane.checked_mul(lane_bytes)
                        .ok_or_else(|| "vector lane offset overflow".to_owned())?
                        as i32,
                );
                body.push(Instr::LocalGet(operand_local(
                    layout,
                    instruction,
                    lane as usize,
                )?));
                store_valtype(lane_type, body);
            }
        }
        SsaInstructionKind::VectorSplat {
            element_type,
            lanes,
        } => {
            let dest = dest_local(layout, instruction)?;
            let value = operand_local(layout, instruction, 0)?;
            let (lane_type, lane_bytes, _) = vector_lane_realization(element_type)?;
            emit_alloc(
                body,
                dest,
                lanes
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector allocation size overflow".to_owned())?,
            )?;
            for lane in 0..*lanes {
                body.push(Instr::LocalGet(dest));
                emit_offset(
                    body,
                    lane.checked_mul(lane_bytes)
                        .ok_or_else(|| "vector lane offset overflow".to_owned())?
                        as i32,
                );
                body.push(Instr::LocalGet(value));
                store_valtype(lane_type, body);
            }
        }
        SsaInstructionKind::VectorExtract {
            element_type,
            lanes,
            evidence,
        } => {
            let dest = dest_local(layout, instruction)?;
            let vector = operand_local(layout, instruction, 0)?;
            let index = operand_local(layout, instruction, 1)?;
            let (lane_type, lane_bytes, _) = vector_lane_realization(element_type)?;
            emit_vector_lane_check(body, index, *lanes, evidence);
            body.push(Instr::LocalGet(vector));
            body.push(Instr::LocalGet(index));
            body.push(Instr::I64Const(i64::from(lane_bytes)));
            body.push(Instr::I64Mul);
            body.push(Instr::I32WrapI64);
            body.push(Instr::I32Add);
            load_valtype(lane_type, body);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::VectorReplace {
            element_type,
            lanes,
            evidence,
        } => {
            let dest = dest_local(layout, instruction)?;
            let source = operand_local(layout, instruction, 0)?;
            let index = operand_local(layout, instruction, 1)?;
            let element = operand_local(layout, instruction, 2)?;
            let (lane_type, lane_bytes, _) = vector_lane_realization(element_type)?;
            emit_vector_lane_check(body, index, *lanes, evidence);
            emit_alloc(
                body,
                dest,
                lanes
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector allocation size overflow".to_owned())?,
            )?;
            for lane in 0..*lanes {
                let offset = lane
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector lane offset overflow".to_owned())?
                    as i32;
                body.push(Instr::LocalGet(dest));
                emit_offset(body, offset);
                body.push(Instr::LocalGet(source));
                emit_offset(body, offset);
                load_valtype(lane_type, body);
                store_valtype(lane_type, body);
            }
            body.push(Instr::LocalGet(dest));
            body.push(Instr::LocalGet(index));
            body.push(Instr::I64Const(i64::from(lane_bytes)));
            body.push(Instr::I64Mul);
            body.push(Instr::I32WrapI64);
            body.push(Instr::I32Add);
            body.push(Instr::LocalGet(element));
            store_valtype(lane_type, body);
        }
        SsaInstructionKind::VectorBinary {
            operator,
            element_type,
            lanes,
            intent,
        } => {
            if !matches!(intent, ArithmeticIntent::Wrapping) {
                return Err(format!("portable WASM scalar vector realization does not yet support {intent:?} lane arithmetic"));
            }
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            let (lane_type, lane_bytes, integer) = vector_lane_realization(element_type)?;
            emit_alloc(
                body,
                dest,
                lanes
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector allocation size overflow".to_owned())?,
            )?;
            for lane in 0..*lanes {
                let offset = lane
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector lane offset overflow".to_owned())?
                    as i32;
                body.push(Instr::LocalGet(dest));
                emit_offset(body, offset);
                emit_vector_binary_lane(body, lane_type, integer, operator, left, right, offset)?;
                store_valtype(lane_type, body);
            }
        }
        SsaInstructionKind::VectorCompare {
            predicate,
            element_type,
            lanes,
        } => {
            let dest = dest_local(layout, instruction)?;
            let left = operand_local(layout, instruction, 0)?;
            let right = operand_local(layout, instruction, 1)?;
            let (lane_type, lane_bytes, integer) = vector_lane_realization(element_type)?;
            body.push(Instr::I64Const(0));
            body.push(Instr::LocalSet(dest));
            for lane in 0..*lanes {
                let offset = lane
                    .checked_mul(lane_bytes)
                    .ok_or_else(|| "vector lane offset overflow".to_owned())?
                    as i32;
                body.push(Instr::LocalGet(dest));
                body.push(Instr::LocalGet(left));
                emit_offset(body, offset);
                load_valtype(lane_type, body);
                body.push(Instr::LocalGet(right));
                emit_offset(body, offset);
                load_valtype(lane_type, body);
                body.push(compare_instr(predicate, integer)?);
                body.push(Instr::I64ExtendI32U);
                if lane != 0 {
                    body.push(Instr::I64Const(i64::from(lane)));
                    body.push(Instr::I64Shl);
                }
                body.push(Instr::I64Or);
                body.push(Instr::LocalSet(dest));
            }
        }
        SsaInstructionKind::MaskBinary { operator, lanes } => {
            let dest = dest_local(layout, instruction)?;
            body.push(Instr::LocalGet(operand_local(layout, instruction, 0)?));
            body.push(Instr::LocalGet(operand_local(layout, instruction, 1)?));
            body.push(match operator.as_str() {
                "and" => Instr::I64And,
                "or" => Instr::I64Or,
                "xor" => Instr::I64Xor,
                other => return Err(format!("unsupported mask operator {other}")),
            });
            emit_mask_limit(body, *lanes);
            body.push(Instr::I64And);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::MaskNot { lanes } => {
            let dest = dest_local(layout, instruction)?;
            body.push(Instr::LocalGet(operand_local(layout, instruction, 0)?));
            emit_mask_limit(body, *lanes);
            body.push(Instr::I64Xor);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::MaskReduce { operator, lanes } => {
            let dest = dest_local(layout, instruction)?;
            let value = operand_local(layout, instruction, 0)?;
            body.push(Instr::LocalGet(value));
            match operator.as_str() {
                "any" => body.push(Instr::I64Eqz),
                "none" => body.push(Instr::I64Eqz),
                "all" => {
                    emit_mask_limit(body, *lanes);
                    body.push(Instr::I64Eq);
                }
                other => return Err(format!("unsupported mask reduction {other}")),
            }
            if operator == "any" {
                body.push(Instr::I32Eqz);
            }
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::VectorReduce {
            operator,
            element_type,
            lanes,
            intent,
        } => {
            if *lanes == 0 {
                return Err("vector reduction requires at least one lane".to_owned());
            }
            if !matches!(intent, ArithmeticIntent::Wrapping) {
                return Err(format!(
                    "portable WASM scalar vector reduction does not yet support {intent:?}"
                ));
            }
            let dest = dest_local(layout, instruction)?;
            let vector = operand_local(layout, instruction, 0)?;
            let (lane_type, lane_bytes, integer) = vector_lane_realization(element_type)?;
            let seed = match operator.as_str() {
                "sum" => 0,
                "min" if integer.signed && integer.bits == 32 => i64::from(i32::MAX),
                "min" if integer.signed => i64::MAX,
                "min" if integer.bits == 32 => i64::from(u32::MAX),
                "min" => -1,
                "max" if integer.signed && integer.bits == 32 => i64::from(i32::MIN),
                "max" if integer.signed => i64::MIN,
                "max" => 0,
                other => return Err(format!("unsupported vector reduction {other}")),
            };
            body.push(match lane_type {
                ValType::I32 => Instr::I32Const(seed as i32),
                ValType::I64 => Instr::I64Const(seed),
            });
            body.push(Instr::LocalSet(dest));
            for lane in 0..*lanes {
                emit_vector_reduce_lane(
                    body,
                    dest,
                    vector,
                    (lane * lane_bytes) as i32,
                    lane_type,
                    integer,
                    operator,
                )?;
            }
        }
        SsaInstructionKind::SequenceProject { bound, evidence } => {
            let dest = dest_local(layout, instruction)?;
            let seq = operand_local(layout, instruction, 0)?;
            let index = operand_local(layout, instruction, 1)?;
            let checked = matches!(evidence, mncs_model::BoundsEvidence::RuntimeChecked { .. });
            match bound {
                mncs_model::SequenceBound::Exact(length) => {
                    if checked {
                        // Out-of-bounds indexes trap exactly like checked
                        // arithmetic edges.
                        body.push(Instr::LocalGet(index));
                        body.push(Instr::I64Const(i64::from(*length)));
                        body.push(Instr::I64GeU);
                        body.push(Instr::If);
                        body.push(Instr::Unreachable);
                        body.push(Instr::End);
                    }
                    body.push(Instr::LocalGet(seq));
                    body.push(Instr::LocalGet(index));
                    body.push(Instr::I64Const(3));
                    body.push(Instr::I64Shl);
                    body.push(Instr::I32WrapI64);
                    body.push(Instr::I32Add);
                    load_element_width(layout, instruction, body)?;
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    if checked {
                        body.push(Instr::LocalGet(index));
                        body.push(Instr::LocalGet(seq));
                        body.push(Instr::I64Const(32));
                        body.push(Instr::I64ShrU);
                        body.push(Instr::I64GeU);
                        body.push(Instr::If);
                        body.push(Instr::Unreachable);
                        body.push(Instr::End);
                    }
                    if matches!(&instruction.outputs[0].ty, IrType::Named(name) if name == "byte") {
                        // Bit 0 marks a byte view whose source is still an
                        // exact canonical-cell sequence. Host-packed byte
                        // views leave it clear. Mask the marker before the
                        // load and preserve the source representation's
                        // stride for the index.
                        body.push(Instr::LocalGet(seq));
                        body.push(Instr::I64Const(4_294_967_294));
                        body.push(Instr::I64And);
                        body.push(Instr::I32WrapI64);
                        body.push(Instr::LocalGet(index));
                        body.push(Instr::I64Const(3));
                        body.push(Instr::I64Shl);
                        body.push(Instr::LocalGet(index));
                        body.push(Instr::LocalGet(seq));
                        body.push(Instr::I64Const(1));
                        body.push(Instr::I64And);
                        body.push(Instr::I32WrapI64);
                        body.push(Instr::Select);
                        body.push(Instr::I32WrapI64);
                    } else {
                        body.push(Instr::LocalGet(seq));
                        body.push(Instr::I32WrapI64);
                        body.push(Instr::LocalGet(index));
                        body.push(Instr::I64Const(3));
                        body.push(Instr::I64Shl);
                        body.push(Instr::I32WrapI64);
                    }
                    body.push(Instr::I32Add);
                    load_element_width(layout, instruction, body)?;
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            }
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::SequenceLength { bound } => {
            let dest = dest_local(layout, instruction)?;
            let seq = operand_local(layout, instruction, 0)?;
            match bound {
                mncs_model::SequenceBound::Exact(length) => {
                    body.push(Instr::I64Const(i64::from(*length)));
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    body.push(Instr::LocalGet(seq));
                    body.push(Instr::I64Const(32));
                    body.push(Instr::I64ShrU);
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            }
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::ViewConstruct {
            source_bound,
            view_bound,
        } => {
            let mncs_model::SequenceBound::UpTo(cap) = view_bound else {
                return Err("view construction must produce an UpTo view".to_owned());
            };
            let dest = dest_local(layout, instruction)?;
            let seq = operand_local(layout, instruction, 0)?;
            let start = operand_local(layout, instruction, 1)?;
            let end = operand_local(layout, instruction, 2)?;
            // Source length: static for exact sequences, packed for views.
            let source_len = |body: &mut Vec<Instr>| match source_bound {
                mncs_model::SequenceBound::Exact(length) => {
                    body.push(Instr::I64Const(i64::from(*length)));
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    body.push(Instr::LocalGet(seq));
                    body.push(Instr::I64Const(32));
                    body.push(Instr::I64ShrU);
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            };
            // start > end -> trap
            body.push(Instr::LocalGet(start));
            body.push(Instr::LocalGet(end));
            body.push(Instr::I64GtU);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
            // end > len(source) -> trap
            body.push(Instr::LocalGet(end));
            source_len(body);
            body.push(Instr::I64GtU);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
            // span > cap -> trap
            body.push(Instr::LocalGet(end));
            body.push(Instr::LocalGet(start));
            body.push(Instr::I64Sub);
            body.push(Instr::I64Const(i64::from(*cap)));
            body.push(Instr::I64GtU);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
            // Exact sequences use canonical eight-byte cells; an UpTo source
            // already carries a packed view descriptor whose low half is the
            // byte address of its element representation. Preserve that
            // representation when applying the slice start.
            let byte_view = matches!(&instruction.outputs[0].ty, IrType::Named(name) if name.contains("[byte;"));
            match source_bound {
                mncs_model::SequenceBound::Exact(_) => {
                    body.push(Instr::LocalGet(seq));
                    body.push(Instr::I64ExtendI32U);
                    body.push(Instr::LocalGet(start));
                    body.push(Instr::I64Const(3));
                    body.push(Instr::I64Shl);
                }
                mncs_model::SequenceBound::UpTo(_) => {
                    body.push(Instr::LocalGet(seq));
                    body.push(Instr::I64Const(if byte_view {
                        4_294_967_294
                    } else {
                        4_294_967_295
                    }));
                    body.push(Instr::I64And);
                    if byte_view {
                        // Select start*8 for an exact-cell byte view (marker
                        // set) and start for a packed host byte view.
                        body.push(Instr::LocalGet(start));
                        body.push(Instr::I64Const(3));
                        body.push(Instr::I64Shl);
                        body.push(Instr::LocalGet(start));
                        body.push(Instr::LocalGet(seq));
                        body.push(Instr::I64Const(1));
                        body.push(Instr::I64And);
                        body.push(Instr::I32WrapI64);
                        body.push(Instr::Select);
                    } else {
                        body.push(Instr::LocalGet(start));
                        body.push(Instr::I64Const(3));
                        body.push(Instr::I64Shl);
                    }
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            }
            body.push(Instr::I64Add);
            if byte_view {
                match source_bound {
                    mncs_model::SequenceBound::Exact(_) => {
                        body.push(Instr::I64Const(1));
                    }
                    mncs_model::SequenceBound::UpTo(_) => {
                        body.push(Instr::LocalGet(seq));
                        body.push(Instr::I64Const(1));
                        body.push(Instr::I64And);
                    }
                    mncs_model::SequenceBound::Param(_)
                    | mncs_model::SequenceBound::UpToParam(_) => {
                        unreachable!(
                            "generic SequenceBound must be specialized before backend lowering"
                        )
                    }
                }
                body.push(Instr::I64Or);
            }
            // Keep only the low 32 bits of the address half.
            body.push(Instr::I64Const(4_294_967_295));
            body.push(Instr::I64And);
            body.push(Instr::LocalGet(end));
            body.push(Instr::LocalGet(start));
            body.push(Instr::I64Sub);
            body.push(Instr::I64Const(32));
            body.push(Instr::I64Shl);
            body.push(Instr::I64Or);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::FinitePayloadProject {
            type_identity,
            discriminant,
            field,
            ..
        } => {
            let Some(variants) = composites.boxed_finites.get(type_identity) else {
                return Err(
                    "payload projection requires the payload-bearing realization".to_owned(),
                );
            };
            let fields = variants.get(discriminant).ok_or_else(|| {
                format!("finite variant {discriminant} has no declared payload layout")
            })?;
            let index = fields
                .iter()
                .position(|(name, _)| name == field)
                .ok_or_else(|| format!("variant payload has no field {field:?}"))?;
            let dest = dest_local(layout, instruction)?;
            let value = operand_local(layout, instruction, 0)?;
            body.push(Instr::LocalGet(value));
            emit_offset(body, ((index + 1) * 8) as i32);
            load_instr(fields[index].1, body);
            body.push(Instr::LocalSet(dest));
        }
        SsaInstructionKind::FiniteIsVariant {
            type_identity,
            discriminant,
            ..
        } => {
            let dest = dest_local(layout, instruction)?;
            let value = operand_local(layout, instruction, 0)?;
            if composites.is_boxed_finite(type_identity) {
                body.push(Instr::LocalGet(value));
                body.push(Instr::I32Load);
                body.push(Instr::I32Const(*discriminant as i32));
            } else {
                body.push(Instr::LocalGet(value));
                body.push(Instr::I32Const(*discriminant as i32));
            }
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
        SsaInstructionKind::RecordConstruct {
            type_identity,
            field_names,
            ..
        } => {
            let Some(fields) = composites.records.get(type_identity) else {
                return Err(format!(
                    "record construction names an unknown record layout {type_identity}"
                ));
            };
            if fields.len() != field_names.len() || fields.len() != instruction.inputs.len() {
                return Err("record construction does not match its declared layout".to_owned());
            }
            let dest = dest_local(layout, instruction)?;
            emit_alloc(body, dest, (fields.len() as u32) * 8)?;
            for (index, (name, width)) in fields.iter().enumerate() {
                let declared = &field_names[index];
                if name != declared {
                    return Err(format!(
                        "record construction field {declared:?} does not match the canonical layout {name:?}"
                    ));
                }
                let operand = operand_local(layout, instruction, index)?;
                body.push(Instr::LocalGet(dest));
                emit_offset(body, (index * 8) as i32);
                body.push(Instr::LocalGet(operand));
                store_width(*width, body);
            }
        }
        SsaInstructionKind::RecordProject {
            type_identity,
            field,
            ..
        } => {
            let Some(fields) = composites.records.get(type_identity) else {
                return Err(format!(
                    "record projection names an unknown record layout {type_identity}"
                ));
            };
            let index = fields
                .iter()
                .position(|(name, _)| name == field)
                .ok_or_else(|| format!("record layout has no field {field:?}"))?;
            let dest = dest_local(layout, instruction)?;
            let value = operand_local(layout, instruction, 0)?;
            body.push(Instr::LocalGet(value));
            emit_offset(body, (index * 8) as i32);
            load_instr(fields[index].1, body);
            body.push(Instr::LocalSet(dest));
        }
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
    let (wasm_type, integer) = wasm_type(ty)?;
    match wasm_type {
        ValType::I32 => {
            let encoded = match integer {
                Some(integer) if !integer.signed => {
                    if value < 0 || value >= (1_i128 << integer.bits) {
                        return Err("unsigned constant is outside its declared width".to_owned());
                    }
                    value as u32 as i32
                }
                _ => i32::try_from(value)
                    .map_err(|_| "constant does not fit in wasm i32".to_owned())?,
            };
            body.push(Instr::I32Const(encoded));
        }
        ValType::I64 => {
            let encoded = match integer {
                Some(integer) if !integer.signed => {
                    if value < 0 || value >= (1_i128 << integer.bits) {
                        return Err("unsigned constant is outside its declared width".to_owned());
                    }
                    value as u64 as i64
                }
                _ => i64::try_from(value)
                    .map_err(|_| "constant does not fit in wasm i64".to_owned())?,
            };
            body.push(Instr::I64Const(encoded));
        }
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
    let unsigned = !operand_type.signed;
    let wrapping = match (wasm, operator, unsigned) {
        (ValType::I32, "add", _) => Instr::I32Add,
        (ValType::I32, "sub", _) => Instr::I32Sub,
        (ValType::I32, "mul", _) => Instr::I32Mul,
        (ValType::I32, "div", false) => Instr::I32DivS,
        (ValType::I32, "div", true) => Instr::I32DivU,
        (ValType::I32, "mod", false) => Instr::I32RemS,
        (ValType::I32, "mod", true) => Instr::I32RemU,
        (ValType::I32, "and", _) => Instr::I32And,
        (ValType::I32, "or", _) => Instr::I32Or,
        (ValType::I32, "xor", _) => Instr::I32Xor,
        // Total shift semantics: counts are modulo the declared width and
        // signed shr is arithmetic.
        (ValType::I32, "shl", _) => Instr::I32Shl,
        (ValType::I32, "shr", true) => Instr::I32ShrS,
        (ValType::I32, "shr", false) => Instr::I32ShrU,
        (ValType::I64, "shl", _) => Instr::I64Shl,
        (ValType::I64, "shr", true) => Instr::I64ShrS,
        (ValType::I64, "shr", false) => Instr::I64ShrU,
        (ValType::I64, "add", _) => Instr::I64Add,
        (ValType::I64, "sub", _) => Instr::I64Sub,
        (ValType::I64, "mul", _) => Instr::I64Mul,
        (ValType::I64, "div", false) => Instr::I64DivS,
        (ValType::I64, "div", true) => Instr::I64DivU,
        (ValType::I64, "mod", false) => Instr::I64RemS,
        (ValType::I64, "mod", true) => Instr::I64RemU,
        (ValType::I64, "and", _) => Instr::I64And,
        (ValType::I64, "or", _) => Instr::I64Or,
        (ValType::I64, "xor", _) => Instr::I64Xor,
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
            // Narrow results are normalized through an explicit local round
            // trip; a bare mask here would pop an empty operand stack.
            if result_type.bits < 32 && matches!(wasm, ValType::I32) {
                body.push(Instr::LocalGet(dest));
                mask_width(body, result_type, wasm);
                body.push(Instr::LocalSet(dest));
            }
        }
        ArithmeticIntent::Saturating => {
            if matches!(val_type(result_type)?, ValType::I64) {
                emit_saturating_wide(body, operator, operand_type, left, right, dest)?
            } else {
                emit_saturating(body, operator, operand_type, result_type, left, right, dest)?
            }
        }
        ArithmeticIntent::Widening { .. } => {
            emit_widening(body, operator, operand_type, result_type, left, right, dest)?
        }
    }
    Ok(())
}

/// Checked division/remainder: guard against division by zero (both) and
/// `MIN / -1` (division only), then use the native trapping operations.
/// Guards branch to an explicit trap so failure statuses match the reference
/// executors' runtime-failure semantics.
fn emit_checked_division(
    body: &mut Vec<Instr>,
    operator: &str,
    ty: IntegerType,
    wasm: ValType,
    left: u32,
    right: u32,
    dest: u32,
) -> Result<(), String> {
    let is_div = operator == "div";
    // MIN / -1 is a signed-only overflow case; unsigned division has no
    // such corner and must not trap on it.
    let signed = ty.signed;
    // if divisor == 0 { trap } — the zero test must match the operand's
    // carrier width; an i32 test on an i64 divisor misses high-word values.
    body.push(Instr::LocalGet(right));
    body.push(match wasm {
        ValType::I64 => Instr::I64Eqz,
        ValType::I32 => Instr::I32Eqz,
    });
    body.push(Instr::If);
    body.push(Instr::Unreachable);
    body.push(Instr::End);
    if is_div && signed {
        // if dividend == MIN && divisor == -1 { trap }
        let (min_const, minus_one): (Instr, Instr) = match wasm {
            ValType::I32 => (Instr::I32Const(i32::MIN), Instr::I32Const(-1)),
            ValType::I64 => (Instr::I64Const(i64::MIN), Instr::I64Const(-1)),
        };
        let eq: Instr = match wasm {
            ValType::I32 => Instr::I32Eq,
            ValType::I64 => Instr::I64Eq,
        };
        let eq2: Instr = match wasm {
            ValType::I32 => Instr::I32Eq,
            ValType::I64 => Instr::I64Eq,
        };
        body.push(Instr::LocalGet(right));
        body.push(minus_one);
        body.push(eq);
        body.push(Instr::If);
        {
            body.push(Instr::LocalGet(left));
            body.push(min_const);
            body.push(eq2);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
        }
        body.push(Instr::End);
    }
    body.push(Instr::LocalGet(left));
    body.push(Instr::LocalGet(right));
    // `signed` selects the signed-division instructions; unsigned operands
    // must use div.u/rem.u so their modular domain is preserved.
    let native = match (wasm, operator, signed) {
        (ValType::I32, "div", true) => Instr::I32DivS,
        (ValType::I32, "div", false) => Instr::I32DivU,
        (ValType::I32, "mod", true) => Instr::I32RemS,
        (ValType::I32, "mod", false) => Instr::I32RemU,
        (ValType::I64, "div", true) => Instr::I64DivS,
        (ValType::I64, "div", false) => Instr::I64DivU,
        (ValType::I64, "mod", true) => Instr::I64RemS,
        (ValType::I64, "mod", false) => Instr::I64RemU,
        _ => return Err(format!("unsupported division operator {operator}")),
    };
    body.push(native);
    body.push(Instr::LocalSet(dest));
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

/// 64-bit saturating arithmetic in i64 cells. The wrapping i64 result is
/// computed first, then overflow is detected exactly:
/// - signed add overflows when `~(l ^ r) & (l ^ w)` is negative;
/// - signed sub overflows when `(l ^ r) & (l ^ w)` is negative;
/// - unsigned add/sub compare the wrapped value against its inputs;
/// - signed mul guards the divisor (`l == 0` cannot overflow, `l == -1`
///   overflows only for `r == MIN`) before dividing, so no native trap is
///   reachable and failure statuses stay language-owned.
fn emit_saturating_wide(
    body: &mut Vec<Instr>,
    operator: &str,
    operand: IntegerType,
    left: u32,
    right: u32,
    dest: u32,
) -> Result<(), String> {
    if !matches!(operator, "add" | "sub" | "mul") {
        return Err("portable WASM saturating arithmetic realizes add, sub, and mul".to_owned());
    }
    let signed = operand.signed;
    let min = if signed { i64::MIN } else { 0_i64 };
    let max = if signed { i64::MAX } else { u64::MAX as i64 };
    body.push(Instr::LocalGet(left));
    body.push(Instr::LocalGet(right));
    match operator {
        "sub" => body.push(Instr::I64Sub),
        "mul" => body.push(Instr::I64Mul),
        _ => body.push(Instr::I64Add),
    }
    body.push(Instr::LocalSet(dest));

    let emit_overflow_condition = |body: &mut Vec<Instr>| {
        match (operator, signed) {
            ("add", true) => {
                // ~(l ^ r) & (l ^ w) < 0
                body.push(Instr::LocalGet(left));
                body.push(Instr::LocalGet(right));
                body.push(Instr::I64Xor);
                body.push(Instr::I64Const(-1));
                body.push(Instr::I64Xor);
                body.push(Instr::LocalGet(left));
                body.push(Instr::LocalGet(dest));
                body.push(Instr::I64Xor);
                body.push(Instr::I64And);
                body.push(Instr::I64Const(0));
                body.push(Instr::I64LtS);
            }
            ("sub", true) => {
                // (l ^ r) & (l ^ w) < 0
                body.push(Instr::LocalGet(left));
                body.push(Instr::LocalGet(right));
                body.push(Instr::I64Xor);
                body.push(Instr::LocalGet(left));
                body.push(Instr::LocalGet(dest));
                body.push(Instr::I64Xor);
                body.push(Instr::I64And);
                body.push(Instr::I64Const(0));
                body.push(Instr::I64LtS);
            }
            ("add", false) => {
                // w < l (unsigned wraparound)
                body.push(Instr::LocalGet(dest));
                body.push(Instr::LocalGet(left));
                body.push(Instr::I64LtU);
            }
            ("sub", false) => {
                // r > l (unsigned underflow)
                body.push(Instr::LocalGet(right));
                body.push(Instr::LocalGet(left));
                body.push(Instr::I64GtU);
            }
            _ => unreachable!("mul handled separately"),
        }
    };

    if operator == "mul" {
        if !signed {
            return Err("portable WASM realizes 64-bit saturating multiplication for signed operands; unsigned 64-bit saturating multiplication awaits a wide-multiply lowering".to_owned());
        }
        // l == 0: product cannot overflow.
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64Eqz);
        body.push(Instr::If);
        body.push(Instr::Else);
        // l == -1: overflow exactly when r == MIN.
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64Const(-1));
        body.push(Instr::I64Eq);
        body.push(Instr::If);
        body.push(Instr::LocalGet(right));
        body.push(Instr::I64Const(i64::MIN));
        body.push(Instr::I64Eq);
        body.push(Instr::If);
        // Same sign (both negative): upward saturation.
        body.push(Instr::I64Const(max));
        body.push(Instr::LocalSet(dest));
        body.push(Instr::End);
        body.push(Instr::Else);
        // Generic case: l ∉ {0, -1} so the division cannot trap;
        // overflow iff w / l != r. Saturation direction is downward when
        // the operand signs differ.
        body.push(Instr::LocalGet(dest));
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64DivS);
        body.push(Instr::LocalGet(right));
        body.push(Instr::I64Ne);
        body.push(Instr::If);
        body.push(Instr::I64Const(min));
        body.push(Instr::I64Const(max));
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64Const(63));
        body.push(Instr::I64ShrS);
        body.push(Instr::I64Const(0));
        body.push(Instr::I64Ne);
        body.push(Instr::LocalGet(right));
        body.push(Instr::I64Const(63));
        body.push(Instr::I64ShrS);
        body.push(Instr::I64Const(0));
        body.push(Instr::I64Ne);
        body.push(Instr::I32Xor);
        body.push(Instr::Select);
        body.push(Instr::LocalSet(dest));
        body.push(Instr::End);
        body.push(Instr::End);
        body.push(Instr::End);
        return Ok(());
    }

    emit_overflow_condition(body);
    body.push(Instr::If);
    if signed {
        // Direction: add saturates downward when l is negative; sub
        // saturates downward when r is positive. Select picks the first
        // value when its condition is nonzero.
        let direction_source = if operator == "add" { left } else { right };
        let downward_when_negative = operator == "add";
        body.push(Instr::I64Const(min));
        body.push(Instr::I64Const(max));
        body.push(Instr::LocalGet(direction_source));
        body.push(Instr::I64Const(63));
        body.push(Instr::I64ShrS);
        body.push(Instr::I64Const(0));
        body.push(Instr::I64Ne);
        if !downward_when_negative {
            // For sub, positive r means downward saturation: invert.
            body.push(Instr::I32Eqz);
        }
        body.push(Instr::Select);
    } else {
        body.push(Instr::I64Const(if operator == "add" { max } else { min }));
    }
    body.push(Instr::LocalSet(dest));
    body.push(Instr::End);
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
        return Err("portable WASM saturating arithmetic requires signed 1..=32-bit or unsigned 1..=31-bit add/sub/mul in i32 cells".to_owned());
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
    if matches!(operator, "div" | "mod") {
        emit_checked_division(body, operator, ty, wasm, left, right, dest)?;
        return Ok(());
    }
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
        ValType::I64 => emit_unsigned_i64_overflow_trap(body, left, right, dest, operator),
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
    if operator == "mul" {
        // For l == 0 the product cannot overflow. For l == -1 the only
        // overflowing product is MIN * -1; all other nonzero divisors are
        // safe for the quotient test because MIN / -1 is the sole trapping
        // signed i64 division case.
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64Eqz);
        body.push(Instr::If);
        body.push(Instr::Else);
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64Const(-1));
        body.push(Instr::I64Eq);
        body.push(Instr::If);
        body.push(Instr::LocalGet(right));
        body.push(Instr::I64Const(i64::MIN));
        body.push(Instr::I64Eq);
        body.push(Instr::If);
        body.push(Instr::Unreachable);
        body.push(Instr::End);
        body.push(Instr::Else);
        body.push(Instr::LocalGet(dest));
        body.push(Instr::LocalGet(left));
        body.push(Instr::I64DivS);
        body.push(Instr::LocalGet(right));
        body.push(Instr::I64Ne);
        body.push(Instr::If);
        body.push(Instr::Unreachable);
        body.push(Instr::End);
        body.push(Instr::End);
        body.push(Instr::End);
        return Ok(());
    }
    if operator != "add" && operator != "sub" {
        return Err(format!("checked i64 operator {operator} is unsupported"));
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
    right: u32,
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
        "mul" => {
            // A zero multiplier cannot overflow. Otherwise, the wrapped
            // product divided by the right operand must recover the left
            // operand; unsigned division is total for every nonzero divisor.
            body.push(Instr::LocalGet(right));
            body.push(Instr::I64Eqz);
            body.push(Instr::If);
            body.push(Instr::Else);
            body.push(Instr::LocalGet(dest));
            body.push(Instr::LocalGet(right));
            body.push(Instr::I64DivU);
            body.push(Instr::LocalGet(left));
            body.push(Instr::I64Ne);
            body.push(Instr::If);
            body.push(Instr::Unreachable);
            body.push(Instr::End);
            body.push(Instr::End);
        }
        _ => {
            return Err(format!(
                "checked unsigned i64 operator {operator} is unsupported"
            ))
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

/// Emit `$mncs_alloc(bytes) -> ptr` into every module that materializes
/// composites, plus the bump-pointer global it uses (global 0).
const ALLOC_FUNCTION_NAME: &str = "mncs_alloc";
/// Export a stable byte-buffer reservation ABI alongside the allocator.
/// The returned i64 packs the byte capacity in the high half and the linear
/// memory offset in the low half.
const HOST_BUFFER_FUNCTION_NAME: &str = "mncs_host_buffer";
const HOST_BUFFER_RESET_FUNCTION_NAME: &str = "mncs_host_buffer_reset";

pub fn emit_alloc_helpers(module: &mut WasmModule) {
    if !module.globals.is_empty() && module.memory.is_some() {
        // Global 0 is the allocator cursor. Keep the end of the host-owned
        // region in a second cursor so hosts can recycle target-array
        // allocations between calls without overwriting their input bytes.
        module.globals.push(crate::wasm::WasmGlobal {
            valtype: ValType::I32,
            mutable: true,
            init: 8,
        });
        let host_end_global = (module.globals.len() - 1) as u32;
        // $mncs_alloc(bytes): bump the global pointer, keep 8-byte alignment,
        // return the previous pointer. Appending keeps every existing callee
        // index stable.
        module.functions.push(WasmFunction {
            name: ALLOC_FUNCTION_NAME.to_owned(),
            params: vec![ValType::I32],
            results: vec![ValType::I32],
            locals: vec![ValType::I32],
            body: vec![
                Instr::GlobalGet(0),
                Instr::LocalSet(1),
                Instr::GlobalGet(0),
                Instr::LocalGet(0),
                // round the request up to an 8-byte multiple
                Instr::I32Const(7),
                Instr::I32Add,
                Instr::I32Const(-8),
                Instr::I32And,
                Instr::I32Add,
                Instr::GlobalSet(0),
                Instr::LocalGet(1),
            ],
        });
        let alloc_index = (module.functions.len() - 1) as u32;
        for function in &mut module.functions {
            rewrite_alloc_calls(function, alloc_index);
        }
        module.functions.push(WasmFunction {
            name: HOST_BUFFER_FUNCTION_NAME.to_owned(),
            params: vec![ValType::I32],
            results: vec![ValType::I64],
            locals: vec![ValType::I32],
            body: vec![
                // Reserve the host-owned region before any later composite
                // allocations and return {capacity:32, offset:32}.
                Instr::LocalGet(0),
                Instr::Call(alloc_index),
                Instr::LocalSet(1),
                Instr::GlobalGet(0),
                Instr::GlobalSet(host_end_global),
                Instr::LocalGet(1),
                Instr::I64ExtendI32U,
                Instr::LocalGet(0),
                Instr::I64ExtendI32U,
                Instr::I64Const(32),
                Instr::I64Shl,
                Instr::I64Or,
            ],
        });
        module.functions.push(WasmFunction {
            name: HOST_BUFFER_RESET_FUNCTION_NAME.to_owned(),
            params: Vec::new(),
            results: Vec::new(),
            locals: Vec::new(),
            body: vec![Instr::GlobalGet(host_end_global), Instr::GlobalSet(0)],
        });
    }
    // Any leftover marker means a lowered function asked for allocation
    // without composites being declared; fail loudly rather than encode it.
    for function in &module.functions {
        if function
            .body
            .iter()
            .any(|instr| matches!(instr, crate::wasm::Instr::AllocCall))
        {
            panic!("AllocCall marker survived lowering without arena helpers");
        }
    }
}

/// Rewrite `Call(placeholder)` markers emitted during lowering into the real
/// helper index once it is known.
fn rewrite_alloc_calls(function: &mut WasmFunction, alloc_index: u32) {
    for instr in &mut function.body {
        if matches!(instr, crate::wasm::Instr::AllocCall) {
            *instr = crate::wasm::Instr::Call(alloc_index);
        }
    }
}

/// Emit a call to `$mncs_alloc` with a constant byte count; result lands in
/// `dest`. The call is emitted as an `AllocCall` marker and rewritten when the
/// helper index is assigned at module level.
fn emit_alloc(body: &mut Vec<Instr>, dest_local: u32, bytes: u32) -> Result<(), String> {
    body.push(Instr::I32Const(bytes as i32));
    body.push(Instr::AllocCall);
    body.push(Instr::LocalSet(dest_local));
    Ok(())
}

/// Address arithmetic: push base + offset (offset may be negative-free here;
/// composite slot offsets are always multiples of 8).
fn emit_offset(body: &mut Vec<Instr>, offset: i32) {
    if offset != 0 {
        body.push(Instr::I32Const(offset));
        body.push(Instr::I32Add);
    }
}

fn load_instr(width: SlotWidth, body: &mut Vec<Instr>) {
    match width {
        SlotWidth::W32 => body.push(Instr::I32Load),
        SlotWidth::W64 => body.push(Instr::I64Load),
    }
}

fn vector_lane_realization(element: &BodyType) -> Result<(ValType, u32, IntegerType), String> {
    let BodyType::Integer(integer) = element else {
        return Err("initial vector realization supports integer lanes only".to_owned());
    };
    match integer.bits {
        32 => Ok((ValType::I32, 4, *integer)),
        64 => Ok((ValType::I64, 8, *integer)),
        bits => Err(format!(
            "portable WASM vector realization does not yet support {bits}-bit lanes"
        )),
    }
}

fn load_valtype(ty: ValType, body: &mut Vec<Instr>) {
    match ty {
        ValType::I32 => body.push(Instr::I32Load),
        ValType::I64 => body.push(Instr::I64Load),
    }
}

fn store_valtype(ty: ValType, body: &mut Vec<Instr>) {
    match ty {
        ValType::I32 => body.push(Instr::I32Store),
        ValType::I64 => body.push(Instr::I64Store),
    }
}

fn emit_vector_lane_check(
    body: &mut Vec<Instr>,
    index: u32,
    lanes: u32,
    evidence: &mncs_model::BoundsEvidence,
) {
    if matches!(evidence, mncs_model::BoundsEvidence::RuntimeChecked { .. }) {
        body.push(Instr::LocalGet(index));
        body.push(Instr::I64Const(i64::from(lanes)));
        body.push(Instr::I64GeU);
        body.push(Instr::If);
        body.push(Instr::Unreachable);
        body.push(Instr::End);
    }
}

fn emit_mask_limit(body: &mut Vec<Instr>, lanes: u32) {
    let mask = if lanes >= 64 {
        -1_i64
    } else {
        ((1_u64 << lanes) - 1) as i64
    };
    body.push(Instr::I64Const(mask));
}

fn emit_vector_binary_lane(
    body: &mut Vec<Instr>,
    lane_type: ValType,
    integer: IntegerType,
    operator: &str,
    left: u32,
    right: u32,
    offset: i32,
) -> Result<(), String> {
    let load = |body: &mut Vec<Instr>, local| {
        body.push(Instr::LocalGet(local));
        emit_offset(body, offset);
        load_valtype(lane_type, body);
    };
    if matches!(operator, "min" | "max") {
        // b ^ ((a ^ b) & -(a cmp b)); branchless min/max over signed or
        // unsigned lanes, where cmp is lt for min and gt for max.
        load(body, right);
        load(body, left);
        load(body, right);
        body.push(match lane_type {
            ValType::I32 => Instr::I32Xor,
            ValType::I64 => Instr::I64Xor,
        });
        body.push(match lane_type {
            ValType::I32 => Instr::I32Const(0),
            ValType::I64 => Instr::I64Const(0),
        });
        load(body, left);
        load(body, right);
        body.push(compare_instr(
            if operator == "min" { "lt" } else { "gt" },
            integer,
        )?);
        if lane_type == ValType::I64 {
            body.push(Instr::I64ExtendI32U);
        }
        body.push(match lane_type {
            ValType::I32 => Instr::I32Sub,
            ValType::I64 => Instr::I64Sub,
        });
        body.push(match lane_type {
            ValType::I32 => Instr::I32And,
            ValType::I64 => Instr::I64And,
        });
        body.push(match lane_type {
            ValType::I32 => Instr::I32Xor,
            ValType::I64 => Instr::I64Xor,
        });
        return Ok(());
    }
    load(body, left);
    load(body, right);
    body.push(match (lane_type, operator, integer.signed) {
        (ValType::I32, "add", _) => Instr::I32Add,
        (ValType::I32, "sub", _) => Instr::I32Sub,
        (ValType::I32, "mul", _) => Instr::I32Mul,
        (ValType::I32, "and", _) => Instr::I32And,
        (ValType::I32, "or", _) => Instr::I32Or,
        (ValType::I32, "xor", _) => Instr::I32Xor,
        (ValType::I32, "shl", _) => Instr::I32Shl,
        (ValType::I32, "shr", true) => Instr::I32ShrS,
        (ValType::I32, "shr", false) => Instr::I32ShrU,
        (ValType::I64, "add", _) => Instr::I64Add,
        (ValType::I64, "sub", _) => Instr::I64Sub,
        (ValType::I64, "mul", _) => Instr::I64Mul,
        (ValType::I64, "and", _) => Instr::I64And,
        (ValType::I64, "or", _) => Instr::I64Or,
        (ValType::I64, "xor", _) => Instr::I64Xor,
        (ValType::I64, "shl", _) => Instr::I64Shl,
        (ValType::I64, "shr", true) => Instr::I64ShrS,
        (ValType::I64, "shr", false) => Instr::I64ShrU,
        (_, other, _) => return Err(format!("unsupported vector operator {other}")),
    });
    Ok(())
}

fn emit_vector_reduce_lane(
    body: &mut Vec<Instr>,
    dest: u32,
    vector: u32,
    offset: i32,
    lane_type: ValType,
    integer: IntegerType,
    operator: &str,
) -> Result<(), String> {
    let lane = |body: &mut Vec<Instr>| {
        body.push(Instr::LocalGet(vector));
        emit_offset(body, offset);
        load_valtype(lane_type, body);
    };
    if matches!(operator, "min" | "max") {
        // Same branchless blend as lane-wise min/max: b ^ ((a ^ b) & -cmp).
        lane(body);
        body.push(Instr::LocalGet(dest));
        lane(body);
        body.push(match lane_type {
            ValType::I32 => Instr::I32Xor,
            ValType::I64 => Instr::I64Xor,
        });
        body.push(match lane_type {
            ValType::I32 => Instr::I32Const(0),
            ValType::I64 => Instr::I64Const(0),
        });
        body.push(Instr::LocalGet(dest));
        lane(body);
        body.push(compare_instr(
            if operator == "min" { "lt" } else { "gt" },
            integer,
        )?);
        if lane_type == ValType::I64 {
            body.push(Instr::I64ExtendI32U);
        }
        body.push(match lane_type {
            ValType::I32 => Instr::I32Sub,
            ValType::I64 => Instr::I64Sub,
        });
        body.push(match lane_type {
            ValType::I32 => Instr::I32And,
            ValType::I64 => Instr::I64And,
        });
        body.push(match lane_type {
            ValType::I32 => Instr::I32Xor,
            ValType::I64 => Instr::I64Xor,
        });
    } else if operator == "sum" {
        body.push(Instr::LocalGet(dest));
        lane(body);
        body.push(match lane_type {
            ValType::I32 => Instr::I32Add,
            ValType::I64 => Instr::I64Add,
        });
    } else {
        return Err(format!("unsupported vector reduction {operator}"));
    }
    body.push(Instr::LocalSet(dest));
    Ok(())
}

fn store_width(width: SlotWidth, body: &mut Vec<Instr>) {
    match width {
        SlotWidth::W32 => body.push(Instr::I32Store),
        SlotWidth::W64 => body.push(Instr::I64Store),
    }
}

/// Store instruction matching an operand's SSA value type at the top of stack.
fn store_instr(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
    index: usize,
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    let identity = instruction
        .inputs
        .get(index)
        .ok_or_else(|| format!("payload operand {index} is missing"))?;
    let valtype = layout.types.get(identity).copied().unwrap_or(ValType::I32);
    match valtype {
        ValType::I64 => store_width(SlotWidth::W64, body),
        ValType::I32 => store_width(SlotWidth::W32, body),
    }
    Ok(())
}

/// Store for one sequence element: width follows the operand's realization.
fn store_element_width(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
    index: usize,
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    let identity = instruction
        .inputs
        .get(index)
        .ok_or_else(|| format!("sequence element {index} is missing"))?;
    let valtype = layout.types.get(identity).copied().unwrap_or(ValType::I32);
    match valtype {
        ValType::I64 => store_width(SlotWidth::W64, body),
        ValType::I32 => store_width(SlotWidth::W32, body),
    }
    Ok(())
}

/// Load for one sequence element: the destination's realization type picks
/// the load width, mirroring the canonical slot contract.
fn load_element_width(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
    body: &mut Vec<Instr>,
) -> Result<(), String> {
    let output = instruction
        .outputs
        .first()
        .ok_or_else(|| "projection has no result".to_owned())?;
    if matches!(&output.ty, IrType::Named(name) if name == "byte") {
        // A bounded byte view is a packed host buffer, not an arena cell.
        // Read one unsigned byte so adjacent bytes in the same view do not
        // get folded into a four-byte scalar load.
        body.push(Instr::I32Load8U);
    } else {
        let valtype = layout
            .types
            .get(&output.identity)
            .copied()
            .unwrap_or(ValType::I32);
        match valtype {
            ValType::I64 => body.push(Instr::I64Load),
            ValType::I32 => body.push(Instr::I32Load),
        }
    }
    Ok(())
}

/// Explicit total conversion (Profile 0.7): narrowing truncates high bits;
/// widening extends by the *source* signedness (`byte` zero-extends).
fn emit_convert(
    layout: &FunctionLayout,
    instruction: &mncs_model::SsaInstruction,
    body: &mut Vec<Instr>,
    from: BodyType,
    to: BodyType,
) -> Result<(), String> {
    let dest = dest_local(layout, instruction)?;
    let src = operand_local(layout, instruction, 0)?;
    let src_wide = matches!(&from, BodyType::Integer(ty) if ty.bits == 64);
    let dst_wide = matches!(&to, BodyType::Integer(ty) if ty.bits == 64);
    let dst_signed = matches!(&to, BodyType::Integer(ty) if ty.signed);
    let (dst_bits, dst_is_byte) = match &to {
        BodyType::Byte => (8u16, true),
        BodyType::Integer(ty) => (ty.bits, false),
        _ => return Err("conversion target must be scalar".to_owned()),
    };
    body.push(Instr::LocalGet(src));
    if !src_wide && dst_wide {
        // Widen i32 cells into i64 by source signedness; bytes zero-extend.
        if matches!(&from, BodyType::Integer(ty) if ty.signed) {
            body.push(Instr::I64ExtendI32S);
        } else {
            body.push(Instr::I64ExtendI32U);
        }
    } else if src_wide && !dst_wide {
        body.push(Instr::I32WrapI64);
    }
    if dst_bits < 32 || dst_is_byte {
        // Narrow targets normalize inside their declared domain.
        let mask = (1_i32.checked_shl(u32::from(dst_bits)).unwrap_or(0)).wrapping_sub(1);
        body.push(Instr::I32Const(mask));
        body.push(Instr::I32And);
        if dst_signed {
            let sign = 1_i32.wrapping_shl(u32::from(dst_bits - 1));
            body.push(Instr::I32Const(sign));
            body.push(Instr::I32Xor);
            body.push(Instr::I32Const(sign));
            body.push(Instr::I32Sub);
        }
    } else if dst_wide && matches!(&to, BodyType::Integer(ty) if ty.bits < 64) {
        let mask = ((1_i128 << dst_bits) - 1) as i64;
        body.push(Instr::I64Const(mask));
        body.push(Instr::I64And);
        if dst_signed {
            let shift = 64 - i64::from(dst_bits);
            body.push(Instr::I64Const(shift));
            body.push(Instr::I64Shl);
            body.push(Instr::I64Const(shift));
            body.push(Instr::I64ShrS);
        }
    }
    body.push(Instr::LocalSet(dest));
    Ok(())
}

fn wasm_type(ty: &IrType) -> Result<(ValType, Option<IntegerType>), String> {
    match ty {
        IrType::Finite { .. } => Ok((ValType::I32, None)),
        // Records and payload-bearing finite variants are realized as pointers
        // into linear memory; the value's local holds the cell address.
        IrType::Record { .. } => Ok((ValType::I32, None)),
        IrType::Named(name) if name == "bool" => Ok((ValType::I32, None)),
        IrType::Named(name) => match BodyType::from_semantic_name(name) {
            BodyType::Integer(integer) => Ok((val_type(integer)?, Some(integer))),
            // Bytes ride zero-extended in i32 cells.
            BodyType::Byte => Ok((ValType::I32, None)),
            // Exact sequences are canonical cell pointers; bounded views are
            // packed (offset | length << 32) descriptors riding i64.
            BodyType::Sequence {
                bound: mncs_model::SequenceBound::Exact(_),
                ..
            } => Ok((ValType::I32, None)),
            BodyType::Sequence {
                bound: mncs_model::SequenceBound::UpTo(_),
                ..
            } => Ok((ValType::I64, None)),
            BodyType::Sequence {
                bound: mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_),
                ..
            } => {
                Err("generic SequenceBound must be specialized before backend lowering".to_owned())
            }
            BodyType::Vector { .. } => Ok((ValType::I32, None)),
            BodyType::Mask { .. } => Ok((ValType::I64, None)),
            BodyType::Named(_) | BodyType::Finite { .. } | BodyType::Record { .. } => {
                Err(format!("unsupported SSA type {name}"))
            }
            BodyType::GenericParam { .. } => {
                Err("generic type parameter must be specialized before backend lowering".to_owned())
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
