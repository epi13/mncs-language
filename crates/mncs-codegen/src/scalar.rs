//! Backend-neutral scalar view of the currently supported selected-SSA subset.
//!
//! Adapters pretty-print this view. They do not reinterpret MNCS source.
//! Composite values (records, boxed finite variants) appear as references to
//! canonical arena cells; see `crate::composite` for the language-owned
//! layout contract every backend realizes.

use std::collections::BTreeMap;

use mncs_model::{
    ArithmeticIntent, BodyType, IntegerType, IrType, Program, SemanticId, SsaFunction,
    SsaInstruction, SsaInstructionKind, SsaModule, SsaTerminator, SsaValue,
};

use crate::composite::{CompositeLayout, SlotWidth};
use crate::promises::{
    integer_no_overflow_promise, non_arithmetic_promise_decisions, promise_summary, LoweringPromise,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarTy {
    Bool,
    Int(IntegerType),
    /// A byte-oriented 8-bit unsigned logical value (Profile 0.7),
    /// realized in one unsigned 8-bit cell.
    Byte,
    Finite,
    /// A reference to a canonical arena cell (record, boxed finite
    /// variant, or fixed-length sequence). The value itself is the
    /// cell's byte offset.
    Cell,
    /// A bounded view descriptor packed into one 64-bit cell: the low
    /// 32 bits hold the source cell offset, the high 32 bits the runtime
    /// length. One valid representation among possible realizations.
    View,
    /// One packed-bits realization of a semantic mask. Lane identity remains
    /// in SSA operation metadata; this physical choice is backend-local.
    Mask(u32),
}

impl ScalarTy {
    /// Whether values of this type are canonical cell references.
    pub fn is_cell(self) -> bool {
        matches!(self, Self::Cell)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarValue {
    pub id: SemanticId,
    pub ty: ScalarTy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarInst {
    Const {
        dest: ScalarValue,
        value: i128,
    },
    Integer {
        dest: ScalarValue,
        operator: String,
        intent: ArithmeticIntent,
        lhs: SemanticId,
        rhs: SemanticId,
        promise: Box<LoweringPromise>,
    },
    Compare {
        dest: ScalarValue,
        predicate: String,
        operand: IntegerType,
        lhs: SemanticId,
        rhs: SemanticId,
    },
    Boolean {
        dest: ScalarValue,
        operator: String,
        lhs: SemanticId,
        rhs: SemanticId,
    },
    FiniteConstruct {
        dest: ScalarValue,
        discriminant: u32,
    },
    /// Allocate an uninitialized canonical cell and produce its offset.
    CellAlloc {
        dest: ScalarValue,
        bytes: u64,
    },
    /// Store a variant discriminant into slot 0 of a boxed finite cell.
    CellStoreDiscriminant {
        cell: SemanticId,
        discriminant: u32,
    },
    /// Store a field value at a canonical byte offset inside a cell.
    CellStore {
        cell: SemanticId,
        byte_offset: u64,
        width: SlotWidth,
        value: SemanticId,
    },
    /// Load a field value from a canonical byte offset inside a cell.
    CellLoad {
        dest: ScalarValue,
        cell: SemanticId,
        byte_offset: u64,
        width: SlotWidth,
    },
    FiniteIsVariant {
        dest: ScalarValue,
        src: SemanticId,
        discriminant: u32,
    },
    /// Byte bitwise op (Profile 0.7): `and` | `or` | `xor` over two byte
    /// cells; realized as an 8-bit unsigned operation.
    ByteBitwise {
        dest: ScalarValue,
        operator: String,
        lhs: SemanticId,
        rhs: SemanticId,
    },
    /// Byte shift (Profile 0.7): `shl` | `shr` with a u64 count taken
    /// modulo 8.
    ByteShift {
        dest: ScalarValue,
        operator: String,
        lhs: SemanticId,
        rhs: SemanticId,
    },
    /// Explicit total scalar conversion (Profile 0.7). Truncation drops high
    /// bits; widening extends by the source signedness (`byte` zero-extends).
    Convert {
        dest: ScalarValue,
        from: ScalarTy,
        to: ScalarTy,
        src: SemanticId,
    },
    /// Project one element of a bounded sequence (Profile 0.7). Exact
    /// sequences are canonical cells (address `cell + index * 8`); views are
    /// packed descriptors whose low 32 bits hold the source offset and whose
    /// high 32 bits hold the runtime length. Realization performs the bounds
    /// check exactly when the evidence is `RuntimeChecked`, trapping on
    /// violation; other evidences discharge the check statically or through
    /// traversal semantics.
    SequenceProject {
        dest: ScalarValue,
        seq: SemanticId,
        index: SemanticId,
        bound: mncs_model::SequenceBound,
        evidence: mncs_model::BoundsEvidence,
        width: SlotWidth,
    },
    /// Observe a bounded sequence's length (Profile 0.7). Exact bounds fold
    /// to constants; views expose their packed runtime length.
    SequenceLength {
        dest: ScalarValue,
        bound: mncs_model::SequenceBound,
        seq: SemanticId,
    },
    /// Derive a bounded view over a checked half-open range (Profile 0.7).
    /// Realization checks `start <= end`, `end <= len(source)`, and
    /// `end - start <= cap` before packing `(offset | length << 32)` into
    /// one 64-bit descriptor; violations trap as explicit failures.
    ViewConstruct {
        dest: ScalarValue,
        source_bound: mncs_model::SequenceBound,
        view_cap: u32,
        source: SemanticId,
        start: SemanticId,
        end: SemanticId,
    },
    /// Semantic conditional selection (Profile 0.8). Both candidates are
    /// values of one pure selection; every realization of this instruction
    /// is obligated to avoid data-dependent branch divergence between them.
    /// Whether each backend met that obligation is decided by independent
    /// structural verification, never by this lowering's self-report.
    Select {
        dest: ScalarValue,
        condition: SemanticId,
        when_true: SemanticId,
        when_false: SemanticId,
    },
    /// Total functional sequence update over an exact bound (Profile 0.8):
    /// allocate a new cell, copy the source slots, store the replacement
    /// element at the (dynamically checked when required) index. The source
    /// cell is never written.
    SequenceReplace {
        dest: ScalarValue,
        source: SemanticId,
        index: SemanticId,
        element: SemanticId,
        bound: mncs_model::SequenceBound,
        evidence: mncs_model::BoundsEvidence,
        length: u32,
        element_width: SlotWidth,
    },
    /// One source instruction lowered into an ordered group of cell
    /// operations (allocation followed by canonical field stores).
    Sequence(Vec<ScalarInst>),
    Call {
        dest: ScalarValue,
        callee: String,
        args: Vec<SemanticId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScalarTerm {
    Return {
        value: SemanticId,
    },
    Jump {
        target: SemanticId,
        args: Vec<SemanticId>,
    },
    Branch {
        cond: SemanticId,
        then_target: SemanticId,
        then_args: Vec<SemanticId>,
        else_target: SemanticId,
        else_args: Vec<SemanticId>,
    },
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarBlock {
    pub id: SemanticId,
    pub params: Vec<ScalarValue>,
    pub insts: Vec<ScalarInst>,
    pub term: ScalarTerm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarFunction {
    pub export_name: String,
    pub params: Vec<ScalarValue>,
    pub result: ScalarValue,
    pub blocks: Vec<ScalarBlock>,
    pub promises: Vec<String>,
    pub promise_decisions: Vec<mncs_model::BackendPromiseDecision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarModule {
    pub functions: Vec<ScalarFunction>,
    pub unsupported: Vec<String>,
    pub features: Vec<String>,
    pub promise_decisions: Vec<mncs_model::BackendPromiseDecision>,
}

pub fn lower_to_scalar(program: &Program, ssa: &SsaModule, names: &[String]) -> ScalarModule {
    let mut functions = Vec::new();
    let mut unsupported = Vec::new();
    let mut features = Vec::new();
    let mut promise_decisions = Vec::new();
    let layout = CompositeLayout::from_program(program);
    let callees: BTreeMap<SemanticId, String> = ssa
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| {
            (
                function.semantic_identity.clone(),
                names
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| function.semantic_identity.0.clone()),
            )
        })
        .collect();
    for (index, function) in ssa.functions.iter().enumerate() {
        let name = names
            .get(index)
            .cloned()
            .unwrap_or_else(|| crate::support::export_name(&function.semantic_identity.0));
        match lower_function(ssa, function, name, &callees, &layout) {
            Ok(lowered) => {
                features.extend(lowered.promises.iter().cloned());
                promise_decisions.extend(lowered.promise_decisions.iter().cloned());
                functions.push(lowered);
            }
            Err(reason) => unsupported.push(format!("{}: {reason}", function.identity.0)),
        }
    }
    if (!layout.records.is_empty() || !layout.boxed_finites.is_empty())
        && functions
            .iter()
            .any(|function| function_uses_cells(function, &layout))
    {
        features.push("canonical_composite_cells".to_owned());
    }
    if ssa
        .functions
        .iter()
        .any(|function| !function.bounded_iterations.is_empty())
    {
        features.push("semantic_bounded_iteration".to_owned());
    }
    features.sort();
    features.dedup();
    ScalarModule {
        functions,
        unsupported,
        features,
        promise_decisions,
    }
}

/// Whether a lowered function manipulates canonical cells (used to annotate
/// module features honestly).
fn function_uses_cells(function: &ScalarFunction, _layout: &CompositeLayout) -> bool {
    let ty_is_cell = |ty: &ScalarTy| ty.is_cell();
    function.params.iter().any(|param| ty_is_cell(&param.ty))
        || ty_is_cell(&function.result.ty)
        || function.blocks.iter().any(|block| {
            block.insts.iter().any(|inst| {
                matches!(
                    inst,
                    ScalarInst::CellAlloc { .. }
                        | ScalarInst::CellStoreDiscriminant { .. }
                        | ScalarInst::CellStore { .. }
                        | ScalarInst::CellLoad { .. }
                )
            })
        })
}

fn lower_function(
    module: &SsaModule,
    function: &SsaFunction,
    export_name: String,
    callees: &BTreeMap<SemanticId, String>,
    layout: &CompositeLayout,
) -> Result<ScalarFunction, String> {
    if function.outputs.len() != 1 {
        return Err("this realization requires exactly one result".to_owned());
    }
    let params = function
        .inputs
        .iter()
        .map(|value| scalar_value(value, layout))
        .collect::<Result<Vec<_>, _>>()?;
    let result = scalar_value(&function.outputs[0], layout)?;
    let mut promises = Vec::new();
    let mut promise_decisions = Vec::new();
    let mut blocks = Vec::new();
    for block in &function.blocks {
        let params = block
            .parameters
            .iter()
            .map(|value| scalar_value(value, layout))
            .collect::<Result<Vec<_>, _>>()?;
        let mut insts = Vec::new();
        for instruction in &block.instructions {
            insts.push(lower_instruction(
                module,
                instruction,
                callees,
                layout,
                &mut promises,
                &mut promise_decisions,
            )?);
        }
        blocks.push(ScalarBlock {
            id: block.identity.clone(),
            params,
            insts,
            term: lower_term(&block.terminator)?,
        });
    }
    promises.sort();
    promises.dedup();
    Ok(ScalarFunction {
        export_name,
        params,
        result,
        blocks,
        promises,
        promise_decisions,
    })
}

fn lower_instruction(
    module: &SsaModule,
    instruction: &SsaInstruction,
    callees: &BTreeMap<SemanticId, String>,
    layout: &CompositeLayout,
    promises: &mut Vec<String>,
    promise_decisions: &mut Vec<mncs_model::BackendPromiseDecision>,
) -> Result<ScalarInst, String> {
    let dest = instruction
        .outputs
        .first()
        .ok_or_else(|| "instruction has no destination".to_owned())?;
    let dest = scalar_value(dest, layout)?;
    match &instruction.kind {
        SsaInstructionKind::Constant { value, .. } => Ok(ScalarInst::Const {
            dest,
            value: *value,
        }),
        SsaInstructionKind::Integer {
            operator, intent, ..
        } => {
            // Saturating and wrapping arithmetic are total by semantics and
            // are realized by every scalar backend; widening still needs a
            // wider result cell than the scalar envelope provides.
            if matches!(intent, ArithmeticIntent::Widening { .. }) {
                return Err(format!(
                    "arithmetic intent {intent:?} is outside the current scalar realization envelope"
                ));
            }
            if !matches!(
                operator.as_str(),
                "add" | "sub" | "mul" | "div" | "mod" | "and" | "or" | "xor" | "shl" | "shr"
            ) {
                return Err(format!("unsupported integer operator {operator}"));
            }
            let promise = integer_no_overflow_promise(module, instruction);
            promises.push(promise_summary(&promise));
            promise_decisions.push(promise.decision.clone());
            promise_decisions.extend(non_arithmetic_promise_decisions(instruction));
            Ok(ScalarInst::Integer {
                dest,
                operator: operator.clone(),
                intent: *intent,
                lhs: operand(instruction, 0)?,
                rhs: operand(instruction, 1)?,
                promise: Box::new(promise),
            })
        }
        SsaInstructionKind::IntegerCompare {
            predicate,
            operand_type,
        } => Ok(ScalarInst::Compare {
            dest,
            predicate: predicate.clone(),
            operand: *operand_type,
            lhs: operand(instruction, 0)?,
            rhs: operand(instruction, 1)?,
        }),
        SsaInstructionKind::FiniteConstruct {
            type_identity,
            discriminant,
            payload_fields,
            ..
        } => {
            let Some(fields) = layout.boxed_finites.get(type_identity) else {
                if !payload_fields.is_empty() {
                    return Err(
                        "payload-bearing finite construction names an unboxed finite type"
                            .to_owned(),
                    );
                }
                return Ok(ScalarInst::FiniteConstruct {
                    dest,
                    discriminant: *discriminant,
                });
            };
            // Boxed realization: canonical cell with the tag in slot 0.
            let fields = fields.get(discriminant).ok_or_else(|| {
                format!("finite variant {discriminant} has no declared payload layout")
            })?;
            if fields.len() != payload_fields.len() || fields.len() != instruction.inputs.len() {
                return Err(
                    "payload field count does not match the declared variant layout".to_owned(),
                );
            }
            let cell = dest.id.clone();
            let bytes = (fields.len() as u64 + 1) * 8;
            let mut insts = vec![ScalarInst::CellAlloc {
                dest: dest.clone(),
                bytes,
            }];
            insts.push(ScalarInst::CellStoreDiscriminant {
                cell: cell.clone(),
                discriminant: *discriminant,
            });
            for (index, field) in fields.iter().enumerate() {
                insts.push(ScalarInst::CellStore {
                    cell: cell.clone(),
                    byte_offset: (index as u64 + 1) * 8,
                    width: field.width,
                    value: operand(instruction, index)?,
                });
            }
            Ok(ScalarInst::Sequence(insts))
        }
        SsaInstructionKind::BooleanOp { operator } => Ok(ScalarInst::Boolean {
            dest,
            operator: operator.clone(),
            lhs: operand(instruction, 0)?,
            rhs: operand(instruction, 1)?,
        }),
        SsaInstructionKind::ByteBitwise { operator } => Ok(ScalarInst::ByteBitwise {
            dest,
            operator: operator.clone(),
            lhs: operand(instruction, 0)?,
            rhs: operand(instruction, 1)?,
        }),
        SsaInstructionKind::ByteShift { operator } => Ok(ScalarInst::ByteShift {
            dest,
            operator: operator.clone(),
            lhs: operand(instruction, 0)?,
            rhs: operand(instruction, 1)?,
        }),
        SsaInstructionKind::ByteCompare { predicate } => {
            // Bytes realize as unsigned 8-bit cells, so byte comparison
            // reuses the integer comparison machinery exactly.
            Ok(ScalarInst::Compare {
                dest,
                predicate: predicate.clone(),
                operand: IntegerType {
                    bits: 8,
                    signed: false,
                },
                lhs: operand(instruction, 0)?,
                rhs: operand(instruction, 1)?,
            })
        }
        SsaInstructionKind::Convert { from, to } => Ok(ScalarInst::Convert {
            from: scalar_ty_in(&ir_type_of(from), layout)?,
            to: scalar_ty_in(&ir_type_of(to), layout)?,
            dest,
            src: operand(instruction, 0)?,
        }),
        SsaInstructionKind::SequenceConstruct {
            element_type,
            length,
        } => {
            let element_ty = scalar_ty_in(&ir_type_of(element_type), layout)?;
            let width = slot_width_of(element_ty);
            if *length as usize != instruction.inputs.len() {
                return Err("sequence construction does not match its declared length".to_owned());
            }
            let cell = dest.id.clone();
            let mut insts = vec![ScalarInst::CellAlloc {
                dest: dest.clone(),
                bytes: u64::from(*length) * 8,
            }];
            for index in 0..*length as usize {
                insts.push(ScalarInst::CellStore {
                    cell: cell.clone(),
                    byte_offset: index as u64 * 8,
                    width,
                    value: operand(instruction, index)?,
                });
            }
            Ok(ScalarInst::Sequence(insts))
        }
        SsaInstructionKind::SequenceProject { bound, evidence } => {
            let output = instruction
                .outputs
                .first()
                .ok_or_else(|| "projection has no result".to_owned())?;
            let element_ty = scalar_ty_in(&output.ty, layout)?;
            Ok(ScalarInst::SequenceProject {
                dest,
                seq: operand(instruction, 0)?,
                index: operand(instruction, 1)?,
                bound: bound.clone(),
                evidence: evidence.clone(),
                width: slot_width_of(element_ty),
            })
        }
        SsaInstructionKind::SequenceLength { bound } => Ok(ScalarInst::SequenceLength {
            bound: bound.clone(),
            dest,
            seq: operand(instruction, 0)?,
        }),
        SsaInstructionKind::Select { operand_type } => {
            // The branchless promise is recorded as *requested but not yet
            // evidenced*: obligations start UNKNOWN and only independent
            // structural verification of an emitted artifact may discharge
            // them. Lowering never certifies its own realization.
            let decisions = crate::promises::branchless_promise_decisions(module, instruction);
            for decision in &decisions {
                promises.push(format!(
                    "branchless_realization requested (status {:?})",
                    decision
                        .evidence_status
                        .unwrap_or(mncs_model::ObligationStatus::Unknown)
                ));
                promise_decisions.push(decision.clone());
            }
            if let BodyType::Vector { element, lanes } = operand_type {
                lower_vector_select(module, instruction, dest, element, *lanes, layout)
            } else {
                Ok(ScalarInst::Select {
                    dest,
                    condition: operand(instruction, 0)?,
                    when_true: operand(instruction, 1)?,
                    when_false: operand(instruction, 2)?,
                })
            }
        }
        SsaInstructionKind::SequenceReplace {
            element_type,
            bound,
            evidence,
            ..
        } => {
            let mncs_model::SequenceBound::Exact(length) = bound else {
                return Err("functional sequence update requires an exact bound".to_owned());
            };
            let element_ty = scalar_ty_in(&ir_type_of(element_type), layout)?;
            let width = slot_width_of(element_ty);
            Ok(ScalarInst::SequenceReplace {
                dest,
                source: operand(instruction, 0)?,
                index: operand(instruction, 1)?,
                element: operand(instruction, 2)?,
                bound: bound.clone(),
                evidence: evidence.clone(),
                length: *length,
                element_width: width,
            })
        }
        SsaInstructionKind::ViewConstruct {
            source_bound,
            view_bound,
        } => {
            let cap = match view_bound {
                mncs_model::SequenceBound::UpTo(cap) => *cap,
                mncs_model::SequenceBound::Exact(_) => {
                    return Err("view construction must produce an UpTo view".to_owned())
                }
                mncs_model::SequenceBound::Param(_) | mncs_model::SequenceBound::UpToParam(_) => {
                    unreachable!(
                        "generic SequenceBound must be specialized before backend lowering"
                    )
                }
            };
            Ok(ScalarInst::ViewConstruct {
                dest,
                source_bound: source_bound.clone(),
                view_cap: cap,
                source: operand(instruction, 0)?,
                start: operand(instruction, 1)?,
                end: operand(instruction, 2)?,
            })
        }
        SsaInstructionKind::VectorConstruct {
            element_type,
            lanes,
        } => lower_vector_construct(instruction, dest, element_type, *lanes, layout),
        SsaInstructionKind::VectorSplat {
            element_type,
            lanes,
        } => lower_vector_splat(instruction, dest, element_type, *lanes, layout),
        SsaInstructionKind::VectorExtract {
            element_type,
            lanes,
            evidence,
        } => {
            let width = vector_element_width(element_type, layout)?;
            Ok(ScalarInst::SequenceProject {
                dest,
                seq: operand(instruction, 0)?,
                index: operand(instruction, 1)?,
                bound: mncs_model::SequenceBound::Exact(*lanes),
                evidence: evidence.clone(),
                width,
            })
        }
        SsaInstructionKind::VectorReplace {
            element_type,
            lanes,
            evidence,
        } => {
            let width = vector_element_width(element_type, layout)?;
            Ok(ScalarInst::SequenceReplace {
                dest,
                source: operand(instruction, 0)?,
                index: operand(instruction, 1)?,
                element: operand(instruction, 2)?,
                bound: mncs_model::SequenceBound::Exact(*lanes),
                evidence: evidence.clone(),
                length: *lanes,
                element_width: width,
            })
        }
        SsaInstructionKind::VectorBinary {
            operator,
            element_type,
            lanes,
            intent,
        } => lower_vector_binary(
            module,
            instruction,
            dest,
            operator,
            element_type,
            *lanes,
            *intent,
            layout,
        ),
        SsaInstructionKind::VectorCompare {
            predicate,
            element_type,
            lanes,
        } => lower_vector_compare(
            module,
            instruction,
            dest,
            predicate,
            element_type,
            *lanes,
            layout,
        ),
        SsaInstructionKind::MaskBinary { operator, lanes: _ } => {
            let promise = integer_no_overflow_promise(module, instruction);
            Ok(ScalarInst::Integer {
                dest,
                operator: operator.clone(),
                intent: ArithmeticIntent::Wrapping,
                lhs: operand(instruction, 0)?,
                rhs: operand(instruction, 1)?,
                promise: Box::new(promise),
            })
        }
        SsaInstructionKind::MaskNot { lanes } => {
            let limit = mask_limit(*lanes);
            let constant = temporary(instruction, "mask_limit", ScalarTy::Mask(*lanes));
            let promise = integer_no_overflow_promise(module, instruction);
            Ok(ScalarInst::Sequence(vec![
                ScalarInst::Const {
                    dest: constant.clone(),
                    value: i128::from(limit),
                },
                ScalarInst::Integer {
                    dest,
                    operator: "xor".to_owned(),
                    intent: ArithmeticIntent::Wrapping,
                    lhs: operand(instruction, 0)?,
                    rhs: constant.id,
                    promise: Box::new(promise),
                },
            ]))
        }
        SsaInstructionKind::MaskReduce { operator, lanes } => {
            let expected = if operator == "all" {
                mask_limit(*lanes)
            } else {
                0
            };
            let constant = temporary(
                instruction,
                "mask_reduce_rhs",
                ScalarTy::Int(IntegerType {
                    bits: 64,
                    signed: false,
                }),
            );
            Ok(ScalarInst::Sequence(vec![
                ScalarInst::Const {
                    dest: constant.clone(),
                    value: i128::from(expected),
                },
                ScalarInst::Compare {
                    dest,
                    predicate: if operator == "any" {
                        "ne".to_owned()
                    } else {
                        "eq".to_owned()
                    },
                    operand: IntegerType {
                        bits: 64,
                        signed: false,
                    },
                    lhs: operand(instruction, 0)?,
                    rhs: constant.id,
                },
            ]))
        }
        SsaInstructionKind::VectorReduce {
            operator,
            element_type,
            lanes,
            intent,
        } => lower_vector_reduce(
            module,
            instruction,
            dest,
            operator,
            element_type,
            *lanes,
            *intent,
            layout,
        ),
        SsaInstructionKind::FinitePayloadProject {
            type_identity,
            discriminant,
            field,
            ..
        } => {
            let fields = layout
                .boxed_finites
                .get(type_identity)
                .and_then(|variants| variants.get(discriminant))
                .ok_or_else(|| {
                    format!("finite variant {discriminant} has no declared payload layout")
                })?;
            let index = fields
                .iter()
                .position(|entry| entry.name == *field)
                .ok_or_else(|| format!("variant payload has no field {field:?}"))?;
            Ok(ScalarInst::CellLoad {
                dest,
                cell: operand(instruction, 0)?,
                byte_offset: (index as u64 + 1) * 8,
                width: fields[index].width,
            })
        }
        SsaInstructionKind::FiniteIsVariant { discriminant, .. } => {
            Ok(ScalarInst::FiniteIsVariant {
                dest,
                src: operand(instruction, 0)?,
                discriminant: *discriminant,
            })
        }
        SsaInstructionKind::Call { function, .. } => {
            let callee = callees
                .get(function)
                .cloned()
                .ok_or_else(|| "call target is not in this selected SSA module".to_owned())?;
            Ok(ScalarInst::Call {
                dest,
                callee,
                args: instruction.inputs.clone(),
            })
        }
        SsaInstructionKind::Effect => {
            Err("effects are unsupported on this scalar realization envelope".to_owned())
        }
        SsaInstructionKind::RuntimeCheck { .. } => {
            Err("runtime checks have no executable condition in the current SSA subset".to_owned())
        }
        SsaInstructionKind::RecordConstruct {
            type_identity,
            field_names,
            ..
        } => {
            let Some(fields) = layout.records.get(type_identity) else {
                return Err(format!(
                    "record construction names an unknown record layout {type_identity}"
                ));
            };
            if fields.len() != field_names.len() || fields.len() != instruction.inputs.len() {
                return Err("record construction does not match its declared layout".to_owned());
            }
            let cell = dest.id.clone();
            let mut insts = vec![ScalarInst::CellAlloc {
                dest: dest.clone(),
                bytes: fields.len() as u64 * 8,
            }];
            for (index, entry) in fields.iter().enumerate() {
                if entry.name != field_names[index] {
                    return Err(format!(
                        "record construction field {:?} does not match the canonical layout {:?}",
                        field_names[index], entry.name
                    ));
                }
                insts.push(ScalarInst::CellStore {
                    cell: cell.clone(),
                    byte_offset: index as u64 * 8,
                    width: entry.width,
                    value: operand(instruction, index)?,
                });
            }
            Ok(ScalarInst::Sequence(insts))
        }
        SsaInstructionKind::RecordProject {
            type_identity,
            field,
            ..
        } => {
            let Some(fields) = layout.records.get(type_identity) else {
                return Err(format!(
                    "record projection names an unknown record layout {type_identity}"
                ));
            };
            let index = fields
                .iter()
                .position(|entry| entry.name == *field)
                .ok_or_else(|| format!("record layout has no field {field:?}"))?;
            Ok(ScalarInst::CellLoad {
                dest,
                cell: operand(instruction, 0)?,
                byte_offset: index as u64 * 8,
                width: fields[index].width,
            })
        }
    }
}

fn temporary(instruction: &SsaInstruction, suffix: &str, ty: ScalarTy) -> ScalarValue {
    ScalarValue {
        id: SemanticId(format!("{}:scalar:{suffix}", instruction.identity.0)),
        ty,
    }
}

fn vector_element(
    element: &BodyType,
    layout: &CompositeLayout,
) -> Result<(ScalarTy, SlotWidth, IntegerType), String> {
    let BodyType::Integer(integer) = element else {
        return Err("initial scalar vector realization supports integer lanes only".to_owned());
    };
    let ty = scalar_ty_in(&ir_type_of(element), layout)?;
    Ok((ty, slot_width_of(ty), *integer))
}

fn vector_element_width(element: &BodyType, layout: &CompositeLayout) -> Result<SlotWidth, String> {
    vector_element(element, layout).map(|(_, width, _)| width)
}

fn mask_limit(lanes: u32) -> u64 {
    if lanes >= 64 {
        u64::MAX
    } else {
        (1_u64 << lanes) - 1
    }
}

fn lower_vector_construct(
    instruction: &SsaInstruction,
    dest: ScalarValue,
    element: &BodyType,
    lanes: u32,
    layout: &CompositeLayout,
) -> Result<ScalarInst, String> {
    let (_, width, _) = vector_element(element, layout)?;
    if instruction.inputs.len() != lanes as usize {
        return Err("vector construction does not match its declared lane count".to_owned());
    }
    let mut insts = vec![ScalarInst::CellAlloc {
        dest: dest.clone(),
        bytes: u64::from(lanes) * 8,
    }];
    for lane in 0..lanes {
        insts.push(ScalarInst::CellStore {
            cell: dest.id.clone(),
            byte_offset: u64::from(lane) * 8,
            width,
            value: operand(instruction, lane as usize)?,
        });
    }
    Ok(ScalarInst::Sequence(insts))
}

fn lower_vector_splat(
    instruction: &SsaInstruction,
    dest: ScalarValue,
    element: &BodyType,
    lanes: u32,
    layout: &CompositeLayout,
) -> Result<ScalarInst, String> {
    let (_, width, _) = vector_element(element, layout)?;
    let value = operand(instruction, 0)?;
    let mut insts = vec![ScalarInst::CellAlloc {
        dest: dest.clone(),
        bytes: u64::from(lanes) * 8,
    }];
    for lane in 0..lanes {
        insts.push(ScalarInst::CellStore {
            cell: dest.id.clone(),
            byte_offset: u64::from(lane) * 8,
            width,
            value: value.clone(),
        });
    }
    Ok(ScalarInst::Sequence(insts))
}

fn lower_vector_select(
    module: &SsaModule,
    instruction: &SsaInstruction,
    dest: ScalarValue,
    element: &BodyType,
    lanes: u32,
    layout: &CompositeLayout,
) -> Result<ScalarInst, String> {
    let (element_ty, width, _) = vector_element(element, layout)?;
    let mask = operand(instruction, 0)?;
    let when_true = operand(instruction, 1)?;
    let when_false = operand(instruction, 2)?;
    let mut insts = vec![ScalarInst::CellAlloc {
        dest: dest.clone(),
        bytes: u64::from(lanes) * 8,
    }];
    for lane in 0..lanes {
        let true_lane = temporary(instruction, &format!("select_true_{lane}"), element_ty);
        let false_lane = temporary(instruction, &format!("select_false_{lane}"), element_ty);
        let count = temporary(
            instruction,
            &format!("select_count_{lane}"),
            ScalarTy::Int(IntegerType {
                bits: 64,
                signed: false,
            }),
        );
        let shifted = temporary(
            instruction,
            &format!("select_shift_{lane}"),
            ScalarTy::Mask(lanes),
        );
        let one = temporary(
            instruction,
            &format!("select_one_{lane}"),
            ScalarTy::Mask(lanes),
        );
        let active = temporary(
            instruction,
            &format!("select_active_{lane}"),
            ScalarTy::Mask(lanes),
        );
        let selected = temporary(instruction, &format!("select_lane_{lane}"), element_ty);
        let promise = integer_no_overflow_promise(module, instruction);
        insts.extend([
            ScalarInst::CellLoad {
                dest: true_lane.clone(),
                cell: when_true.clone(),
                byte_offset: u64::from(lane) * 8,
                width,
            },
            ScalarInst::CellLoad {
                dest: false_lane.clone(),
                cell: when_false.clone(),
                byte_offset: u64::from(lane) * 8,
                width,
            },
            ScalarInst::Const {
                dest: count.clone(),
                value: i128::from(lane),
            },
            ScalarInst::Integer {
                dest: shifted.clone(),
                operator: "shr".to_owned(),
                intent: ArithmeticIntent::Wrapping,
                lhs: mask.clone(),
                rhs: count.id,
                promise: Box::new(promise.clone()),
            },
            ScalarInst::Const {
                dest: one.clone(),
                value: 1,
            },
            ScalarInst::Integer {
                dest: active.clone(),
                operator: "and".to_owned(),
                intent: ArithmeticIntent::Wrapping,
                lhs: shifted.id,
                rhs: one.id,
                promise: Box::new(promise),
            },
            ScalarInst::Select {
                dest: selected.clone(),
                condition: active.id,
                when_true: true_lane.id,
                when_false: false_lane.id,
            },
            ScalarInst::CellStore {
                cell: dest.id.clone(),
                byte_offset: u64::from(lane) * 8,
                width,
                value: selected.id,
            },
        ]);
    }
    Ok(ScalarInst::Sequence(insts))
}

#[allow(clippy::too_many_arguments)]
fn lower_vector_binary(
    module: &SsaModule,
    instruction: &SsaInstruction,
    dest: ScalarValue,
    operator: &str,
    element: &BodyType,
    lanes: u32,
    intent: ArithmeticIntent,
    layout: &CompositeLayout,
) -> Result<ScalarInst, String> {
    if matches!(intent, ArithmeticIntent::Widening { .. }) {
        return Err("scalar vector widening awaits a distinct widened lane result type".to_owned());
    }
    let (element_ty, width, integer) = vector_element(element, layout)?;
    let left = operand(instruction, 0)?;
    let right = operand(instruction, 1)?;
    let mut insts = vec![ScalarInst::CellAlloc {
        dest: dest.clone(),
        bytes: u64::from(lanes) * 8,
    }];
    for lane in 0..lanes {
        let lhs = temporary(instruction, &format!("binary_lhs_{lane}"), element_ty);
        let rhs = temporary(instruction, &format!("binary_rhs_{lane}"), element_ty);
        let value = temporary(instruction, &format!("binary_value_{lane}"), element_ty);
        insts.push(ScalarInst::CellLoad {
            dest: lhs.clone(),
            cell: left.clone(),
            byte_offset: u64::from(lane) * 8,
            width,
        });
        insts.push(ScalarInst::CellLoad {
            dest: rhs.clone(),
            cell: right.clone(),
            byte_offset: u64::from(lane) * 8,
            width,
        });
        if matches!(operator, "min" | "max") {
            let condition = temporary(instruction, &format!("binary_cmp_{lane}"), ScalarTy::Bool);
            insts.push(ScalarInst::Compare {
                dest: condition.clone(),
                predicate: if operator == "min" {
                    "lt".to_owned()
                } else {
                    "gt".to_owned()
                },
                operand: integer,
                lhs: lhs.id.clone(),
                rhs: rhs.id.clone(),
            });
            insts.push(ScalarInst::Select {
                dest: value.clone(),
                condition: condition.id,
                when_true: lhs.id,
                when_false: rhs.id,
            });
        } else {
            insts.push(ScalarInst::Integer {
                dest: value.clone(),
                operator: operator.to_owned(),
                intent,
                lhs: lhs.id,
                rhs: rhs.id,
                promise: Box::new(integer_no_overflow_promise(module, instruction)),
            });
        }
        insts.push(ScalarInst::CellStore {
            cell: dest.id.clone(),
            byte_offset: u64::from(lane) * 8,
            width,
            value: value.id,
        });
    }
    Ok(ScalarInst::Sequence(insts))
}

fn lower_vector_compare(
    module: &SsaModule,
    instruction: &SsaInstruction,
    dest: ScalarValue,
    predicate: &str,
    element: &BodyType,
    lanes: u32,
    layout: &CompositeLayout,
) -> Result<ScalarInst, String> {
    let (element_ty, width, integer) = vector_element(element, layout)?;
    let left = operand(instruction, 0)?;
    let right = operand(instruction, 1)?;
    let initial = temporary(instruction, "compare_mask_0", ScalarTy::Mask(lanes));
    let mut accumulator = initial.clone();
    let mut insts = vec![ScalarInst::Const {
        dest: initial,
        value: 0,
    }];
    for lane in 0..lanes {
        let lhs = temporary(instruction, &format!("compare_lhs_{lane}"), element_ty);
        let rhs = temporary(instruction, &format!("compare_rhs_{lane}"), element_ty);
        let bit = temporary(
            instruction,
            &format!("compare_bit_{lane}"),
            ScalarTy::Mask(lanes),
        );
        let count = temporary(
            instruction,
            &format!("compare_count_{lane}"),
            ScalarTy::Int(IntegerType {
                bits: 64,
                signed: false,
            }),
        );
        let shifted = temporary(
            instruction,
            &format!("compare_shifted_{lane}"),
            ScalarTy::Mask(lanes),
        );
        let next = if lane + 1 == lanes {
            dest.clone()
        } else {
            temporary(
                instruction,
                &format!("compare_mask_{}", lane + 1),
                ScalarTy::Mask(lanes),
            )
        };
        insts.extend([
            ScalarInst::CellLoad {
                dest: lhs.clone(),
                cell: left.clone(),
                byte_offset: u64::from(lane) * 8,
                width,
            },
            ScalarInst::CellLoad {
                dest: rhs.clone(),
                cell: right.clone(),
                byte_offset: u64::from(lane) * 8,
                width,
            },
            ScalarInst::Compare {
                dest: bit.clone(),
                predicate: predicate.to_owned(),
                operand: integer,
                lhs: lhs.id,
                rhs: rhs.id,
            },
            ScalarInst::Const {
                dest: count.clone(),
                value: i128::from(lane),
            },
            ScalarInst::Integer {
                dest: shifted.clone(),
                operator: "shl".to_owned(),
                intent: ArithmeticIntent::Wrapping,
                lhs: bit.id,
                rhs: count.id,
                promise: Box::new(integer_no_overflow_promise(module, instruction)),
            },
            ScalarInst::Integer {
                dest: next.clone(),
                operator: "or".to_owned(),
                intent: ArithmeticIntent::Wrapping,
                lhs: accumulator.id,
                rhs: shifted.id,
                promise: Box::new(integer_no_overflow_promise(module, instruction)),
            },
        ]);
        accumulator = next;
    }
    Ok(ScalarInst::Sequence(insts))
}

#[allow(clippy::too_many_arguments)]
fn lower_vector_reduce(
    module: &SsaModule,
    instruction: &SsaInstruction,
    dest: ScalarValue,
    operator: &str,
    element: &BodyType,
    lanes: u32,
    intent: ArithmeticIntent,
    layout: &CompositeLayout,
) -> Result<ScalarInst, String> {
    if lanes == 0 {
        return Err("vector reduction requires at least one lane".to_owned());
    }
    let (element_ty, width, integer) = vector_element(element, layout)?;
    let vector = operand(instruction, 0)?;
    let first = if lanes == 1 {
        dest.clone()
    } else {
        temporary(instruction, "reduce_acc_0", element_ty)
    };
    let mut insts = vec![ScalarInst::CellLoad {
        dest: first.clone(),
        cell: vector.clone(),
        byte_offset: 0,
        width,
    }];
    let mut accumulator = first;
    for lane in 1..lanes {
        let value = temporary(instruction, &format!("reduce_lane_{lane}"), element_ty);
        let next = if lane + 1 == lanes {
            dest.clone()
        } else {
            temporary(instruction, &format!("reduce_acc_{lane}"), element_ty)
        };
        insts.push(ScalarInst::CellLoad {
            dest: value.clone(),
            cell: vector.clone(),
            byte_offset: u64::from(lane) * 8,
            width,
        });
        if operator == "sum" {
            insts.push(ScalarInst::Integer {
                dest: next.clone(),
                operator: "add".to_owned(),
                intent,
                lhs: accumulator.id,
                rhs: value.id,
                promise: Box::new(integer_no_overflow_promise(module, instruction)),
            });
        } else if matches!(operator, "min" | "max") {
            let condition = temporary(instruction, &format!("reduce_cmp_{lane}"), ScalarTy::Bool);
            insts.push(ScalarInst::Compare {
                dest: condition.clone(),
                predicate: if operator == "min" {
                    "lt".to_owned()
                } else {
                    "gt".to_owned()
                },
                operand: integer,
                lhs: accumulator.id.clone(),
                rhs: value.id.clone(),
            });
            insts.push(ScalarInst::Select {
                dest: next.clone(),
                condition: condition.id,
                when_true: accumulator.id,
                when_false: value.id,
            });
        } else {
            return Err(format!("unsupported vector reduction {operator}"));
        }
        accumulator = next;
    }
    Ok(ScalarInst::Sequence(insts))
}

fn lower_term(terminator: &SsaTerminator) -> Result<ScalarTerm, String> {
    Ok(match terminator {
        SsaTerminator::Return { values } => ScalarTerm::Return {
            value: values
                .first()
                .cloned()
                .ok_or_else(|| "return terminator has no value".to_owned())?,
        },
        SsaTerminator::Branch { target, arguments } => ScalarTerm::Jump {
            target: target.clone(),
            args: arguments.clone(),
        },
        SsaTerminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        } => ScalarTerm::Branch {
            cond: condition.clone(),
            then_target: then_target.clone(),
            then_args: then_arguments.clone(),
            else_target: else_target.clone(),
            else_args: else_arguments.clone(),
        },
        SsaTerminator::Failure { .. } => ScalarTerm::Fail,
    })
}

fn operand(instruction: &SsaInstruction, index: usize) -> Result<SemanticId, String> {
    instruction
        .inputs
        .get(index)
        .cloned()
        .ok_or_else(|| "instruction is missing an operand".to_owned())
}

fn scalar_value(value: &SsaValue, layout: &CompositeLayout) -> Result<ScalarValue, String> {
    Ok(ScalarValue {
        id: value.identity.clone(),
        ty: scalar_ty_in(&value.ty, layout)?,
    })
}

/// Map a semantic type into its scalar realization type. Composite types
/// become canonical cell references when the program declares their layout.
pub fn scalar_ty_in(ty: &IrType, layout: &CompositeLayout) -> Result<ScalarTy, String> {
    match ty {
        IrType::Finite { identity, .. } => {
            if layout.is_boxed_finite(identity) {
                Ok(ScalarTy::Cell)
            } else {
                Ok(ScalarTy::Finite)
            }
        }
        IrType::Record { identity, name } => {
            if layout.records.contains_key(identity) {
                Ok(ScalarTy::Cell)
            } else {
                Err(format!(
                    "record values ({name}) have no canonical layout in this program"
                ))
            }
        }
        IrType::Named(name) if name == "bool" => Ok(ScalarTy::Bool),
        IrType::Named(name) => match BodyType::from_semantic_name(name) {
            BodyType::Integer(integer) if matches!(integer.bits, 8 | 16 | 32 | 64) => {
                Ok(ScalarTy::Int(integer))
            }
            // Bytes realize as unsigned 8-bit cells.
            BodyType::Byte => Ok(ScalarTy::Byte),
            // Exact sequences are canonical cells; bounded views are packed
            // descriptors riding one 64-bit cell.
            BodyType::Sequence {
                bound: mncs_model::SequenceBound::Exact(_),
                ..
            } => Ok(ScalarTy::Cell),
            BodyType::Sequence {
                bound: mncs_model::SequenceBound::UpTo(_),
                ..
            } => Ok(ScalarTy::View),
            BodyType::Vector { .. } => Ok(ScalarTy::Cell),
            BodyType::Mask { lanes } => Ok(ScalarTy::Mask(lanes)),
            _ => Err(format!("unsupported SSA type {name}")),
        },
    }
}

pub fn abi_bits(ty: ScalarTy) -> u16 {
    match ty {
        // Cell references and packed view descriptors are full 64-bit values.
        ScalarTy::Cell | ScalarTy::View | ScalarTy::Mask(_) => 64,
        ScalarTy::Bool | ScalarTy::Finite => 32,
        ScalarTy::Byte => 8,
        ScalarTy::Int(integer) => integer.bits.clamp(32, 64),
    }
}

pub fn c_type(ty: ScalarTy) -> &'static str {
    match ty {
        // Cell references are unsigned byte offsets into the arena; view
        // descriptors pack offset and length into one unsigned word.
        ScalarTy::Cell | ScalarTy::View | ScalarTy::Mask(_) => "uint64_t",
        ScalarTy::Byte => "uint8_t",
        _ => match abi_bits(ty) {
            64 => "int64_t",
            _ => "int32_t",
        },
    }
}

pub fn llvm_type(ty: ScalarTy) -> String {
    match ty {
        ScalarTy::Cell | ScalarTy::View | ScalarTy::Mask(_) => "i64".to_owned(),
        ScalarTy::Bool | ScalarTy::Finite => "i32".to_owned(),
        ScalarTy::Byte => "i8".to_owned(),
        ScalarTy::Int(integer) => format!("i{}", integer.bits),
    }
}

/// Map a semantic body type into the IR view used by scalar lowering.
/// Nominal types keep their structural variants; scalars, bytes, and
/// bounded sequences travel as canonical semantic names.
fn ir_type_of(ty: &mncs_model::BodyType) -> IrType {
    match ty {
        mncs_model::BodyType::Finite { identity, name } => IrType::Finite {
            identity: identity.clone(),
            name: name.clone(),
        },
        mncs_model::BodyType::Record { identity, name } => IrType::Record {
            identity: identity.clone(),
            name: name.clone(),
        },
        other => IrType::Named(other.semantic_name()),
    }
}

/// Canonical slot width for a sequence element inside an exact cell.
pub fn slot_width_of(ty: ScalarTy) -> SlotWidth {
    match ty {
        ScalarTy::Int(integer) if integer.bits == 64 => SlotWidth::W64,
        ScalarTy::Cell | ScalarTy::View | ScalarTy::Mask(_) => SlotWidth::W64,
        _ => SlotWidth::W32,
    }
}

#[cfg(test)]
mod tests {
    use super::{scalar_ty_in, CompositeLayout};
    use mncs_model::{IrType, SemanticId};

    #[test]
    fn record_values_without_a_declared_layout_fail_closed() {
        let record = IrType::Record {
            identity: SemanticId("mncs:0.2:record-type:m::R".to_owned()),
            name: "R".to_owned(),
        };
        let error =
            scalar_ty_in(&record, &CompositeLayout::default()).expect_err("must fail closed");
        assert!(error.contains("canonical layout"), "{error}");
        assert!(error.contains('R'), "diagnostic names the record type");
    }
}
