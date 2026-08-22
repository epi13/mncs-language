//! Backend-neutral scalar view of the currently supported selected-SSA subset.
//!
//! Adapters pretty-print this view. They do not reinterpret MNCS source.

use std::collections::BTreeMap;

use mncs_model::{
    ArithmeticIntent, BodyType, IntegerType, IrType, Program, SemanticId, SsaFunction,
    SsaInstruction, SsaInstructionKind, SsaModule, SsaTerminator, SsaValue,
};

use crate::promises::{integer_no_overflow_promise, promise_summary, LoweringPromise};

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
        promise: LoweringPromise,
    },
    Compare {
        dest: ScalarValue,
        predicate: String,
        operand: IntegerType,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarModule {
    pub functions: Vec<ScalarFunction>,
    pub unsupported: Vec<String>,
    pub features: Vec<String>,
}

pub fn lower_to_scalar(_program: &Program, ssa: &SsaModule, names: &[String]) -> ScalarModule {
    let mut functions = Vec::new();
    let mut unsupported = Vec::new();
    let mut features = Vec::new();
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
    })
}

fn lower_instruction(
    module: &SsaModule,
    instruction: &SsaInstruction,
    callees: &BTreeMap<SemanticId, String>,
    promises: &mut Vec<String>,
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
            if matches!(
                intent,
                ArithmeticIntent::Saturating | ArithmeticIntent::Widening { .. }
            ) {
                return Err(format!(
                    "arithmetic intent {intent:?} is outside the current scalar realization envelope"
                ));
            }
            if !matches!(
                operator.as_str(),
                "add" | "sub" | "mul" | "and" | "or" | "xor"
            ) {
                return Err(format!("unsupported integer operator {operator}"));
            }
            let promise = integer_no_overflow_promise(module, instruction);
            promises.push(promise_summary(&promise));
            Ok(ScalarInst::Integer {
                dest,
                operator: operator.clone(),
                intent: *intent,
                lhs: operand(instruction, 0)?,
                rhs: operand(instruction, 1)?,
                promise,
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
        SsaInstructionKind::FiniteConstruct { discriminant, .. } => {
            Ok(ScalarInst::FiniteConstruct {
                dest,
                discriminant: *discriminant,
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
