//! A deliberately small, deterministic, evidence-aware SSA artifact.
//!
//! This is an inspectable intermediate representation, not a backend and not
//! a proof that lowering is correct.  Block parameters provide the phi-like
//! join mechanism for the currently supported executable-body subset.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::canonical::sha256_hex;
use crate::identity::{function_id, parameter_id};
use crate::ir::{
    CapabilityUse, HighLevelIr, IrOperationKind, IrType, MachineIntentLinks, PathKind,
};
use crate::provenance::{TargetIdentity, TransformationRecord};
use crate::{
    ArithmeticIntent, BodyOperationKind, BodyTerminator, EvidenceFreshness, FailureMode, Function,
    ObligationRecord, ObligationStatus, Program, SemanticId, VerifierResult,
};

pub const SSA_SCHEMA_VERSION: &str = "0.4";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaValue {
    pub identity: SemanticId,
    pub semantic_identity: Option<SemanticId>,
    pub ty: IrType,
    pub producer: Option<SemanticId>,
    pub block: SemanticId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SsaInstructionKind {
    Constant {
        value: i128,
        ty: IrType,
    },
    Integer {
        operator: String,
        operand_type: crate::IntegerType,
        intent: ArithmeticIntent,
    },
    IntegerCompare {
        predicate: String,
        operand_type: crate::IntegerType,
    },
    Effect,
    RuntimeCheck {
        obligation: SemanticId,
        fact: SemanticId,
        failure: FailureMode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaInstruction {
    pub identity: SemanticId,
    pub semantic_identity: Option<SemanticId>,
    pub hir_identity: Option<SemanticId>,
    pub kind: SsaInstructionKind,
    pub inputs: Vec<SemanticId>,
    pub outputs: Vec<SsaValue>,
    pub effects: Vec<SemanticId>,
    pub capability_uses: Vec<CapabilityUse>,
    pub contracts: Vec<SemanticId>,
    pub assumptions: Vec<SemanticId>,
    pub evidence: Vec<SemanticId>,
    pub machine_intent: Option<MachineIntentLinks>,
    pub obligations: Vec<SemanticId>,
    pub failure: Option<FailureMode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SsaTerminator {
    Return {
        values: Vec<SemanticId>,
    },
    Branch {
        target: SemanticId,
        arguments: Vec<SemanticId>,
    },
    ConditionalBranch {
        condition: SemanticId,
        then_target: SemanticId,
        then_arguments: Vec<SemanticId>,
        else_target: SemanticId,
        else_arguments: Vec<SemanticId>,
    },
    Failure {
        mode: FailureMode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaBlock {
    pub identity: SemanticId,
    pub semantic_identity: Option<SemanticId>,
    pub path: PathKind,
    pub parameters: Vec<SsaValue>,
    pub instructions: Vec<SsaInstruction>,
    pub terminator: SsaTerminator,
    pub failure_target: Option<SemanticId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaFunction {
    pub identity: SemanticId,
    pub semantic_identity: SemanticId,
    pub inputs: Vec<SsaValue>,
    pub outputs: Vec<SsaValue>,
    pub blocks: Vec<SsaBlock>,
    pub failure: FailureMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaTraceEntry {
    pub semantic_identity: SemanticId,
    pub hir_identities: Vec<SemanticId>,
    pub ssa_identities: Vec<SemanticId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaTraceMap {
    pub entries: Vec<SsaTraceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaModule {
    pub schema_version: String,
    pub identity: SemanticId,
    pub semantic_identity: SemanticId,
    pub hir_fingerprint: String,
    pub functions: Vec<SsaFunction>,
    pub obligations: Vec<ObligationRecord>,
    pub trace: SsaTraceMap,
    pub transformations: Vec<TransformationRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaValidationReport {
    pub valid: bool,
    pub errors: Vec<SsaDiagnostic>,
}

#[derive(Debug, Error)]
pub enum SsaError {
    #[error("cannot lower an invalid program to SSA")]
    InvalidProgram(crate::ValidationReport),
    #[error("high-level IR lowering failed: {0}")]
    Ir(#[from] crate::IrError),
    #[error("SSA artifact is invalid")]
    Invalid(SsaValidationReport),
}

impl SsaModule {
    pub fn canonical_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn fingerprint(&self) -> Result<String, serde_json::Error> {
        Ok(sha256_hex(self.canonical_json()?.as_bytes()))
    }

    pub fn validate(&self) -> SsaValidationReport {
        let mut errors = Vec::new();
        let mut function_ids = BTreeSet::new();
        for (function_index, function) in self.functions.iter().enumerate() {
            let path = format!("functions[{function_index}]");
            if !function_ids.insert(function.identity.clone()) {
                errors.push(diagnostic(
                    "SSA001",
                    &path,
                    "duplicate SSA function identity",
                ));
            }
            validate_function(function, &self.obligations, &path, &mut errors);
        }
        let mut trace_keys = BTreeSet::new();
        for (index, entry) in self.trace.entries.iter().enumerate() {
            if !trace_keys.insert(entry.semantic_identity.clone()) {
                errors.push(diagnostic(
                    "SSA014",
                    &format!("trace.entries[{index}]"),
                    "duplicate SSA trace semantic identity",
                ));
            }
            if entry.ssa_identities.is_empty() {
                errors.push(diagnostic(
                    "SSA015",
                    &format!("trace.entries[{index}]"),
                    "SSA trace entry must retain an SSA identity",
                ));
            }
        }
        SsaValidationReport {
            valid: errors.is_empty(),
            errors,
        }
    }

    /// Permit one deliberately narrow evidence-gated experiment: eliminating
    /// the checked failure behavior of one integer operation only when an
    /// exact current verifier PASS is bound to that operation and obligation.
    pub fn attempt_no_overflow_elision(
        &self,
        operation: &SemanticId,
        obligation: &SemanticId,
        evidence: &[VerifierResult],
    ) -> SsaTransformationDecision {
        let eligible = evidence.iter().any(|result| {
            result.obligation == *obligation
                && result.subject == *operation
                && result.scope == operation.0
                && result.status == ObligationStatus::Pass
                && result.freshness == EvidenceFreshness::Current
                && result.authority == crate::EvidenceAuthorityClass::Verifier
                && result.dependencies.contains(operation)
        });
        let mut transformed = self.clone();
        let mut output_identity = self.identity.clone();
        if eligible {
            for function in &mut transformed.functions {
                for block in &mut function.blocks {
                    for instruction in &mut block.instructions {
                        if instruction.semantic_identity.as_ref() == Some(operation)
                            && matches!(
                                instruction.kind,
                                SsaInstructionKind::Integer {
                                    intent: ArithmeticIntent::Checked,
                                    ..
                                }
                            )
                        {
                            if let SsaInstructionKind::Integer { intent, .. } =
                                &mut instruction.kind
                            {
                                *intent = ArithmeticIntent::Wrapping;
                            }
                        }
                    }
                }
            }
            output_identity = SemanticId(format!(
                "mncs:0.4:ssa:transformed:{}",
                sha256_hex(format!("{}:{}:{}", self.identity, operation, obligation).as_bytes())
            ));
            transformed.identity = output_identity.clone();
        }
        let mut record = TransformationRecord::new(
            self.identity.clone(),
            output_identity,
            "evidence-gated-no-overflow-elision".to_owned(),
            operation.clone(),
            TargetIdentity {
                identity: SemanticId("mncs:0.4:target:portable".to_owned()),
                family: "portable".to_owned(),
                features: Vec::new(),
                integer_widths: vec![8, 16, 32, 64, 128],
                abi: None,
            },
            SemanticId("mncs:0.4:transformation:evidence-gated".to_owned()),
        );
        record.obligations_required.push(obligation.clone());
        if eligible {
            record
                .backend_promises_permitted
                .push("no-overflow".to_owned());
            if let Some(result) = evidence
                .iter()
                .find(|result| result.obligation == *obligation)
            {
                if let Some(artifact) = &result.artifact {
                    record.evidence_consumed.push(SemanticId(artifact.clone()));
                }
            }
        } else {
            record
                .backend_promises_withheld
                .push("no-overflow".to_owned());
        }
        SsaTransformationDecision {
            permitted: eligible,
            module: transformed,
            record,
            reason: if eligible {
                "exact current verifier PASS permits the bounded transformation".to_owned()
            } else {
                "missing exact current verifier PASS conservatively withholds the transformation"
                    .to_owned()
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaTransformationDecision {
    pub permitted: bool,
    pub module: SsaModule,
    pub record: TransformationRecord,
    pub reason: String,
}

impl Program {
    pub fn lower_to_ssa(&self) -> Result<SsaModule, SsaError> {
        let report = self.validate();
        if !report.valid {
            return Err(SsaError::InvalidProgram(report));
        }
        let ir = self.lower_to_ir()?;
        let semantic_identity = crate::identity::program_id(&self.module);
        let mut functions = Vec::new();
        let mut trace_entries = BTreeMap::<SemanticId, SsaTraceEntry>::new();
        let mut transformations = Vec::new();
        let mut semantic_functions = self.functions.iter().collect::<Vec<_>>();
        semantic_functions.sort_by(|left, right| left.name.cmp(&right.name));
        for function in semantic_functions {
            let semantic_function = function_id(&self.module, &function.name);
            let Some(body) = &function.body else {
                functions.push(declaration_function(function, &semantic_function));
                continue;
            };
            let hir_function = ir
                .functions
                .iter()
                .find(|candidate| candidate.semantic_identity == semantic_function)
                .expect("validated body has HIR function");
            let (lowered, entries, records) = lower_body(self, function, body, hir_function, &ir);
            functions.push(lowered);
            transformations.extend(records);
            for entry in entries {
                trace_entries
                    .entry(entry.semantic_identity.clone())
                    .and_modify(|existing| {
                        existing.hir_identities.extend(entry.hir_identities.clone());
                        existing.ssa_identities.extend(entry.ssa_identities.clone());
                        existing.hir_identities.sort();
                        existing.hir_identities.dedup();
                        existing.ssa_identities.sort();
                        existing.ssa_identities.dedup();
                    })
                    .or_insert(entry);
            }
        }
        let hir_fingerprint = ir.fingerprint().expect("HIR is serializable");
        let identity = SemanticId(format!(
            "mncs:0.4:ssa:module:{}",
            sha256_hex(format!("{}:{}", semantic_identity, hir_fingerprint).as_bytes())
        ));
        let module = SsaModule {
            schema_version: SSA_SCHEMA_VERSION.to_owned(),
            identity,
            semantic_identity,
            hir_fingerprint,
            functions,
            obligations: ir.obligations,
            trace: SsaTraceMap {
                entries: trace_entries.into_values().collect(),
            },
            transformations,
        };
        let validation = module.validate();
        if validation.valid {
            Ok(module)
        } else {
            Err(SsaError::Invalid(validation))
        }
    }
}

fn declaration_function(function: &Function, semantic_function: &SemanticId) -> SsaFunction {
    let identity = ssa_identity("function", semantic_function, 0);
    let inputs = function
        .inputs
        .iter()
        .enumerate()
        .map(|(index, value)| SsaValue {
            identity: ssa_identity("input", semantic_function, index),
            semantic_identity: None,
            ty: IrType::Named(value.value_type.clone()),
            producer: None,
            block: identity.clone(),
        })
        .collect::<Vec<_>>();
    let block_identity = ssa_identity("block", semantic_function, 0);
    SsaFunction {
        identity: identity.clone(),
        semantic_identity: semantic_function.clone(),
        inputs,
        outputs: Vec::new(),
        blocks: vec![SsaBlock {
            identity: block_identity,
            semantic_identity: None,
            path: PathKind::Normal,
            parameters: Vec::new(),
            instructions: Vec::new(),
            terminator: SsaTerminator::Return { values: Vec::new() },
            failure_target: None,
        }],
        failure: function.failure.clone(),
    }
}

fn lower_body(
    program: &Program,
    function: &Function,
    body: &crate::FunctionBody,
    hir_function: &crate::IrFunction,
    ir: &HighLevelIr,
) -> (SsaFunction, Vec<SsaTraceEntry>, Vec<TransformationRecord>) {
    let semantic_function = function_id(&program.module, &function.name);
    let function_identity = ssa_identity("function", &semantic_function, 0);
    let mut value_ids = BTreeMap::<String, SemanticId>::new();
    let inputs = body
        .parameters
        .iter()
        .map(|parameter| {
            let identity = parameter_id(&program.module, &function.name, &parameter.id);
            value_ids.insert(parameter.id.clone(), identity.clone());
            SsaValue {
                identity: identity.clone(),
                semantic_identity: Some(identity),
                ty: body_type(&parameter.ty),
                producer: None,
                block: function_identity.clone(),
            }
        })
        .collect::<Vec<_>>();
    let outputs = hir_function
        .outputs
        .iter()
        .cloned()
        .map(|value| SsaValue {
            identity: value.identity,
            semantic_identity: value.semantic_identity,
            ty: value.ty,
            producer: value.producer,
            block: function_identity.clone(),
        })
        .collect::<Vec<_>>();
    let error_identity = ssa_identity("failure", &semantic_function, 0);
    let mut blocks = Vec::new();
    let mut trace = Vec::new();
    let mut transformations = Vec::new();
    for body_block in &body.blocks {
        let semantic_block = body.block_identity(&program.module, &function.name, &body_block.id);
        let block_identity = ssa_identity("block", &semantic_block, 0);
        let parameters = body_block
            .parameters
            .iter()
            .map(|parameter| {
                let identity = crate::identity::value_id(
                    &program.module,
                    &function.name,
                    &body_block.id,
                    "parameter",
                    &parameter.id,
                );
                value_ids.insert(parameter.id.clone(), identity.clone());
                SsaValue {
                    identity: identity.clone(),
                    semantic_identity: Some(identity),
                    ty: body_type(&parameter.ty),
                    producer: None,
                    block: block_identity.clone(),
                }
            })
            .collect::<Vec<_>>();
        let mut instructions = Vec::new();
        for body_operation in &body_block.operations {
            let operation_identity =
                body_operation.identity(&program.module, &function.name, &body_block.id);
            let instruction_identity = ssa_identity("instruction", &operation_identity, 0);
            let hir_operation = hir_function
                .blocks
                .iter()
                .flat_map(|block| block.operations.iter())
                .find(|operation| {
                    operation.semantic_identity.as_ref() == Some(&operation_identity)
                });
            let inputs = body_operation
                .operands
                .iter()
                .filter_map(|operand| value_ids.get(operand).cloned())
                .collect::<Vec<_>>();
            let outputs = body_operation
                .results
                .iter()
                .map(|result| {
                    let identity = body_operation.result_identity(
                        &program.module,
                        &function.name,
                        &body_block.id,
                        &result.id,
                    );
                    value_ids.insert(result.id.clone(), identity.clone());
                    SsaValue {
                        identity: identity.clone(),
                        semantic_identity: Some(identity),
                        ty: body_type(&result.ty),
                        producer: Some(instruction_identity.clone()),
                        block: block_identity.clone(),
                    }
                })
                .collect::<Vec<_>>();
            let (
                kind,
                effects,
                capability_uses,
                machine_intent,
                obligations,
                contracts,
                assumptions,
                evidence,
            ) = if let Some(operation) = hir_operation {
                (
                    ssa_kind(&operation.kind),
                    operation.effects.clone(),
                    operation.capability_uses.clone(),
                    operation.machine_intent.clone(),
                    operation.obligations.clone(),
                    operation.contracts.clone(),
                    operation.assumptions.clone(),
                    operation.evidence.clone(),
                )
            } else {
                (
                    ssa_kind_from_body(&body_operation.kind),
                    Vec::new(),
                    Vec::new(),
                    None,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
            };
            let instruction = SsaInstruction {
                identity: instruction_identity.clone(),
                semantic_identity: Some(operation_identity.clone()),
                hir_identity: hir_operation.map(|operation| operation.identity.clone()),
                kind,
                inputs,
                outputs,
                effects,
                capability_uses,
                contracts,
                assumptions,
                evidence,
                machine_intent,
                obligations,
                failure: body_failure(&body_operation.kind),
            };
            if matches!(instruction.kind, SsaInstructionKind::Integer { .. })
                && instruction.machine_intent.is_none()
            {
                // HIR normally supplies this link. Keep the artifact honest
                // if a future compatible HIR omits it: validation rejects it.
            }
            let mut transformation = TransformationRecord::new(
                operation_identity.clone(),
                instruction_identity.clone(),
                "high-level-ir-to-ssa".to_owned(),
                operation_identity.clone(),
                TargetIdentity {
                    identity: SemanticId("mncs:0.4:target:portable".to_owned()),
                    family: "portable".to_owned(),
                    features: Vec::new(),
                    integer_widths: vec![8, 16, 32, 64, 128],
                    abi: None,
                },
                SemanticId("mncs:0.4:implementation:ssa-lowerer".to_owned()),
            );
            transformation
                .obligations_required
                .clone_from(&instruction.obligations);
            transformation
                .evidence_consumed
                .clone_from(&instruction.evidence);
            if matches!(instruction.kind, SsaInstructionKind::RuntimeCheck { .. }) {
                transformation
                    .runtime_checks_inserted
                    .push(instruction_identity.clone());
            }
            transformations.push(transformation);
            trace.push(SsaTraceEntry {
                semantic_identity: operation_identity,
                hir_identities: instruction.hir_identity.clone().into_iter().collect(),
                ssa_identities: vec![instruction_identity],
            });
            instructions.push(instruction);
        }
        let terminator = lower_terminator(&body_block.terminator, &value_ids, |target| {
            ssa_identity(
                "block",
                &body.block_identity(&program.module, &function.name, target),
                0,
            )
        });
        trace.push(SsaTraceEntry {
            semantic_identity: semantic_block.clone(),
            hir_identities: ir
                .trace_for(&semantic_block)
                .map(|entry| entry.ir_identities.clone())
                .unwrap_or_default(),
            ssa_identities: vec![block_identity.clone()],
        });
        blocks.push(SsaBlock {
            identity: block_identity,
            semantic_identity: Some(semantic_block),
            path: PathKind::Normal,
            parameters,
            instructions,
            terminator,
            failure_target: Some(error_identity.clone()),
        });
    }
    blocks.push(SsaBlock {
        identity: error_identity,
        semantic_identity: None,
        path: PathKind::Error,
        parameters: Vec::new(),
        instructions: Vec::new(),
        terminator: SsaTerminator::Failure {
            mode: function.failure.clone(),
        },
        failure_target: None,
    });
    (
        SsaFunction {
            identity: function_identity,
            semantic_identity: semantic_function,
            inputs,
            outputs,
            blocks,
            failure: function.failure.clone(),
        },
        trace,
        transformations,
    )
}

fn lower_terminator(
    terminator: &BodyTerminator,
    values: &BTreeMap<String, SemanticId>,
    target: impl Fn(&str) -> SemanticId,
) -> SsaTerminator {
    match terminator {
        BodyTerminator::Failure { mode } => SsaTerminator::Failure { mode: mode.clone() },
        BodyTerminator::Return { values: returned } => SsaTerminator::Return {
            values: returned
                .iter()
                .filter_map(|value| values.get(value).cloned())
                .collect(),
        },
        BodyTerminator::Branch {
            target: next,
            arguments,
        } => SsaTerminator::Branch {
            target: target(next),
            arguments: arguments
                .iter()
                .filter_map(|value| values.get(value).cloned())
                .collect(),
        },
        BodyTerminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        } => SsaTerminator::ConditionalBranch {
            condition: values
                .get(condition)
                .cloned()
                .unwrap_or_else(|| SemanticId("missing".to_owned())),
            then_target: target(then_target),
            then_arguments: then_arguments
                .iter()
                .filter_map(|value| values.get(value).cloned())
                .collect(),
            else_target: target(else_target),
            else_arguments: else_arguments
                .iter()
                .filter_map(|value| values.get(value).cloned())
                .collect(),
        },
    }
}

fn validate_function(
    function: &SsaFunction,
    obligations: &[ObligationRecord],
    path: &str,
    errors: &mut Vec<SsaDiagnostic>,
) {
    let block_ids = function
        .blocks
        .iter()
        .map(|block| block.identity.clone())
        .collect::<BTreeSet<_>>();
    if block_ids.len() != function.blocks.len() {
        errors.push(diagnostic(
            "SSA003",
            path,
            "SSA block identities must be unique",
        ));
    }
    let entry = function.blocks.first().map(|block| block.identity.clone());
    let mut successors = BTreeMap::<SemanticId, BTreeSet<SemanticId>>::new();
    for block in &function.blocks {
        let targets = match &block.terminator {
            SsaTerminator::Branch { target, .. } => vec![target.clone()],
            SsaTerminator::ConditionalBranch {
                then_target,
                else_target,
                ..
            } => {
                vec![then_target.clone(), else_target.clone()]
            }
            SsaTerminator::Return { .. } | SsaTerminator::Failure { .. } => Vec::new(),
        };
        let mut targets = targets;
        if let Some(failure_target) = &block.failure_target {
            targets.push(failure_target.clone());
        }
        for target in &targets {
            if !block_ids.contains(target) {
                errors.push(diagnostic(
                    "SSA004",
                    path,
                    "SSA terminator targets an unknown block",
                ));
            }
        }
        successors.insert(block.identity.clone(), targets.into_iter().collect());
    }
    let reachable = reachable_blocks(entry.as_ref(), &successors);
    for block in &function.blocks {
        if !reachable.contains(&block.identity) {
            errors.push(diagnostic(
                "SSA005",
                path,
                "unreachable SSA block is not supported",
            ));
        }
    }
    let dominators = dominators(entry.as_ref(), &reachable, &successors);
    let mut definitions = BTreeMap::<SemanticId, (Option<SemanticId>, IrType)>::new();
    for value in &function.inputs {
        if definitions
            .insert(value.identity.clone(), (None, value.ty.clone()))
            .is_some()
        {
            errors.push(diagnostic("SSA002", path, "duplicate SSA value definition"));
        }
    }
    for block in &function.blocks {
        for value in &block.parameters {
            if definitions
                .insert(
                    value.identity.clone(),
                    (Some(block.identity.clone()), value.ty.clone()),
                )
                .is_some()
            {
                errors.push(diagnostic("SSA002", path, "duplicate SSA value definition"));
            }
        }
        for instruction in &block.instructions {
            for value in &instruction.outputs {
                if definitions
                    .insert(
                        value.identity.clone(),
                        (Some(block.identity.clone()), value.ty.clone()),
                    )
                    .is_some()
                {
                    errors.push(diagnostic("SSA002", path, "duplicate SSA value definition"));
                }
            }
        }
    }
    let obligation_ids = obligations
        .iter()
        .map(|obligation| obligation.identity.clone())
        .collect::<BTreeSet<_>>();
    for block in &function.blocks {
        let mut available = function
            .inputs
            .iter()
            .map(|value| (value.identity.clone(), value.ty.clone()))
            .collect::<BTreeMap<_, _>>();
        for value in &block.parameters {
            available.insert(value.identity.clone(), value.ty.clone());
        }
        for (value, (definition_block, ty)) in &definitions {
            if definition_block.as_ref().is_some_and(|definition| {
                definition != &block.identity
                    && dominators
                        .get(&block.identity)
                        .is_some_and(|set| set.contains(definition))
            }) {
                available.insert(value.clone(), ty.clone());
            }
        }
        for instruction in &block.instructions {
            for input in &instruction.inputs {
                if !available.contains_key(input) {
                    errors.push(diagnostic(
                        "SSA006",
                        path,
                        "SSA operand is undefined or not dominated",
                    ));
                }
            }
            for obligation in &instruction.obligations {
                if !obligation_ids.contains(obligation) {
                    errors.push(diagnostic(
                        "SSA011",
                        path,
                        "SSA instruction references an unknown obligation",
                    ));
                }
            }
            if matches!(instruction.kind, SsaInstructionKind::Integer { .. }) {
                if instruction.machine_intent.is_none() {
                    errors.push(diagnostic(
                        "SSA012",
                        path,
                        "integer SSA instruction lost machine intent",
                    ));
                }
                if instruction.obligations.is_empty() {
                    errors.push(diagnostic(
                        "SSA013",
                        path,
                        "integer SSA instruction has no obligation link",
                    ));
                }
            }
            if matches!(instruction.kind, SsaInstructionKind::Effect)
                && instruction.capability_uses.is_empty()
            {
                errors.push(diagnostic(
                    "SSA010",
                    path,
                    "effect SSA instruction has no capability use",
                ));
            }
            for output in &instruction.outputs {
                available.insert(output.identity.clone(), output.ty.clone());
            }
        }
        validate_terminator(function, block, &available, &block_ids, path, errors);
    }
}

fn validate_terminator(
    function: &SsaFunction,
    block: &SsaBlock,
    available: &BTreeMap<SemanticId, IrType>,
    block_ids: &BTreeSet<SemanticId>,
    path: &str,
    errors: &mut Vec<SsaDiagnostic>,
) {
    let parameters = |target: &SemanticId| {
        function
            .blocks
            .iter()
            .find(|block| &block.identity == target)
            .map(|block| &block.parameters)
    };
    let check_edge =
        |target: &SemanticId, arguments: &[SemanticId], errors: &mut Vec<SsaDiagnostic>| {
            if !block_ids.contains(target) {
                return;
            }
            let Some(parameters) = parameters(target) else {
                return;
            };
            if parameters.len() != arguments.len() {
                errors.push(diagnostic(
                    "SSA007",
                    path,
                    "SSA branch argument arity does not match block parameters",
                ));
            }
            for (parameter, argument) in parameters.iter().zip(arguments) {
                if available.get(argument) != Some(&parameter.ty) {
                    errors.push(diagnostic(
                        "SSA008",
                        path,
                        "SSA branch argument type mismatch",
                    ));
                }
            }
        };
    match &block.terminator {
        SsaTerminator::Return { values } => {
            if values.len() != function.outputs.len() {
                errors.push(diagnostic("SSA009", path, "SSA return arity mismatch"));
            }
        }
        SsaTerminator::Branch { target, arguments } => check_edge(target, arguments, errors),
        SsaTerminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        } => {
            if !available.contains_key(condition) {
                errors.push(diagnostic(
                    "SSA006",
                    path,
                    "SSA branch condition is undefined",
                ));
            }
            check_edge(then_target, then_arguments, errors);
            check_edge(else_target, else_arguments, errors);
        }
        SsaTerminator::Failure { .. } => {}
    }
}

fn reachable_blocks(
    entry: Option<&SemanticId>,
    successors: &BTreeMap<SemanticId, BTreeSet<SemanticId>>,
) -> BTreeSet<SemanticId> {
    let mut reachable = BTreeSet::new();
    let Some(entry) = entry else {
        return reachable;
    };
    let mut queue = VecDeque::from([entry.clone()]);
    while let Some(block) = queue.pop_front() {
        if !reachable.insert(block.clone()) {
            continue;
        }
        if let Some(targets) = successors.get(&block) {
            queue.extend(targets.iter().cloned());
        }
    }
    reachable
}

fn dominators(
    entry: Option<&SemanticId>,
    reachable: &BTreeSet<SemanticId>,
    successors: &BTreeMap<SemanticId, BTreeSet<SemanticId>>,
) -> BTreeMap<SemanticId, BTreeSet<SemanticId>> {
    let mut predecessors = reachable
        .iter()
        .map(|block| (block.clone(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (from, targets) in successors {
        for target in targets {
            if reachable.contains(target) {
                predecessors
                    .entry(target.clone())
                    .or_default()
                    .insert(from.clone());
            }
        }
    }
    let Some(entry) = entry else {
        return BTreeMap::new();
    };
    let mut result = reachable
        .iter()
        .map(|block| {
            (
                block.clone(),
                if block == entry {
                    BTreeSet::from([block.clone()])
                } else {
                    reachable.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut changed = true;
    while changed {
        changed = false;
        for block in reachable.iter().filter(|block| *block != entry) {
            let mut predecessors = predecessors
                .get(block)
                .into_iter()
                .flat_map(BTreeSet::iter)
                .filter_map(|pred| result.get(pred));
            let Some(first) = predecessors.next() else {
                continue;
            };
            let mut next = first.clone();
            for set in predecessors {
                next = next.intersection(set).cloned().collect();
            }
            next.insert(block.clone());
            if result.get(block) != Some(&next) {
                result.insert(block.clone(), next);
                changed = true;
            }
        }
    }
    result
}

fn ssa_kind(kind: &IrOperationKind) -> SsaInstructionKind {
    match kind {
        IrOperationKind::Constant { value, ty } => SsaInstructionKind::Constant {
            value: *value,
            ty: ty.clone(),
        },
        IrOperationKind::Integer {
            operator,
            operand_type,
            intent,
        } => SsaInstructionKind::Integer {
            operator: operator.clone(),
            operand_type: *operand_type,
            intent: *intent,
        },
        IrOperationKind::IntegerCompare {
            predicate,
            operand_type,
        } => SsaInstructionKind::IntegerCompare {
            predicate: predicate.clone(),
            operand_type: *operand_type,
        },
        IrOperationKind::Effect => SsaInstructionKind::Effect,
        IrOperationKind::RuntimeCheck {
            obligation,
            fact,
            failure,
        } => SsaInstructionKind::RuntimeCheck {
            obligation: obligation.clone(),
            fact: fact.clone(),
            failure: failure.clone(),
        },
        _ => SsaInstructionKind::Effect,
    }
}

fn ssa_kind_from_body(kind: &BodyOperationKind) -> SsaInstructionKind {
    match kind {
        BodyOperationKind::Constant { value, ty } => SsaInstructionKind::Constant {
            value: *value,
            ty: body_type(ty),
        },
        BodyOperationKind::Integer {
            operator,
            operand_type,
            intent,
        } => SsaInstructionKind::Integer {
            operator: operator.clone(),
            operand_type: *operand_type,
            intent: *intent,
        },
        BodyOperationKind::IntegerCompare {
            predicate,
            operand_type,
        } => SsaInstructionKind::IntegerCompare {
            predicate: predicate.clone(),
            operand_type: *operand_type,
        },
        BodyOperationKind::Effect { .. } => SsaInstructionKind::Effect,
        BodyOperationKind::RuntimeCheck {
            obligation,
            fact,
            failure,
        } => SsaInstructionKind::RuntimeCheck {
            obligation: obligation.clone(),
            fact: fact.identity.clone(),
            failure: failure.clone(),
        },
    }
}

fn body_failure(kind: &BodyOperationKind) -> Option<FailureMode> {
    match kind {
        BodyOperationKind::RuntimeCheck { failure, .. } => Some(failure.clone()),
        _ => None,
    }
}

fn body_type(ty: &crate::BodyType) -> IrType {
    IrType::Named(ty.semantic_name())
}

fn ssa_identity(kind: &str, subject: &SemanticId, ordinal: usize) -> SemanticId {
    SemanticId(format!(
        "mncs:0.4:ssa:{kind}:{}",
        sha256_hex(format!("{subject}:{ordinal}").as_bytes())
    ))
}

fn diagnostic(code: &str, path: &str, message: &str) -> SsaDiagnostic {
    SsaDiagnostic {
        code: code.to_owned(),
        path: path.to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::tests::executable_program;
    use crate::MicroVerifier;

    #[test]
    fn executable_body_lowers_to_deterministic_valid_ssa_with_trace() {
        let program = executable_program();
        let first = program.lower_to_ssa().expect("SSA");
        let second = program.lower_to_ssa().expect("SSA");
        assert_eq!(first, second);
        assert!(first.validate().valid, "{:#?}", first.validate().errors);
        assert_eq!(first.schema_version, SSA_SCHEMA_VERSION);
        assert!(first
            .trace
            .entries
            .iter()
            .any(|entry| entry.ssa_identities.len() == 1));
        assert_eq!(first.fingerprint().unwrap(), second.fingerprint().unwrap());
    }

    #[test]
    fn ssa_preserves_machine_intent_and_rejects_unknown_stronger_transformation() {
        let program = executable_program();
        let ssa = program.lower_to_ssa().expect("SSA");
        let instruction = ssa.functions[0].blocks[0]
            .instructions
            .iter()
            .find(|instruction| matches!(instruction.kind, SsaInstructionKind::Integer { .. }))
            .expect("integer");
        let obligation = instruction.obligations[0].clone();
        let denied = ssa.attempt_no_overflow_elision(
            instruction.semantic_identity.as_ref().expect("semantic"),
            &obligation,
            &[],
        );
        assert!(!denied.permitted);
        assert!(denied.record.backend_promises_permitted.is_empty());
        assert_eq!(denied.module, ssa);
    }

    #[test]
    fn exact_current_verifier_pass_allows_only_the_bound_transformation() {
        let program = executable_program();
        let ssa = program.lower_to_ssa().expect("SSA");
        let instruction = ssa.functions[0].blocks[0]
            .instructions
            .iter()
            .find(|instruction| matches!(instruction.kind, SsaInstructionKind::Integer { .. }))
            .expect("integer");
        let operation = instruction
            .semantic_identity
            .clone()
            .expect("semantic operation");
        let obligation = instruction.obligations[0].clone();
        let mut evidence = VerifierResult {
            schema_version: "0.2".to_owned(),
            obligation: obligation.clone(),
            subject: operation.clone(),
            scope: operation.0.clone(),
            status: ObligationStatus::Pass,
            verifier: crate::DeterministicVerifier::default().identity().clone(),
            method: crate::VerifierMethod::IntegerRange,
            assumptions: Vec::new(),
            dependencies: vec![operation.clone()],
            dependency_fingerprints: BTreeMap::new(),
            artifact: Some("evidence://exact".to_owned()),
            limitations: Vec::new(),
            freshness: EvidenceFreshness::Current,
            authority: crate::EvidenceAuthorityClass::Verifier,
        };
        let allowed = ssa.attempt_no_overflow_elision(&operation, &obligation, &[evidence.clone()]);
        assert!(allowed.permitted);
        assert!(allowed
            .record
            .backend_promises_permitted
            .contains(&"no-overflow".to_owned()));
        evidence.freshness = EvidenceFreshness::Stale;
        let stale = ssa.attempt_no_overflow_elision(&operation, &obligation, &[evidence]);
        assert!(!stale.permitted);
    }

    #[test]
    fn ssa_validation_rejects_an_undefined_operand() {
        let program = executable_program();
        let mut ssa = program.lower_to_ssa().expect("SSA");
        ssa.functions[0].blocks[0].instructions[1]
            .inputs
            .push(SemanticId("mncs:missing-value".to_owned()));
        let report = ssa.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "SSA006"));
    }
}
