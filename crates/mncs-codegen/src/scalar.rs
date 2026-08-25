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
    Finite,
    /// A reference to a canonical arena cell (record or boxed finite
    /// variant). The value itself is the cell's byte offset.
    Cell,
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
    if !layout.records.is_empty() || !layout.boxed_finites.is_empty() {
        if functions
            .iter()
            .any(|function| function_uses_cells(function, &layout))
        {
            features.push("canonical_composite_cells".to_owned());
        }
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
                "add" | "sub" | "mul" | "div" | "mod" | "and" | "or" | "xor"
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
                return Err("payload field count does not match the declared variant layout".to_owned());
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
        SsaInstructionKind::FiniteIsVariant { discriminant, .. } => Ok(ScalarInst::FiniteIsVariant {
            dest,
            src: operand(instruction, 0)?,
            discriminant: *discriminant,
        }),
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
                if &entry.name != &field_names[index] {
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
            _ => Err(format!("unsupported SSA type {name}")),
        },
    }
}

/// Type mapping without a composite layout: records fail closed, boxed
/// finite detection is unavailable and finite types stay unboxed.
pub fn scalar_ty(ty: &IrType) -> Result<ScalarTy, String> {
    scalar_ty_in(ty, &CompositeLayout::default())
}

pub fn abi_bits(ty: ScalarTy) -> u16 {
    match ty {
        ScalarTy::Bool | ScalarTy::Finite | ScalarTy::Cell { .. } => 32,
        ScalarTy::Int(integer) => integer.bits.clamp(32, 64),
    }
}

pub fn c_type(ty: ScalarTy) -> &'static str {
    match ty {
        // Cell references are unsigned byte offsets into the arena.
        ScalarTy::Cell { .. } => "uint64_t",
        _ => match abi_bits(ty) {
            64 => "int64_t",
            _ => "int32_t",
        },
    }
}

pub fn llvm_type(ty: ScalarTy) -> String {
    match ty {
        ScalarTy::Cell { .. } => "i64".to_owned(),
        ScalarTy::Bool | ScalarTy::Finite => "i32".to_owned(),
        ScalarTy::Int(integer) => format!("i{}", integer.bits),
    }
}

#[cfg(test)]
mod tests {
    use super::scalar_ty;
    use mncs_model::{IrType, SemanticId};

    #[test]
    fn record_values_are_explicitly_unsupported_by_scalar_backends() {
        let record = IrType::Record {
            identity: SemanticId("mncs:0.2:record-type:m::R".to_owned()),
            name: "R".to_owned(),
        };
        let error = scalar_ty(&record).expect_err("records must fail closed");
        assert!(error.contains("unsupported"), "{error}");
        assert!(error.contains('R'), "diagnostic names the record type");
    }
}
