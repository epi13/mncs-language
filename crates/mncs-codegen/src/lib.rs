//! Replaceable portable backend for MNCS selected SSA.
//!
//! The language owns legality. This crate consumes an explicit target/backend
//! envelope and produces an identity-bound WASM MVP artifact. Execution of
//! that artifact is a research interpreter of the emitted subset, not a host
//! CLI and not a proof of compiler correctness.

mod lower;
mod wasm;

use std::collections::BTreeMap;

use mncs_model::{
    execute_ssa_module, execute_with_policy, ArtifactRepresentation, BackendArtifact,
    BackendConfiguration, BackendEvidence, BackendIdentity, BackendResult, CompilerArtifactRef,
    CompilerDiagnostic, CompilerDiagnosticKind, ExecutionCorpus, ExecutionFailure,
    ExecutionRequest, ExecutionResult, ExecutionStatus, ExecutionTarget, ExecutionValue,
    IntegerType, Program, SemanticId, SsaModule, TargetContractRef, TargetLoweringPlan,
    TransformationStatus, BACKEND_ARTIFACT_SCHEMA_VERSION, COMPILER_ARTIFACT_SCHEMA_VERSION,
    LAYERED_EXECUTION_COMPARISON_INTERPRETATION, PORTABLE_WASM_MVP_BACKEND_NAME,
    PORTABLE_WASM_MVP_BACKEND_VERSION, PORTABLE_WASM_MVP_TARGET, SSA_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};

use crate::lower::lower_module;
use crate::wasm::{decode_module, encode_module, execute_function, WASM_MAGIC, WASM_VERSION};

pub const PORTABLE_WASM_FORMAT: &str = "application/wasm; mncs-portable-wasm-mvp-0.1";
pub const BACKEND_EXECUTION_RESULT_SCHEMA_VERSION: &str = "0.1";
pub const LAYERED_EXECUTION_COMPARISON_SCHEMA_VERSION: &str = "0.1";

pub fn portable_wasm_backend() -> BackendIdentity {
    BackendIdentity::new(
        PORTABLE_WASM_MVP_BACKEND_NAME,
        PORTABLE_WASM_MVP_BACKEND_VERSION,
    )
}

pub fn portable_wasm_backend_configuration() -> BackendConfiguration {
    BackendConfiguration {
        backend: portable_wasm_backend(),
        options: BTreeMap::from([("format".to_owned(), PORTABLE_WASM_FORMAT.to_owned())]),
        target_features: [
            "mvp".to_owned(),
            "no-memory".to_owned(),
            "no-wasi".to_owned(),
        ]
        .into_iter()
        .collect(),
        linker_toolchain: None,
        assumptions: vec![
            "WASM MVP has no linear memory in this subset".to_owned(),
            "checked overflow traps as unreachable".to_owned(),
            "saturating and widening arithmetic are unsupported".to_owned(),
        ],
    }
}

pub fn portable_wasm_target() -> TargetContractRef {
    TargetContractRef::new(
        PORTABLE_WASM_MVP_TARGET,
        BTreeMap::from([
            (
                "data-layout".to_owned(),
                "scalar two's-complement; i8/i16/i32 in wasm i32; i64 in wasm i64; no aggregates"
                    .to_owned(),
            ),
            (
                "abi".to_owned(),
                "one exported wasm function per MNCS function; parameters/results only; no memory"
                    .to_owned(),
            ),
            (
                "integer".to_owned(),
                "wrapping=wasm wrapping; checked=explicit overflow test then trap".to_owned(),
            ),
            (
                "trap".to_owned(),
                "failure terminators and checked overflow map to wasm unreachable".to_owned(),
            ),
        ]),
        vec![SemanticId(
            "mncs:target-evidence:portable-wasm-mvp-0.1:declared-research-contract".to_owned(),
        )],
        vec![
            "portable WASM MVP is a research execution envelope, not a native ABI".to_owned(),
            "the embedded interpreter defines research execution, not a host wasm CLI".to_owned(),
        ],
    )
}

pub fn portable_wasm_plan(selected_ssa: CompilerArtifactRef) -> TargetLoweringPlan {
    TargetLoweringPlan::with_explicit_facts(
        selected_ssa,
        portable_wasm_target(),
        Some(portable_wasm_backend_configuration()),
        vec![
            "scalar two's-complement integers".to_owned(),
            "no aggregate or pointer layout".to_owned(),
        ],
        vec![
            "wasm function parameters and a single result".to_owned(),
            "no memory, tables, or imported host functions".to_owned(),
        ],
        BTreeMap::from([
            (
                "wrapping".to_owned(),
                "wasm wrapping add/sub/mul/and/or/xor".to_owned(),
            ),
            (
                "checked".to_owned(),
                "explicit overflow test then unreachable".to_owned(),
            ),
            (
                "unsupported".to_owned(),
                "saturating, widening, effects, runtime-check".to_owned(),
            ),
        ]),
        BTreeMap::from([
            ("overflow".to_owned(), "unreachable".to_owned()),
            ("failure_terminator".to_owned(), "unreachable".to_owned()),
        ]),
        Vec::new(),
        vec!["preserve public return and declared failure".to_owned()],
        TransformationStatus::Pass,
    )
}

pub fn target_is_portable_wasm(target: &TargetContractRef) -> bool {
    target.candidate == PORTABLE_WASM_MVP_TARGET
        && target.facts.contains_key("data-layout")
        && target.facts.contains_key("abi")
        && target.facts.contains_key("integer")
        && target.facts.contains_key("trap")
        && !target.evidence.is_empty()
}

pub fn lower_selected_ssa(
    program: &Program,
    ssa: &SsaModule,
    selected_ssa: CompilerArtifactRef,
    plan: &TargetLoweringPlan,
) -> BackendResult {
    let mut diagnostics = Vec::new();
    if selected_ssa.representation != ArtifactRepresentation::SelectedSsa
        || !selected_ssa.identity_is_valid()
    {
        diagnostics.push(CompilerDiagnostic::new(
            "CGN101",
            CompilerDiagnosticKind::InvalidRequest,
            "backend lowering requires an exact selected SSA identity",
        ));
        return failed(diagnostics);
    }
    let expected = ssa.fingerprint().unwrap_or_default();
    if selected_ssa.fingerprint != expected {
        diagnostics.push(CompilerDiagnostic::new(
            "CGN102",
            CompilerDiagnosticKind::InvalidRequest,
            "selected SSA fingerprint does not match the supplied SSA module",
        ));
        return failed(diagnostics);
    }
    if !target_is_portable_wasm(&plan.target) {
        diagnostics.push(CompilerDiagnostic::new(
            "CGN201",
            CompilerDiagnosticKind::MissingTargetEvidence,
            "portable WASM lowering requires explicit layout, ABI, integer, and trap facts",
        ));
        return BackendResult {
            status: TransformationStatus::Unknown,
            artifact: None,
            artifact_ref: None,
            evidence: None,
            diagnostics,
        };
    }
    if plan.status != TransformationStatus::Pass {
        diagnostics.push(CompilerDiagnostic::new(
            "CGN202",
            CompilerDiagnosticKind::MissingTargetEvidence,
            "target lowering plan is not PASS; the backend will not invent target facts",
        ));
        return BackendResult {
            status: TransformationStatus::Unknown,
            artifact: None,
            artifact_ref: None,
            evidence: None,
            diagnostics,
        };
    }
    let names = program
        .functions
        .iter()
        .map(|function| function.name.clone())
        .collect::<Vec<_>>();
    let outcome = lower_module(ssa, &names);
    if !outcome.unsupported.is_empty() || outcome.module.is_none() {
        diagnostics.push(CompilerDiagnostic {
            code: "CGN301".to_owned(),
            kind: CompilerDiagnosticKind::UnavailableBackendCapability,
            path: None,
            message: "selected SSA is outside the portable WASM MVP subset".to_owned(),
            related_artifacts: vec![selected_ssa.identity.clone()],
            obligations: Vec::new(),
        });
        return BackendResult {
            status: TransformationStatus::Unknown,
            artifact: None,
            artifact_ref: None,
            evidence: None,
            diagnostics: {
                let mut extra = diagnostics;
                for reason in outcome.unsupported {
                    extra.push(CompilerDiagnostic::new(
                        "CGN302",
                        CompilerDiagnosticKind::UnavailableBackendCapability,
                        reason,
                    ));
                }
                extra
            },
        };
    }
    let module = outcome.module.expect("checked above");
    let bytes = encode_module(&module);
    if decode_module(&bytes).is_err() {
        diagnostics.push(CompilerDiagnostic::new(
            "CGN303",
            CompilerDiagnosticKind::InternalCompilerDefect,
            "encoded WASM module could not be decoded by the research interpreter",
        ));
        return failed(diagnostics);
    }
    let backend = plan
        .backend
        .as_ref()
        .map(|configuration| configuration.backend.clone())
        .unwrap_or_else(portable_wasm_backend);
    let mut assumptions = plan.assumptions_introduced.clone();
    assumptions.extend(plan.target.assumptions.clone());
    let artifact = BackendArtifact::new(
        backend.clone(),
        selected_ssa.clone(),
        plan.target.clone(),
        PORTABLE_WASM_FORMAT,
        &bytes,
        outcome.exports,
        assumptions.clone(),
        Vec::new(),
        vec![
            "embedded mncs-portable-wasm-mvp interpreter".to_owned(),
            "no host CLI".to_owned(),
            "no WASI".to_owned(),
        ],
        plan.target.evidence.clone(),
        Vec::new(),
        TransformationStatus::Pass,
    );
    let artifact_ref = CompilerArtifactRef::new(
        ArtifactRepresentation::BackendArtifact,
        BACKEND_ARTIFACT_SCHEMA_VERSION,
        artifact.bytes_sha256.clone(),
    );
    let evidence = BackendEvidence::new(
        backend,
        selected_ssa,
        artifact_ref.clone(),
        assumptions,
        plan.target.evidence.clone(),
        TransformationStatus::Pass,
    );
    BackendResult {
        status: TransformationStatus::Pass,
        artifact: Some(artifact),
        artifact_ref: Some(artifact_ref),
        evidence: Some(evidence),
        diagnostics,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendExecutionResult {
    pub schema_version: String,
    pub status: ExecutionStatus,
    pub target: ExecutionTarget,
    pub backend: BackendIdentity,
    pub artifact_identity: Option<SemanticId>,
    pub artifact_sha256: Option<String>,
    pub returned: Vec<ExecutionValue>,
    pub failure: Option<ExecutionFailure>,
}

pub fn execute_backend(
    artifact: &BackendArtifact,
    request: &ExecutionRequest,
) -> BackendExecutionResult {
    let mut result = BackendExecutionResult {
        schema_version: BACKEND_EXECUTION_RESULT_SCHEMA_VERSION.to_owned(),
        status: ExecutionStatus::InvalidRequest,
        target: request.target.clone(),
        backend: artifact.backend.clone(),
        artifact_identity: Some(artifact.identity.clone()),
        artifact_sha256: Some(artifact.bytes_sha256.clone()),
        returned: Vec::new(),
        failure: None,
    };
    if !artifact.identity_is_valid() {
        result.failure = Some(ExecutionFailure {
            identity: Some(artifact.identity.clone()),
            reason: "backend artifact identity is stale or laundered".to_owned(),
        });
        return result;
    }
    let bytes = match artifact.wasm_bytes() {
        Ok(bytes) => bytes,
        Err(reason) => {
            result.failure = Some(ExecutionFailure {
                identity: Some(artifact.identity.clone()),
                reason,
            });
            return result;
        }
    };
    if bytes.len() < 8 || bytes[..4] != WASM_MAGIC || bytes[4..8] != WASM_VERSION {
        result.failure = Some(ExecutionFailure {
            identity: Some(artifact.identity.clone()),
            reason: "backend artifact is not a WASM MVP module".to_owned(),
        });
        return result;
    }
    let module = match decode_module(&bytes) {
        Ok(module) => module,
        Err(trap) => {
            result.status = trap.status;
            result.failure = Some(ExecutionFailure {
                identity: Some(artifact.identity.clone()),
                reason: trap.reason,
            });
            return result;
        }
    };
    match execute_function(
        &module,
        &request.target.function,
        &request.arguments,
        request.step_budget,
    ) {
        Ok(returned) => {
            result.status = ExecutionStatus::Returned;
            result.returned = returned;
        }
        Err(trap) => {
            result.status = trap.status;
            result.failure = Some(ExecutionFailure {
                identity: Some(artifact.identity.clone()),
                reason: trap.reason,
            });
        }
    }
    result
}

fn widen_to_type(value: i128, ty: IntegerType) -> i128 {
    let modulus = 1_i128 << ty.bits;
    let masked = value.rem_euclid(modulus);
    if ty.signed {
        let sign = 1_i128 << (ty.bits - 1);
        if masked >= sign {
            masked - modulus
        } else {
            masked
        }
    } else {
        masked
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayeredExecutionStatus {
    ConsistentOverCorpus,
    MismatchDetected,
    Unsupported,
    InvalidInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayeredExecutionMismatch {
    pub case_id: String,
    pub reason: String,
    pub body: ExecutionStatus,
    pub ssa: ExecutionStatus,
    pub backend: ExecutionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayeredExecutionComparison {
    pub schema_version: String,
    pub interpretation: String,
    pub status: LayeredExecutionStatus,
    pub matching_cases: usize,
    pub mismatching_cases: usize,
    pub mismatches: Vec<LayeredExecutionMismatch>,
}

pub fn compare_body_ssa_and_backend(
    program: &Program,
    ssa: &SsaModule,
    artifact: &BackendArtifact,
    corpus: &ExecutionCorpus,
) -> LayeredExecutionComparison {
    let mut matching = 0usize;
    let mut mismatching = 0usize;
    let mut mismatches = Vec::new();
    let mut unsupported = false;
    let mut invalid = corpus.cases.is_empty();
    for case_ in &corpus.cases {
        let body = execute_with_policy(program, &case_.request);
        let ssa_result = execute_ssa_module(program, ssa, &case_.request);
        let backend = execute_backend(artifact, &case_.request);
        unsupported |= matches!(body.status, ExecutionStatus::Unsupported)
            || matches!(ssa_result.status, ExecutionStatus::Unsupported)
            || matches!(backend.status, ExecutionStatus::Unsupported);
        invalid |= matches!(body.status, ExecutionStatus::InvalidRequest)
            || matches!(ssa_result.status, ExecutionStatus::InvalidRequest)
            || matches!(backend.status, ExecutionStatus::InvalidRequest);
        if observable_agree(&body, ssa_result.status, &ssa_result.returned, &backend) {
            matching += 1;
        } else {
            mismatching += 1;
            if mismatches.is_empty() {
                mismatches.push(LayeredExecutionMismatch {
                    case_id: case_.id.clone(),
                    reason: "body, SSA, and backend observations diverge over this corpus case"
                        .to_owned(),
                    body: body.status,
                    ssa: ssa_result.status,
                    backend: backend.status,
                });
            }
        }
    }
    let status = if invalid && matching == 0 {
        LayeredExecutionStatus::InvalidInput
    } else if mismatching > 0 {
        LayeredExecutionStatus::MismatchDetected
    } else if unsupported && matching == 0 {
        LayeredExecutionStatus::Unsupported
    } else {
        LayeredExecutionStatus::ConsistentOverCorpus
    };
    LayeredExecutionComparison {
        schema_version: LAYERED_EXECUTION_COMPARISON_SCHEMA_VERSION.to_owned(),
        interpretation: LAYERED_EXECUTION_COMPARISON_INTERPRETATION.to_owned(),
        status,
        matching_cases: matching,
        mismatching_cases: mismatching,
        mismatches,
    }
}

fn observable_agree(
    body: &ExecutionResult,
    ssa_status: ExecutionStatus,
    ssa_returned: &[ExecutionValue],
    backend: &BackendExecutionResult,
) -> bool {
    if body.status != ssa_status || ssa_status != backend.status {
        return false;
    }
    match body.status {
        ExecutionStatus::Returned => {
            body.returned == ssa_returned && values_agree(ssa_returned, &backend.returned)
        }
        ExecutionStatus::RuntimeFailure | ExecutionStatus::BudgetExhausted => true,
        ExecutionStatus::Unsupported | ExecutionStatus::InvalidRequest => false,
    }
}

fn values_agree(left: &[ExecutionValue], right: &[ExecutionValue]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .all(|(left, right)| match (left, right) {
            (
                ExecutionValue::Integer {
                    value: left_value,
                    ty: left_ty,
                },
                ExecutionValue::Integer {
                    value: right_value,
                    ty: right_ty,
                },
            ) => {
                left_ty.bits == right_ty.bits
                    && *left_value == widen_to_type(*right_value, *left_ty)
            }
            (ExecutionValue::Boolean { value: left }, ExecutionValue::Boolean { value: right }) => {
                left == right
            }
            (
                ExecutionValue::Boolean { value },
                ExecutionValue::Integer {
                    value: int_value, ..
                },
            )
            | (
                ExecutionValue::Integer {
                    value: int_value, ..
                },
                ExecutionValue::Boolean { value },
            ) => *int_value == i128::from(*value),
        })
}

fn failed(diagnostics: Vec<CompilerDiagnostic>) -> BackendResult {
    BackendResult {
        status: TransformationStatus::Fail,
        artifact: None,
        artifact_ref: None,
        evidence: None,
        diagnostics,
    }
}

pub fn selected_ssa_ref(ssa: &SsaModule) -> CompilerArtifactRef {
    CompilerArtifactRef::new(
        ArtifactRepresentation::SelectedSsa,
        SSA_SCHEMA_VERSION,
        ssa.fingerprint().expect("SSA is serializable"),
    )
}

pub fn plan_ref(plan: &TargetLoweringPlan) -> CompilerArtifactRef {
    let bytes = serde_json::to_vec(plan).expect("target lowering plan is serializable");
    CompilerArtifactRef::new(
        ArtifactRepresentation::TargetLoweringPlan,
        COMPILER_ARTIFACT_SCHEMA_VERSION,
        sha256_hex(&bytes),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;
    use mncs_model::{ExecutionCase, ExecutionPolicy, ExecutionTarget};

    fn program() -> Program {
        Program::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/executable/checked-add.mncs.json"
        )))
        .unwrap()
    }

    fn request(a: i128, b: i128) -> ExecutionRequest {
        ExecutionRequest {
            schema_version: mncs_model::EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
            target: ExecutionTarget {
                module: "Examples.Executable".to_owned(),
                function: "checked_add".to_owned(),
            },
            arguments: vec![
                ExecutionValue::Integer {
                    value: a,
                    ty: IntegerType {
                        bits: 32,
                        signed: true,
                    },
                },
                ExecutionValue::Integer {
                    value: b,
                    ty: IntegerType {
                        bits: 32,
                        signed: true,
                    },
                },
            ],
            step_budget: 64,
            policy: ExecutionPolicy::default(),
        }
    }

    #[test]
    fn checked_add_lowers_to_wasm_and_agrees_with_body_and_ssa() {
        let program = program();
        let ssa = program.lower_to_ssa().unwrap();
        let selected = selected_ssa_ref(&ssa);
        let plan = portable_wasm_plan(selected.clone());
        let result = lower_selected_ssa(&program, &ssa, selected, &plan);
        assert_eq!(result.status, TransformationStatus::Pass);
        let artifact = result.artifact.unwrap();
        assert!(artifact.bytes_hex.starts_with("0061736d01000000"));
        assert!(artifact.identity_is_valid());
        let executed = execute_backend(&artifact, &request(20, 22));
        assert_eq!(executed.status, ExecutionStatus::Returned);
        assert_eq!(
            executed.returned,
            vec![ExecutionValue::Integer {
                value: 42,
                ty: IntegerType {
                    bits: 32,
                    signed: true,
                },
            }]
        );
        let overflow = execute_backend(&artifact, &request(i128::from(i32::MAX), 1));
        assert_eq!(overflow.status, ExecutionStatus::RuntimeFailure);
        let corpus = ExecutionCorpus {
            schema_version: mncs_model::EXECUTION_CORPUS_SCHEMA_VERSION.to_owned(),
            name: "checked-add-backend".to_owned(),
            cases: vec![
                ExecutionCase {
                    id: "sum".to_owned(),
                    request: request(20, 22),
                },
                ExecutionCase {
                    id: "overflow".to_owned(),
                    request: request(i128::from(i32::MAX), 1),
                },
            ],
        };
        let comparison = compare_body_ssa_and_backend(&program, &ssa, &artifact, &corpus);
        assert_eq!(
            comparison.interpretation,
            LAYERED_EXECUTION_COMPARISON_INTERPRETATION
        );
        assert_eq!(
            comparison.status,
            LayeredExecutionStatus::ConsistentOverCorpus
        );
    }

    #[test]
    fn unevidenced_target_does_not_emit_a_backend_artifact() {
        let program = program();
        let ssa = program.lower_to_ssa().unwrap();
        let selected = selected_ssa_ref(&ssa);
        let plan = TargetLoweringPlan::with_unknown_facts(
            selected.clone(),
            TargetContractRef::new("mystery-target", BTreeMap::new(), Vec::new(), Vec::new()),
            None,
            vec!["data-layout evidence".to_owned()],
        );
        let result = lower_selected_ssa(&program, &ssa, selected, &plan);
        assert_eq!(result.status, TransformationStatus::Unknown);
        assert!(result.artifact.is_none());
    }

    #[test]
    fn malformed_wasm_is_not_executed_as_success() {
        let program = program();
        let ssa = program.lower_to_ssa().unwrap();
        let selected = selected_ssa_ref(&ssa);
        let plan = portable_wasm_plan(selected.clone());
        let mut artifact = lower_selected_ssa(&program, &ssa, selected, &plan)
            .artifact
            .unwrap();
        artifact.bytes_hex = "00".to_owned();
        let executed = execute_backend(&artifact, &request(1, 1));
        assert_ne!(executed.status, ExecutionStatus::Returned);
    }
}
