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
    BooleanOp {
        operator: String,
    },
    /// Byte bitwise op (Profile 0.7): `and` | `or` | `xor`.
    ByteBitwise {
        operator: String,
    },
    /// Byte shift (Profile 0.7): `shl` | `shr`; count modulo 8.
    ByteShift {
        operator: String,
    },
    /// Unsigned byte comparison (Profile 0.7).
    ByteCompare {
        predicate: String,
    },
    /// Semantic conditional selection (Profile 0.8). Both candidates are
    /// values of one pure selection; realization must not introduce
    /// data-dependent branch divergence between them.
    Select {
        operand_type: crate::BodyType,
    },
    /// Total functional sequence update over an exact bound (Profile 0.8):
    /// a new sequence equal to the source with one element replaced.
    SequenceReplace {
        element_type: crate::BodyType,
        bound: crate::SequenceBound,
        evidence: crate::BoundsEvidence,
    },
    VectorConstruct {
        element_type: crate::BodyType,
        lanes: u32,
    },
    VectorSplat {
        element_type: crate::BodyType,
        lanes: u32,
    },
    VectorExtract {
        element_type: crate::BodyType,
        lanes: u32,
        evidence: crate::BoundsEvidence,
    },
    VectorReplace {
        element_type: crate::BodyType,
        lanes: u32,
        evidence: crate::BoundsEvidence,
    },
    VectorBinary {
        operator: String,
        element_type: crate::BodyType,
        lanes: u32,
        intent: crate::ArithmeticIntent,
    },
    VectorCompare {
        predicate: String,
        element_type: crate::BodyType,
        lanes: u32,
    },
    MaskBinary {
        operator: String,
        lanes: u32,
    },
    MaskNot {
        lanes: u32,
    },
    MaskReduce {
        operator: String,
        lanes: u32,
    },
    VectorReduce {
        operator: String,
        element_type: crate::BodyType,
        lanes: u32,
        intent: crate::ArithmeticIntent,
    },
    /// Explicit total scalar conversion (Profile 0.7).
    Convert {
        from: crate::BodyType,
        to: crate::BodyType,
    },
    /// Construct an exact-length sequence (Profile 0.7).
    SequenceConstruct {
        element_type: Box<crate::BodyType>,
        length: u32,
    },
    /// Element projection with recorded bounds evidence (Profile 0.7).
    SequenceProject {
        bound: crate::SequenceBound,
        evidence: crate::BoundsEvidence,
    },
    /// Length observation as u64 (Profile 0.7). The declared bound rides
    /// with the instruction so every realization folds or observes it
    /// without rediscovery.
    SequenceLength {
        bound: crate::SequenceBound,
    },
    /// Checked bounded view construction over a half-open range (Profile 0.7).
    ViewConstruct {
        source_bound: crate::SequenceBound,
        view_bound: crate::SequenceBound,
    },
    FiniteConstruct {
        type_identity: SemanticId,
        variant_identity: SemanticId,
        discriminant: u32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        payload_fields: Vec<String>,
    },
    FinitePayloadProject {
        type_identity: SemanticId,
        variant_identity: SemanticId,
        discriminant: u32,
        field: String,
    },
    FiniteIsVariant {
        type_identity: SemanticId,
        variant_identity: SemanticId,
        discriminant: u32,
    },
    RecordConstruct {
        type_identity: SemanticId,
        field_names: Vec<String>,
    },
    RecordProject {
        type_identity: SemanticId,
        field: String,
    },
    Call {
        function: SemanticId,
        required_capabilities: Vec<SemanticId>,
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
    #[serde(default, skip_serializing_if = "crate::BodyCyclePolicy::is_legacy")]
    pub cycle_policy: crate::BodyCyclePolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bounded_iterations: Vec<SsaBoundedIteration>,
    pub blocks: Vec<SsaBlock>,
    pub failure: FailureMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaBoundedIteration {
    pub identity: SemanticId,
    pub bound: u32,
    /// What the iteration counts over; absent (legacy attempts) forms
    /// serialize unchanged.
    #[serde(default)]
    pub domain: crate::IterationDomain,
    pub state_type: IrType,
    pub preheader: SemanticId,
    pub header: SemanticId,
    pub body_entry: SemanticId,
    pub backedge: SemanticId,
    pub exit: SemanticId,
    pub body_blocks: Vec<SemanticId>,
    pub callees: Vec<SemanticId>,
    pub required_capabilities: Vec<SemanticId>,
    pub completion_modes: Vec<crate::BoundedIterationCompletion>,
    pub obligations: Vec<SemanticId>,
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

/// Logical record declarations carried into selected SSA so every backend
/// can compute its own representation without re-reading source.  Field
/// order is canonical (sorted by name); field types are leaf scalars,
/// finite types, or other declared records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaRecordType {
    pub identity: SemanticId,
    pub name: String,
    pub fields: Vec<(String, IrType)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaModule {
    pub schema_version: String,
    pub identity: SemanticId,
    pub semantic_identity: SemanticId,
    pub hir_fingerprint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub record_types: Vec<SsaRecordType>,
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
        let semantic_function_ids = self
            .functions
            .iter()
            .map(|function| function.semantic_identity.clone())
            .collect::<BTreeSet<_>>();
        for (function_index, function) in self.functions.iter().enumerate() {
            let path = format!("functions[{function_index}]");
            if !function_ids.insert(function.identity.clone()) {
                errors.push(diagnostic(
                    "SSA001",
                    &path,
                    "duplicate SSA function identity",
                ));
            }
            validate_function(
                function,
                &semantic_function_ids,
                &self.obligations,
                &path,
                &mut errors,
            );
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
            let semantic_function =
                function_id(function.identity_namespace(&self.module), &function.name);
            let Some(body) = &function.body else {
                functions.push(declaration_function(self, function, &semantic_function));
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
        let record_types = self
            .record_types
            .iter()
            .map(|record| SsaRecordType {
                identity: record.identity.clone(),
                name: record.name.clone(),
                fields: record
                    .fields
                    .iter()
                    .map(|field| {
                        let field_ir_type = if let Some(finite) = self
                            .finite_types
                            .iter()
                            .find(|item| item.name == field.field_type)
                        {
                            IrType::Finite {
                                identity: finite.identity.clone(),
                                name: field.field_type.clone(),
                            }
                        } else if let Some(nested) = self
                            .record_types
                            .iter()
                            .find(|item| item.name == field.field_type)
                        {
                            IrType::Record {
                                identity: nested.identity.clone(),
                                name: field.field_type.clone(),
                            }
                        } else {
                            IrType::Named(field.field_type.clone())
                        };
                        (field.name.clone(), field_ir_type)
                    })
                    .collect(),
            })
            .collect();
        let module = SsaModule {
            schema_version: SSA_SCHEMA_VERSION.to_owned(),
            identity,
            semantic_identity,
            hir_fingerprint,
            record_types,
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

fn declaration_function(
    program: &Program,
    function: &Function,
    semantic_function: &SemanticId,
) -> SsaFunction {
    let identity = ssa_identity("function", semantic_function, 0);
    let inputs = function
        .inputs
        .iter()
        .enumerate()
        .map(|(index, value)| SsaValue {
            identity: ssa_identity("input", semantic_function, index),
            semantic_identity: None,
            ty: ir_type_from_semantic(program, &value.value_type),
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
        cycle_policy: crate::BodyCyclePolicy::Legacy,
        bounded_iterations: Vec::new(),
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
    let home = function.identity_namespace(&program.module).to_owned();
    let semantic_function = function_id(&home, &function.name);
    let function_identity = ssa_identity("function", &semantic_function, 0);
    let mut value_ids = BTreeMap::<String, SemanticId>::new();
    let inputs = body
        .parameters
        .iter()
        .map(|parameter| {
            let identity = parameter_id(&home, &function.name, &parameter.id);
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
        let semantic_block = body.block_identity(&home, &function.name, &body_block.id);
        let block_identity = ssa_identity("block", &semantic_block, 0);
        let parameters = body_block
            .parameters
            .iter()
            .map(|parameter| {
                let identity = crate::identity::value_id(
                    &home,
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
            let operation_identity = body_operation.identity(&home, &function.name, &body_block.id);
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
                        &home,
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
                &body.block_identity(&home, &function.name, target),
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
    let bounded_iterations = body
        .bounded_iterations
        .iter()
        .map(|iteration| {
            let identity = crate::identity::iteration_id(&home, &function.name, &iteration.id);
            let block_identity = |block: &str| {
                ssa_identity(
                    "block",
                    &body.block_identity(&home, &function.name, block),
                    0,
                )
            };
            let hir_iteration = hir_function
                .bounded_iterations
                .iter()
                .find(|candidate| candidate.identity == identity);
            SsaBoundedIteration {
                domain: iteration.domain.clone(),
                identity,
                bound: iteration.bound,
                state_type: body_type(&iteration.state_type),
                preheader: block_identity(&iteration.preheader),
                header: block_identity(&iteration.header),
                body_entry: block_identity(&iteration.body_entry),
                backedge: block_identity(&iteration.backedge),
                exit: block_identity(&iteration.exit),
                body_blocks: iteration
                    .body_blocks
                    .iter()
                    .map(|block| block_identity(block))
                    .collect(),
                callees: iteration.callees.clone(),
                required_capabilities: hir_iteration
                    .map(|iteration| iteration.required_capabilities.clone())
                    .unwrap_or_default(),
                completion_modes: iteration.completion_modes.clone(),
                obligations: hir_iteration
                    .map(|iteration| iteration.obligations.clone())
                    .unwrap_or_default(),
            }
        })
        .collect();
    (
        SsaFunction {
            identity: function_identity,
            semantic_identity: semantic_function,
            inputs,
            outputs,
            cycle_policy: body.cycle_policy,
            bounded_iterations,
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
    semantic_function_ids: &BTreeSet<SemanticId>,
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
    let declared_backedges = function
        .bounded_iterations
        .iter()
        .map(|iteration| (iteration.backedge.clone(), iteration.header.clone()))
        .collect::<BTreeSet<_>>();
    if function.cycle_policy == crate::BodyCyclePolicy::BoundedIterationOnly {
        for (from, targets) in &successors {
            for target in targets {
                if dominators.get(from).is_some_and(|set| set.contains(target))
                    && !declared_backedges.contains(&(from.clone(), target.clone()))
                {
                    errors.push(diagnostic(
                        "SSA018",
                        path,
                        "SSA cycle is not an exact semantically bounded iteration backedge",
                    ));
                }
            }
        }
    }
    for iteration in &function.bounded_iterations {
        match &iteration.domain {
            crate::IterationDomain::Attempts => {
                if !(1..=crate::SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND).contains(&iteration.bound) {
                    errors.push(diagnostic(
                        "SSA019",
                        path,
                        "SSA bounded iteration has an invalid Profile 0.4 ceiling",
                    ));
                }
            }
            crate::IterationDomain::OverSequence { .. } => {
                if iteration.bound > crate::MAX_SEQUENCE_BOUND {
                    errors.push(diagnostic(
                        "SSA019",
                        path,
                        "SSA sequence traversal exceeds the declared profile ceiling",
                    ));
                }
            }
        }
        if [
            &iteration.preheader,
            &iteration.header,
            &iteration.body_entry,
            &iteration.backedge,
            &iteration.exit,
        ]
        .iter()
        .any(|block| !block_ids.contains(*block))
        {
            errors.push(diagnostic(
                "SSA020",
                path,
                "SSA bounded iteration references a missing region block",
            ));
            continue;
        }
        let header = function
            .blocks
            .iter()
            .find(|block| block.identity == iteration.header);
        if !header.is_some_and(|block| {
            block.parameters.len() == 2
                && block.parameters[0].ty == iteration.state_type
                && block.parameters[1].ty == IrType::Named("u64".to_owned())
        }) {
            errors.push(diagnostic(
                "SSA021",
                path,
                "SSA bounded iteration carried state/counter shape is malformed",
            ));
        }
        let mut callees = BTreeSet::new();
        let mut capabilities = BTreeSet::new();
        for block in function
            .blocks
            .iter()
            .filter(|block| iteration.body_blocks.contains(&block.identity))
        {
            for instruction in &block.instructions {
                if let SsaInstructionKind::Call {
                    function: callee,
                    required_capabilities,
                } = &instruction.kind
                {
                    callees.insert(callee.clone());
                    capabilities.extend(required_capabilities.iter().cloned());
                }
            }
        }
        if callees != iteration.callees.iter().cloned().collect()
            || capabilities != iteration.required_capabilities.iter().cloned().collect()
        {
            errors.push(diagnostic(
                "SSA022",
                path,
                "SSA bounded iteration lost exact call/authority closure",
            ));
        }
        if iteration.obligations.len() != 6
            || iteration
                .obligations
                .iter()
                .any(|obligation| !obligations.iter().any(|item| item.identity == *obligation))
        {
            errors.push(diagnostic(
                "SSA023",
                path,
                "SSA bounded iteration lost its bound/resource/authority/state/completion obligations",
            ));
        }
    }
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
            if let SsaInstructionKind::Call {
                function: callee,
                required_capabilities,
            } = &instruction.kind
            {
                if !semantic_function_ids.contains(callee) {
                    errors.push(diagnostic(
                        "SSA016",
                        path,
                        "call SSA instruction targets an unknown semantic function identity",
                    ));
                }
                let required = required_capabilities.iter().collect::<BTreeSet<_>>();
                let used = instruction
                    .capability_uses
                    .iter()
                    .map(|capability_use| &capability_use.capability)
                    .collect::<BTreeSet<_>>();
                if required != used {
                    errors.push(diagnostic(
                        "SSA017",
                        path,
                        "call SSA instruction does not retain exact required capability uses",
                    ));
                }
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
        IrOperationKind::BooleanOp { operator } => SsaInstructionKind::BooleanOp {
            operator: operator.clone(),
        },
        IrOperationKind::ByteBitwise { operator } => SsaInstructionKind::ByteBitwise {
            operator: operator.clone(),
        },
        IrOperationKind::ByteShift { operator } => SsaInstructionKind::ByteShift {
            operator: operator.clone(),
        },
        IrOperationKind::ByteCompare { predicate } => SsaInstructionKind::ByteCompare {
            predicate: predicate.clone(),
        },
        IrOperationKind::Select { operand_type } => SsaInstructionKind::Select {
            operand_type: (**operand_type).clone(),
        },
        IrOperationKind::SequenceReplace {
            element_type,
            bound,
            evidence,
        } => SsaInstructionKind::SequenceReplace {
            element_type: element_type.as_ref().clone(),
            bound: *bound,
            evidence: evidence.clone(),
        },
        IrOperationKind::VectorConstruct {
            element_type,
            lanes,
        } => SsaInstructionKind::VectorConstruct {
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
        },
        IrOperationKind::VectorSplat {
            element_type,
            lanes,
        } => SsaInstructionKind::VectorSplat {
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
        },
        IrOperationKind::VectorExtract {
            element_type,
            lanes,
            evidence,
        } => SsaInstructionKind::VectorExtract {
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
            evidence: evidence.clone(),
        },
        IrOperationKind::VectorReplace {
            element_type,
            lanes,
            evidence,
        } => SsaInstructionKind::VectorReplace {
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
            evidence: evidence.clone(),
        },
        IrOperationKind::VectorBinary {
            operator,
            element_type,
            lanes,
            intent,
        } => SsaInstructionKind::VectorBinary {
            operator: operator.clone(),
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
            intent: *intent,
        },
        IrOperationKind::VectorCompare {
            predicate,
            element_type,
            lanes,
        } => SsaInstructionKind::VectorCompare {
            predicate: predicate.clone(),
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
        },
        IrOperationKind::MaskBinary { operator, lanes } => SsaInstructionKind::MaskBinary {
            operator: operator.clone(),
            lanes: *lanes,
        },
        IrOperationKind::MaskNot { lanes } => SsaInstructionKind::MaskNot { lanes: *lanes },
        IrOperationKind::MaskReduce { operator, lanes } => SsaInstructionKind::MaskReduce {
            operator: operator.clone(),
            lanes: *lanes,
        },
        IrOperationKind::VectorReduce {
            operator,
            element_type,
            lanes,
            intent,
        } => SsaInstructionKind::VectorReduce {
            operator: operator.clone(),
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
            intent: *intent,
        },
        IrOperationKind::Convert { from, to } => SsaInstructionKind::Convert {
            from: from.clone(),
            to: to.clone(),
        },
        IrOperationKind::SequenceConstruct {
            element_type,
            length,
        } => SsaInstructionKind::SequenceConstruct {
            element_type: element_type.clone(),
            length: *length,
        },
        IrOperationKind::SequenceProject { bound, evidence } => {
            SsaInstructionKind::SequenceProject {
                bound: *bound,
                evidence: evidence.clone(),
            }
        }
        IrOperationKind::SequenceLength { bound } => {
            SsaInstructionKind::SequenceLength { bound: *bound }
        }
        IrOperationKind::ViewConstruct {
            source_bound,
            view_bound,
        } => SsaInstructionKind::ViewConstruct {
            source_bound: *source_bound,
            view_bound: *view_bound,
        },
        IrOperationKind::RecordConstruct {
            type_identity,
            field_names,
        } => SsaInstructionKind::RecordConstruct {
            type_identity: type_identity.clone(),
            field_names: field_names.clone(),
        },
        IrOperationKind::RecordProject {
            type_identity,
            field,
        } => SsaInstructionKind::RecordProject {
            type_identity: type_identity.clone(),
            field: field.clone(),
        },
        IrOperationKind::FiniteConstruct {
            type_identity,
            variant_identity,
            discriminant,
            payload_fields,
        } => SsaInstructionKind::FiniteConstruct {
            type_identity: type_identity.clone(),
            variant_identity: variant_identity.clone(),
            discriminant: *discriminant,
            payload_fields: payload_fields.clone(),
        },
        IrOperationKind::FinitePayloadProject {
            type_identity,
            variant_identity,
            discriminant,
            field,
        } => SsaInstructionKind::FinitePayloadProject {
            type_identity: type_identity.clone(),
            variant_identity: variant_identity.clone(),
            discriminant: *discriminant,
            field: field.clone(),
        },
        IrOperationKind::FiniteIsVariant {
            type_identity,
            variant_identity,
            discriminant,
        } => SsaInstructionKind::FiniteIsVariant {
            type_identity: type_identity.clone(),
            variant_identity: variant_identity.clone(),
            discriminant: *discriminant,
        },
        IrOperationKind::Call {
            function,
            required_capabilities,
        } => SsaInstructionKind::Call {
            function: function.clone(),
            required_capabilities: required_capabilities.clone(),
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
        BodyOperationKind::BooleanOp { operator } => SsaInstructionKind::BooleanOp {
            operator: operator.clone(),
        },
        BodyOperationKind::ByteBitwise { operator } => SsaInstructionKind::ByteBitwise {
            operator: operator.clone(),
        },
        BodyOperationKind::ByteShift { operator } => SsaInstructionKind::ByteShift {
            operator: operator.clone(),
        },
        BodyOperationKind::ByteCompare { predicate } => SsaInstructionKind::ByteCompare {
            predicate: predicate.clone(),
        },
        BodyOperationKind::Select { operand_type } => SsaInstructionKind::Select {
            operand_type: (**operand_type).clone(),
        },
        BodyOperationKind::SequenceReplace {
            element_type,
            bound,
            evidence,
        } => SsaInstructionKind::SequenceReplace {
            element_type: element_type.as_ref().clone(),
            bound: *bound,
            evidence: evidence.clone(),
        },
        BodyOperationKind::VectorConstruct {
            element_type,
            lanes,
        } => SsaInstructionKind::VectorConstruct {
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
        },
        BodyOperationKind::VectorSplat {
            element_type,
            lanes,
        } => SsaInstructionKind::VectorSplat {
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
        },
        BodyOperationKind::VectorExtract {
            element_type,
            lanes,
            evidence,
        } => SsaInstructionKind::VectorExtract {
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
            evidence: evidence.clone(),
        },
        BodyOperationKind::VectorReplace {
            element_type,
            lanes,
            evidence,
        } => SsaInstructionKind::VectorReplace {
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
            evidence: evidence.clone(),
        },
        BodyOperationKind::VectorBinary {
            operator,
            element_type,
            lanes,
            intent,
        } => SsaInstructionKind::VectorBinary {
            operator: operator.clone(),
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
            intent: *intent,
        },
        BodyOperationKind::VectorCompare {
            predicate,
            element_type,
            lanes,
        } => SsaInstructionKind::VectorCompare {
            predicate: predicate.clone(),
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
        },
        BodyOperationKind::MaskBinary { operator, lanes } => SsaInstructionKind::MaskBinary {
            operator: operator.clone(),
            lanes: *lanes,
        },
        BodyOperationKind::MaskNot { lanes } => SsaInstructionKind::MaskNot { lanes: *lanes },
        BodyOperationKind::MaskReduce { operator, lanes } => SsaInstructionKind::MaskReduce {
            operator: operator.clone(),
            lanes: *lanes,
        },
        BodyOperationKind::VectorReduce {
            operator,
            element_type,
            lanes,
            intent,
        } => SsaInstructionKind::VectorReduce {
            operator: operator.clone(),
            element_type: element_type.as_ref().clone(),
            lanes: *lanes,
            intent: *intent,
        },
        BodyOperationKind::Convert { from, to } => SsaInstructionKind::Convert {
            from: from.clone(),
            to: to.clone(),
        },
        BodyOperationKind::SequenceConstruct {
            element_type,
            length,
        } => SsaInstructionKind::SequenceConstruct {
            element_type: element_type.clone(),
            length: *length,
        },
        BodyOperationKind::SequenceProject { bound, evidence } => {
            SsaInstructionKind::SequenceProject {
                bound: *bound,
                evidence: evidence.clone(),
            }
        }
        BodyOperationKind::SequenceLength { bound } => {
            SsaInstructionKind::SequenceLength { bound: *bound }
        }
        BodyOperationKind::ViewConstruct {
            source_bound,
            view_bound,
        } => SsaInstructionKind::ViewConstruct {
            source_bound: *source_bound,
            view_bound: *view_bound,
        },
        BodyOperationKind::RecordConstruct {
            type_identity,
            field_names,
        } => SsaInstructionKind::RecordConstruct {
            type_identity: type_identity.clone(),
            field_names: field_names.clone(),
        },
        BodyOperationKind::RecordProject {
            type_identity,
            field,
        } => SsaInstructionKind::RecordProject {
            type_identity: type_identity.clone(),
            field: field.clone(),
        },
        BodyOperationKind::FiniteConstruct {
            type_identity,
            variant_identity,
            discriminant,
            payload_fields,
        } => SsaInstructionKind::FiniteConstruct {
            type_identity: type_identity.clone(),
            variant_identity: variant_identity.clone(),
            discriminant: *discriminant,
            payload_fields: payload_fields.clone(),
        },
        BodyOperationKind::FinitePayloadProject {
            type_identity,
            variant_identity,
            discriminant,
            field,
        } => SsaInstructionKind::FinitePayloadProject {
            type_identity: type_identity.clone(),
            variant_identity: variant_identity.clone(),
            discriminant: *discriminant,
            field: field.clone(),
        },
        BodyOperationKind::FiniteIsVariant {
            type_identity,
            variant_identity,
            discriminant,
        } => SsaInstructionKind::FiniteIsVariant {
            type_identity: type_identity.clone(),
            variant_identity: variant_identity.clone(),
            discriminant: *discriminant,
        },
        BodyOperationKind::Call {
            function,
            required_capabilities,
            ..
        } => SsaInstructionKind::Call {
            function: function.clone(),
            required_capabilities: required_capabilities
                .iter()
                .map(|capability| SemanticId(capability.clone()))
                .collect(),
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
        BodyOperationKind::SequenceProject {
            evidence: crate::BoundsEvidence::RuntimeChecked { failure },
            ..
        } => Some(failure.clone()),
        _ => None,
    }
}

fn body_type(ty: &crate::BodyType) -> IrType {
    match ty {
        crate::BodyType::Finite { identity, name } => IrType::Finite {
            identity: identity.clone(),
            name: name.clone(),
        },
        crate::BodyType::Record { identity, name } => IrType::Record {
            identity: identity.clone(),
            name: name.clone(),
        },
        _ => IrType::Named(ty.semantic_name()),
    }
}

fn ir_type_from_semantic(program: &Program, name: &str) -> IrType {
    program
        .finite_types
        .iter()
        .find(|finite_type| finite_type.name == name)
        .map_or_else(
            || IrType::Named(name.to_owned()),
            |finite_type| IrType::Finite {
                identity: finite_type.identity.clone(),
                name: finite_type.name.clone(),
            },
        )
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

    #[test]
    fn ssa_validation_rejects_an_unbounded_cycle_without_iteration_metadata() {
        let mut program = executable_program();
        program.functions[0]
            .body
            .as_mut()
            .expect("body")
            .cycle_policy = crate::BodyCyclePolicy::BoundedIterationOnly;
        let mut ssa = program.lower_to_ssa().expect("SSA");
        let entry = ssa.functions[0].blocks[0].identity.clone();
        ssa.functions[0].blocks[0].terminator = SsaTerminator::Branch {
            target: entry,
            arguments: Vec::new(),
        };
        let report = ssa.validate();
        assert!(!report.valid);
        assert!(report.errors.iter().any(|error| error.code == "SSA018"));
    }
}
