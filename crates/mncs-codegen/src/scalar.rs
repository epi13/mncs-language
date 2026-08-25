//! Backend-neutral scalar view of the currently supported selected-SSA subset.
//!
//! Adapters pretty-print this view. They do not reinterpret MNCS source.

use std::collections::BTreeMap;

use mncs_model::{
    ArithmeticIntent, BodyType, IntegerType, IrType, Program, SemanticId, SsaFunction,
    SsaInstruction, SsaInstructionKind, SsaModule, SsaTerminator, SsaValue,
};

use crate::promises::{
    integer_no_overflow_promise, non_arithmetic_promise_decisions, promise_summary, LoweringPromise,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarTy {
    Bool,
    Int(IntegerType),
    Finite,
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
    FiniteIsVariant {
        dest: ScalarValue,
        src: SemanticId,
        discriminant: u32,
    },
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

pub fn lower_to_scalar(_program: &Program, ssa: &SsaModule, names: &[String]) -> ScalarModule {
    let mut functions = Vec::new();
    let mut unsupported = Vec::new();
    let mut features = Vec::new();
    let mut promise_decisions = Vec::new();
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
        match lower_function(ssa, function, name, &callees) {
            Ok(lowered) => {
                features.extend(lowered.promises.iter().cloned());
                promise_decisions.extend(lowered.promise_decisions.iter().cloned());
                functions.push(lowered);
            }
            Err(reason) => unsupported.push(format!("{}: {reason}", function.identity.0)),
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

fn lower_function(
    module: &SsaModule,
    function: &SsaFunction,
    export_name: String,
    callees: &BTreeMap<SemanticId, String>,
) -> Result<ScalarFunction, String> {
    if function.outputs.len() != 1 {
        return Err("this realization requires exactly one result".to_owned());
    }
    let params = function
        .inputs
        .iter()
        .map(scalar_value)
        .collect::<Result<Vec<_>, _>>()?;
    let result = scalar_value(&function.outputs[0])?;
    let mut promises = Vec::new();
    let mut promise_decisions = Vec::new();
    let mut blocks = Vec::new();
    for block in &function.blocks {
        let params = block
            .parameters
            .iter()
            .map(scalar_value)
            .collect::<Result<Vec<_>, _>>()?;
        let mut insts = Vec::new();
        for instruction in &block.instructions {
            insts.push(lower_instruction(
                module,
                instruction,
                callees,
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
    promises: &mut Vec<String>,
    promise_decisions: &mut Vec<mncs_model::BackendPromiseDecision>,
) -> Result<ScalarInst, String> {
    let dest = instruction
        .outputs
        .first()
        .ok_or_else(|| "instruction has no destination".to_owned())?;
    let dest = scalar_value(dest)?;
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
            discriminant,
            payload_fields,
            ..
        } => {
            if !payload_fields.is_empty() {
                return Err(
                    "payload-bearing finite construction is outside the current scalar realization envelope"
                        .to_owned(),
                );
            }
            Ok(ScalarInst::FiniteConstruct {
                dest,
                discriminant: *discriminant,
            })
        }
        SsaInstructionKind::BooleanOp { operator } => Ok(ScalarInst::Boolean {
            dest,
            operator: operator.clone(),
            lhs: operand(instruction, 0)?,
            rhs: operand(instruction, 1)?,
        }),
        SsaInstructionKind::FinitePayloadProject { .. } => Err(
            "payload projection is outside the current scalar realization envelope".to_owned(),
        ),
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
        SsaInstructionKind::RecordConstruct { type_identity, .. } => Err(format!(
            "record values ({type_identity}) are unsupported by this scalar backend capability envelope"
        )),
        SsaInstructionKind::RecordProject { type_identity, .. } => Err(format!(
            "record values ({type_identity}) are unsupported by this scalar backend capability envelope"
        )),
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

fn scalar_value(value: &SsaValue) -> Result<ScalarValue, String> {
    Ok(ScalarValue {
        id: value.identity.clone(),
        ty: scalar_ty(&value.ty)?,
    })
}

pub fn scalar_ty(ty: &IrType) -> Result<ScalarTy, String> {
    match ty {
        IrType::Finite { .. } => Ok(ScalarTy::Finite),
        IrType::Record { name, .. } => Err(format!(
            "record values ({name}) are unsupported by this scalar backend capability envelope"
        )),
        IrType::Named(name) if name == "bool" => Ok(ScalarTy::Bool),
        IrType::Named(name) => match BodyType::from_semantic_name(name) {
            BodyType::Integer(integer) if matches!(integer.bits, 8 | 16 | 32 | 64) => {
                Ok(ScalarTy::Int(integer))
            }
            _ => Err(format!("unsupported SSA type {name}")),
        },
    }
}

pub fn abi_bits(ty: ScalarTy) -> u16 {
    match ty {
        ScalarTy::Bool | ScalarTy::Finite => 32,
        ScalarTy::Int(integer) => integer.bits.clamp(32, 64),
    }
}

pub fn c_type(ty: ScalarTy) -> &'static str {
    match abi_bits(ty) {
        64 => "int64_t",
        _ => "int32_t",
    }
}

pub fn llvm_type(ty: ScalarTy) -> String {
    match ty {
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
