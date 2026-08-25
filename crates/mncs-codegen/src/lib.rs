//! Replaceable backend adapters for MNCS selected SSA.
//!
//! The language owns legality. Adapters consume an explicit target/backend
//! envelope and produce identity-bound realizations. No adapter is the
//! definition of MNCS.

mod c11;
mod cranelift_backend;
mod llvm;
mod lower;
mod matrix;
mod native;
mod promises;
mod scalar;
mod support;
mod wasm;

use std::collections::BTreeMap;

use mncs_model::{
    execute_ssa_module, execute_with_policy, ArtifactRepresentation, BackendArtifact,
    BackendCapabilityManifest, BackendConfiguration, BackendEvidence, BackendFunctionValueContract,
    BackendIdentity, BackendResult, BackendValueContract, BodyType, CompilerArtifactRef,
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
pub const PORTABLE_WASM_ARTIFACT_KIND: &str = "wasm_module";
pub const RESEARCH_BYTECODE_BACKEND_NAME: &str = "mncs-research-bytecode";
pub const RESEARCH_BYTECODE_BACKEND_VERSION: &str = "0.1";
pub const RESEARCH_BYTECODE_TARGET: &str = "mncs:target:research-bytecode-0.1";
pub const RESEARCH_BYTECODE_FORMAT: &str =
    "application/vnd.mncs.research-bytecode+json; version=0.1";
pub const RESEARCH_BYTECODE_ARTIFACT_KIND: &str = "research_bytecode";
pub const BACKEND_EXECUTION_RESULT_SCHEMA_VERSION: &str = "0.1";
pub const LAYERED_EXECUTION_COMPARISON_SCHEMA_VERSION: &str = "0.1";

pub trait BackendAdapter {
    fn capabilities(&self) -> BackendCapabilityManifest;
    fn target(&self) -> TargetContractRef;
    fn configuration(&self) -> BackendConfiguration;
    fn plan(&self, selected_ssa: CompilerArtifactRef) -> TargetLoweringPlan;
    fn lower(
        &self,
        program: &Program,
        ssa: &SsaModule,
        selected_ssa: CompilerArtifactRef,
        plan: &TargetLoweringPlan,
    ) -> BackendResult;
    fn execute(
        &self,
        artifact: &BackendArtifact,
        request: &ExecutionRequest,
    ) -> BackendExecutionResult;
}

pub struct PortableWasmAdapter;
pub struct ResearchBytecodeAdapter;

pub use c11::{C11Adapter, C11_ARTIFACT_KIND, C11_BACKEND_NAME};
pub use cranelift_backend::{CraneliftAdapter, CRANELIFT_ARTIFACT_KIND, CRANELIFT_BACKEND_NAME};
pub use llvm::{LlvmAdapter, LLVM_ARTIFACT_KIND, LLVM_BACKEND_NAME};
pub use matrix::{backend_family_matrix, with_experiment_status, BackendFamilyMatrix};

pub fn backend_names() -> Vec<&'static str> {
    vec![
        PORTABLE_WASM_MVP_BACKEND_NAME,
        RESEARCH_BYTECODE_BACKEND_NAME,
        LLVM_BACKEND_NAME,
        C11_BACKEND_NAME,
        CRANELIFT_BACKEND_NAME,
    ]
}

pub fn backend_adapter(name: &str) -> Option<Box<dyn BackendAdapter>> {
    match name {
        PORTABLE_WASM_MVP_BACKEND_NAME | "portable-wasm" => Some(Box::new(PortableWasmAdapter)),
        RESEARCH_BYTECODE_BACKEND_NAME | "research-bytecode" => {
            Some(Box::new(ResearchBytecodeAdapter))
        }
        LLVM_BACKEND_NAME | "llvm" | "llvm-ir" => Some(Box::new(LlvmAdapter)),
        C11_BACKEND_NAME | "c11" | "portable-c" => Some(Box::new(C11Adapter)),
        CRANELIFT_BACKEND_NAME | "cranelift" => Some(Box::new(CraneliftAdapter)),
        _ => None,
    }
}

pub fn backend_capabilities(name: &str) -> Option<BackendCapabilityManifest> {
    backend_adapter(name).map(|adapter| adapter.capabilities())
}

pub fn target_for_backend(name: &str) -> Option<TargetContractRef> {
    backend_adapter(name).map(|adapter| adapter.target())
}

pub fn configuration_for_backend(name: &str) -> Option<BackendConfiguration> {
    backend_adapter(name).map(|adapter| adapter.configuration())
}

pub fn plan_for_backend(
    name: &str,
    selected_ssa: CompilerArtifactRef,
) -> Option<TargetLoweringPlan> {
    backend_adapter(name).map(|adapter| adapter.plan(selected_ssa))
}

pub fn lower_with_backend(
    name: &str,
    program: &Program,
    ssa: &SsaModule,
    selected_ssa: CompilerArtifactRef,
    plan: &TargetLoweringPlan,
) -> BackendResult {
    backend_adapter(name).map_or_else(
        || {
            failed(vec![CompilerDiagnostic::new(
                "CGN001",
                CompilerDiagnosticKind::UnavailableBackendCapability,
                format!("unknown backend adapter {name:?}"),
            )])
        },
        |adapter| adapter.lower(program, ssa, selected_ssa, plan),
    )
}

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
            "saturating uses explicit bounded-width clamp logic; widening uses exact i64 intermediates".to_owned(),
        ],
    }
}

pub fn portable_wasm_capabilities() -> BackendCapabilityManifest {
    BackendCapabilityManifest::new(
        portable_wasm_backend(),
        "mncs:selected-ssa-to-portable-wasm-mvp:0.1",
        [PORTABLE_WASM_MVP_TARGET.to_owned()].into_iter().collect(),
        [SSA_SCHEMA_VERSION.to_owned()].into_iter().collect(),
        ["data-layout", "abi", "integer", "trap"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        [
            "checked_integer",
            "wrapping_integer",
            "saturating_integer_bounded_width",
            "widening_integer_bounded_width",
            "explicit_failure",
            "semantic_bounded_iteration",
            "immutable_record_values_intra_function_only",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        [
            "memory",
            "effects",
            "saturating_integer_above_bounded_width",
            "widening_integer_above_64_bits",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        [PORTABLE_WASM_ARTIFACT_KIND.to_owned()]
            .into_iter()
            .collect(),
        ["bounded_execution_agreement".to_owned()]
            .into_iter()
            .collect(),
        ["embedded_mncs_wasm_interpreter".to_owned()]
            .into_iter()
            .collect(),
        ["scalar_parameter_result_abi".to_owned()]
            .into_iter()
            .collect(),
        "checked overflow and declared failure trap through WASM unreachable",
        true,
    )
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
                "wrapping=wasm wrapping; checked=explicit overflow test then trap; saturating=explicit clamp; widening=exact extended-width arithmetic".to_owned(),
            ),
            (
                "trap".to_owned(),
                "failure terminators and checked overflow map to wasm unreachable".to_owned(),
            ),
            (
                "bounded-iteration".to_owned(),
                "language-owned bounded SSA backedges use the private dispatcher loop; iteration metadata remains selected-SSA-bound".to_owned(),
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
            "bounded iteration metadata remains in the selected SSA identity".to_owned(),
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
                "saturating".to_owned(),
                "explicit bounded-width clamp".to_owned(),
            ),
            (
                "widening".to_owned(),
                "exact extended-width arithmetic".to_owned(),
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

pub fn research_bytecode_backend() -> BackendIdentity {
    BackendIdentity::new(
        RESEARCH_BYTECODE_BACKEND_NAME,
        RESEARCH_BYTECODE_BACKEND_VERSION,
    )
}

pub fn research_bytecode_configuration() -> BackendConfiguration {
    BackendConfiguration {
        backend: research_bytecode_backend(),
        options: BTreeMap::from([("encoding".to_owned(), "canonical-json".to_owned())]),
        target_features: ["bounded-ssa-interpreter".to_owned()].into_iter().collect(),
        linker_toolchain: None,
        assumptions: vec![
            "execution uses the bounded MNCS SSA interpreter".to_owned(),
            "artifact serialization is not native machine code".to_owned(),
        ],
    }
}

pub fn research_bytecode_capabilities() -> BackendCapabilityManifest {
    BackendCapabilityManifest::new(
        research_bytecode_backend(),
        "mncs:selected-ssa-to-research-bytecode:0.1",
        [RESEARCH_BYTECODE_TARGET.to_owned()].into_iter().collect(),
        [SSA_SCHEMA_VERSION.to_owned()].into_iter().collect(),
        ["serialization", "integer", "failure", "runtime"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        [
            "checked_integer",
            "wrapping_integer",
            "saturating_integer",
            "widening_integer",
            "explicit_failure",
            "bounded_control_flow",
            "semantic_bounded_iteration",
            "immutable_record_values",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        ["host_effects", "foreign_calls", "unbounded_execution"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        [RESEARCH_BYTECODE_ARTIFACT_KIND.to_owned()]
            .into_iter()
            .collect(),
        ["bounded_execution_agreement".to_owned()]
            .into_iter()
            .collect(),
        ["mncs_ssa_interpreter".to_owned()].into_iter().collect(),
        ["language_level_call_signature".to_owned()]
            .into_iter()
            .collect(),
        "preserves SSA returned values and explicit bounded failure observations",
        true,
    )
}

pub fn research_bytecode_target() -> TargetContractRef {
    TargetContractRef::new(
        RESEARCH_BYTECODE_TARGET,
        BTreeMap::from([
            (
                "serialization".to_owned(),
                "canonical JSON payload".to_owned(),
            ),
            (
                "integer".to_owned(),
                "MNCS SSA integer semantics".to_owned(),
            ),
            (
                "failure".to_owned(),
                "MNCS SSA explicit failure semantics".to_owned(),
            ),
            (
                "runtime".to_owned(),
                "bounded MNCS SSA interpreter".to_owned(),
            ),
            (
                "bounded-iteration".to_owned(),
                "preserve language-owned bounded iteration regions and backedges".to_owned(),
            ),
        ]),
        vec![SemanticId(
            "mncs:target-evidence:research-bytecode-0.1:declared-research-contract".to_owned(),
        )],
        vec!["research bytecode is an inspectable interpreter realization".to_owned()],
    )
}

pub fn research_bytecode_plan(selected_ssa: CompilerArtifactRef) -> TargetLoweringPlan {
    TargetLoweringPlan::with_explicit_facts(
        selected_ssa,
        research_bytecode_target(),
        Some(research_bytecode_configuration()),
        vec!["SSA value types remain explicit in the payload".to_owned()],
        vec!["language-level function signature".to_owned()],
        BTreeMap::from([(
            "integer".to_owned(),
            "execute exact SSA integer intent".to_owned(),
        )]),
        BTreeMap::from([(
            "failure".to_owned(),
            "retain explicit SSA failure status".to_owned(),
        )]),
        Vec::new(),
        vec!["preserve selected SSA without rewriting".to_owned()],
        TransformationStatus::Pass,
    )
}

pub fn target_is_research_bytecode(target: &TargetContractRef) -> bool {
    target.candidate == RESEARCH_BYTECODE_TARGET
        && ["serialization", "integer", "failure", "runtime"]
            .iter()
            .all(|fact| target.facts.contains_key(*fact))
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
    let names = ssa
        .functions
        .iter()
        .map(|ssa_function| {
            program
                .functions
                .iter()
                .find(|function| {
                    mncs_model::function_id(&program.module, &function.name)
                        == ssa_function.semantic_identity
                })
                .map(|function| function.name.clone())
                .unwrap_or_default()
        })
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
    let artifact = BackendArtifact::new_with_kind(
        backend.clone(),
        selected_ssa.clone(),
        plan.target.clone(),
        PORTABLE_WASM_ARTIFACT_KIND,
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
    )
    .with_function_value_contracts(function_value_contracts(program));
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
struct ResearchBytecodePayload {
    schema_version: String,
    program: Program,
    ssa: SsaModule,
}

pub fn lower_research_bytecode(
    program: &Program,
    ssa: &SsaModule,
    selected_ssa: CompilerArtifactRef,
    plan: &TargetLoweringPlan,
) -> BackendResult {
    let mut diagnostics = Vec::new();
    if selected_ssa.representation != ArtifactRepresentation::SelectedSsa
        || !selected_ssa.identity_is_valid()
        || selected_ssa.fingerprint != ssa.fingerprint().unwrap_or_default()
    {
        diagnostics.push(CompilerDiagnostic::new(
            "CGR101",
            CompilerDiagnosticKind::InvalidRequest,
            "research bytecode requires the exact selected SSA identity",
        ));
        return failed(diagnostics);
    }
    if !target_is_research_bytecode(&plan.target) {
        diagnostics.push(CompilerDiagnostic::new(
            "CGR201",
            CompilerDiagnosticKind::MissingTargetEvidence,
            "research bytecode requires serialization, integer, failure, and runtime facts",
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
            "CGR202",
            CompilerDiagnosticKind::MissingTargetEvidence,
            "research bytecode will not realize a non-PASS target plan",
        ));
        return BackendResult {
            status: TransformationStatus::Unknown,
            artifact: None,
            artifact_ref: None,
            evidence: None,
            diagnostics,
        };
    }
    let payload = ResearchBytecodePayload {
        schema_version: "0.1".to_owned(),
        program: program.clone(),
        ssa: ssa.clone(),
    };
    let bytes = match serde_json::to_vec(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            diagnostics.push(CompilerDiagnostic::new(
                "CGR301",
                CompilerDiagnosticKind::InternalCompilerDefect,
                format!("research bytecode serialization failed: {error}"),
            ));
            return failed(diagnostics);
        }
    };
    let backend = research_bytecode_backend();
    let assumptions = plan.assumptions_introduced.clone();
    let artifact = BackendArtifact::new_with_kind(
        backend.clone(),
        selected_ssa.clone(),
        plan.target.clone(),
        RESEARCH_BYTECODE_ARTIFACT_KIND,
        RESEARCH_BYTECODE_FORMAT,
        &bytes,
        program
            .functions
            .iter()
            .map(|function| function.name.clone())
            .collect(),
        assumptions.clone(),
        Vec::new(),
        vec![
            "bounded MNCS SSA interpreter".to_owned(),
            "no host process execution".to_owned(),
        ],
        plan.target.evidence.clone(),
        Vec::new(),
        TransformationStatus::Pass,
    )
    .with_function_value_contracts(function_value_contracts(program));
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
    pub steps: u64,
    pub effects: Vec<mncs_model::ExecutionEffectEvent>,
    pub failure: Option<ExecutionFailure>,
}

fn function_value_contracts(program: &Program) -> BTreeMap<String, BackendFunctionValueContract> {
    let value_contract = |name: &str| {
        program
            .finite_types
            .iter()
            .find(|finite_type| finite_type.name == name)
            .map_or_else(
                || BackendValueContract::Scalar {
                    semantic_type: name.to_owned(),
                },
                |finite_type| BackendValueContract::Finite {
                    type_identity: finite_type.identity.clone(),
                    variants: finite_type
                        .variants
                        .iter()
                        .map(|variant| (variant.discriminant, variant.identity.clone()))
                        .collect(),
                },
            )
    };
    program
        .functions
        .iter()
        .map(|function| {
            (
                function.name.clone(),
                BackendFunctionValueContract {
                    inputs: function
                        .inputs
                        .iter()
                        .map(|value| value_contract(&value.value_type))
                        .collect(),
                    outputs: function
                        .outputs
                        .iter()
                        .map(|value| value_contract(&value.value_type))
                        .collect(),
                },
            )
        })
        .collect()
}

fn execute_portable_wasm(
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
        steps: 0,
        effects: Vec::new(),
        failure: None,
    };
    if !artifact.identity_is_valid() {
        result.failure = Some(ExecutionFailure {
            identity: Some(artifact.identity.clone()),
            reason: "backend artifact identity is stale or laundered".to_owned(),
        });
        return result;
    }
    let bytes = match artifact.bytes() {
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
    let value_contract = artifact
        .function_value_contracts
        .get(&request.target.function);
    if let Some(contract) = value_contract {
        if contract.inputs.len() != request.arguments.len()
            || !contract
                .inputs
                .iter()
                .zip(&request.arguments)
                .all(|(contract, value)| backend_input_matches(contract, value))
        {
            result.failure = Some(ExecutionFailure {
                identity: Some(artifact.identity.clone()),
                reason: "backend request violates the language-owned value contract".to_owned(),
            });
            return result;
        }
    }
    match execute_function(
        &module,
        &request.target.function,
        &request.arguments,
        request.step_budget,
    ) {
        Ok(outcome) => {
            result.steps = outcome.steps;
            let returned = if let Some(contract) = value_contract {
                contract
                    .outputs
                    .iter()
                    .zip(outcome.returned)
                    .map(|(contract, value)| backend_output_value(contract, value))
                    .collect::<Result<Vec<_>, _>>()
            } else {
                Ok(outcome.returned)
            };
            match returned {
                Ok(returned) => {
                    result.status = ExecutionStatus::Returned;
                    result.returned = returned;
                }
                Err(reason) => {
                    result.failure = Some(ExecutionFailure {
                        identity: Some(artifact.identity.clone()),
                        reason,
                    });
                }
            }
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

fn backend_input_matches(contract: &BackendValueContract, value: &ExecutionValue) -> bool {
    match (contract, value) {
        (BackendValueContract::Scalar { semantic_type }, value) => {
            match (BodyType::from_semantic_name(semantic_type), value) {
                (BodyType::Named(name), ExecutionValue::Boolean { .. }) => name == "bool",
                (BodyType::Integer(expected), ExecutionValue::Integer { value, ty }) => {
                    expected == *ty && integer_value_fits(*value, expected)
                }
                _ => false,
            }
        }
        (
            BackendValueContract::Finite {
                type_identity,
                variants,
            },
            ExecutionValue::Finite {
                type_identity: actual_type,
                variant_identity,
                discriminant,
                ..
            },
        ) => type_identity == actual_type && variants.get(discriminant) == Some(variant_identity),
        _ => false,
    }
}

pub(crate) fn backend_output_value(
    contract: &BackendValueContract,
    value: ExecutionValue,
) -> Result<ExecutionValue, String> {
    match contract {
        BackendValueContract::Scalar { semantic_type } => {
            match (BodyType::from_semantic_name(semantic_type), value) {
                (BodyType::Named(name), ExecutionValue::Integer { value, .. })
                    if name == "bool" && (value == 0 || value == 1) =>
                {
                    Ok(ExecutionValue::Boolean { value: value == 1 })
                }
                (BodyType::Integer(expected), ExecutionValue::Integer { value, .. }) => {
                    let value = reinterpret_backend_value(value, expected);
                    if !integer_value_fits(value, expected) {
                        return Err(
                            "backend returned a scalar outside the language-owned integer type"
                                .to_owned(),
                        );
                    }
                    Ok(ExecutionValue::Integer {
                        value,
                        ty: expected,
                    })
                }
                _ => Err(
                    "backend scalar result violates the language-owned value contract".to_owned(),
                ),
            }
        }
        BackendValueContract::Finite {
            type_identity,
            variants,
        } => {
            let ExecutionValue::Integer { value, .. } = value else {
                return Err(
                    "backend finite result was not represented by an integer discriminant"
                        .to_owned(),
                );
            };
            let discriminant = u32::try_from(value).map_err(|_| {
                "backend returned a negative or oversized finite discriminant".to_owned()
            })?;
            let variant_identity = variants
                .get(&discriminant)
                .cloned()
                .ok_or_else(|| "backend returned an invalid finite discriminant".to_owned())?;
            Ok(ExecutionValue::Finite {
                type_identity: type_identity.clone(),
                variant_identity,
                discriminant,
                payload: Vec::new(),
            })
        }
    }
}

fn reinterpret_backend_value(value: i128, ty: IntegerType) -> i128 {
    if ty.signed {
        let modulus = 1_i128 << ty.bits;
        let sign = 1_i128 << (ty.bits - 1);
        if (sign..modulus).contains(&value) {
            value - modulus
        } else {
            value
        }
    } else if value < 0 && matches!(ty.bits, 32 | 64) {
        value + (1_i128 << ty.bits)
    } else {
        value
    }
}

fn integer_value_fits(value: i128, ty: IntegerType) -> bool {
    if ty.signed {
        let bound = 1_i128 << (ty.bits - 1);
        (-bound..bound).contains(&value)
    } else {
        value >= 0 && value < (1_i128 << ty.bits)
    }
}

fn execute_research_bytecode(
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
        steps: 0,
        effects: Vec::new(),
        failure: None,
    };
    if !artifact.identity_is_valid()
        || artifact.artifact_kind != RESEARCH_BYTECODE_ARTIFACT_KIND
        || artifact.backend != research_bytecode_backend()
    {
        result.failure = Some(ExecutionFailure {
            identity: Some(artifact.identity.clone()),
            reason: "artifact is not current research bytecode".to_owned(),
        });
        return result;
    }
    let bytes = match artifact.bytes() {
        Ok(bytes) => bytes,
        Err(reason) => {
            result.failure = Some(ExecutionFailure {
                identity: Some(artifact.identity.clone()),
                reason,
            });
            return result;
        }
    };
    let payload: ResearchBytecodePayload = match serde_json::from_slice(&bytes) {
        Ok(payload) => payload,
        Err(error) => {
            result.failure = Some(ExecutionFailure {
                identity: Some(artifact.identity.clone()),
                reason: format!("invalid research bytecode payload: {error}"),
            });
            return result;
        }
    };
    let observation = execute_ssa_module(&payload.program, &payload.ssa, request);
    result.status = observation.status;
    result.returned = observation.returned;
    result.steps = observation.steps;
    result.effects = observation.effects;
    result.failure = observation.failure;
    result
}

pub fn execute_backend(
    artifact: &BackendArtifact,
    request: &ExecutionRequest,
) -> BackendExecutionResult {
    backend_adapter(&artifact.backend.name).map_or_else(
        || BackendExecutionResult {
            schema_version: BACKEND_EXECUTION_RESULT_SCHEMA_VERSION.to_owned(),
            status: ExecutionStatus::Unsupported,
            target: request.target.clone(),
            backend: artifact.backend.clone(),
            artifact_identity: Some(artifact.identity.clone()),
            artifact_sha256: Some(artifact.bytes_sha256.clone()),
            returned: Vec::new(),
            steps: 0,
            effects: Vec::new(),
            failure: Some(ExecutionFailure {
                identity: Some(artifact.identity.clone()),
                reason: "no registered adapter can execute this backend artifact".to_owned(),
            }),
        },
        |adapter| adapter.execute(artifact, request),
    )
}

impl BackendAdapter for PortableWasmAdapter {
    fn capabilities(&self) -> BackendCapabilityManifest {
        portable_wasm_capabilities()
    }

    fn target(&self) -> TargetContractRef {
        portable_wasm_target()
    }

    fn configuration(&self) -> BackendConfiguration {
        portable_wasm_backend_configuration()
    }

    fn plan(&self, selected_ssa: CompilerArtifactRef) -> TargetLoweringPlan {
        portable_wasm_plan(selected_ssa)
    }

    fn lower(
        &self,
        program: &Program,
        ssa: &SsaModule,
        selected_ssa: CompilerArtifactRef,
        plan: &TargetLoweringPlan,
    ) -> BackendResult {
        lower_selected_ssa(program, ssa, selected_ssa, plan)
    }

    fn execute(
        &self,
        artifact: &BackendArtifact,
        request: &ExecutionRequest,
    ) -> BackendExecutionResult {
        execute_portable_wasm(artifact, request)
    }
}

impl BackendAdapter for ResearchBytecodeAdapter {
    fn capabilities(&self) -> BackendCapabilityManifest {
        research_bytecode_capabilities()
    }

    fn target(&self) -> TargetContractRef {
        research_bytecode_target()
    }

    fn configuration(&self) -> BackendConfiguration {
        research_bytecode_configuration()
    }

    fn plan(&self, selected_ssa: CompilerArtifactRef) -> TargetLoweringPlan {
        research_bytecode_plan(selected_ssa)
    }

    fn lower(
        &self,
        program: &Program,
        ssa: &SsaModule,
        selected_ssa: CompilerArtifactRef,
        plan: &TargetLoweringPlan,
    ) -> BackendResult {
        lower_research_bytecode(program, ssa, selected_ssa, plan)
    }

    fn execute(
        &self,
        artifact: &BackendArtifact,
        request: &ExecutionRequest,
    ) -> BackendExecutionResult {
        execute_research_bytecode(artifact, request)
    }
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
            (
                ExecutionValue::Finite {
                    type_identity: left_type,
                    variant_identity: left_variant,
                    discriminant: left_discriminant,
                    payload: left_payload,
                },
                ExecutionValue::Finite {
                    type_identity: right_type,
                    variant_identity: right_variant,
                    discriminant: right_discriminant,
                    payload: right_payload,
                },
            ) => {
                left_type == right_type
                    && left_variant == right_variant
                    && left_discriminant == right_discriminant
                    && record_values_agree(left_payload, right_payload)
            }
            (
                ExecutionValue::Record { fields: left, .. },
                ExecutionValue::Record { fields: right, .. },
            ) => record_values_agree(left, right),
            _ => false,
        })
}

/// Logical record observations agree when both sides carry the same canonical
/// field sequence (fields are kept sorted by name) and every field agrees
/// under the same scalar rules as top-level observations.
fn record_values_agree(
    left: &[(String, ExecutionValue)],
    right: &[(String, ExecutionValue)],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.0 == right.0
                && values_agree(
                    std::slice::from_ref(&left.1),
                    std::slice::from_ref(&right.1),
                )
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
            properties: Vec::new(),
            cases: vec![
                ExecutionCase {
                    id: "sum".to_owned(),
                    request: request(20, 22),
                    expected: None,
                    expected_status: None,
                    maximum_steps: None,
                    expected_effects: Vec::new(),
                    prohibit_unexpected_effects: false,
                },
                ExecutionCase {
                    id: "overflow".to_owned(),
                    request: request(i128::from(i32::MAX), 1),
                    expected: None,
                    expected_status: None,
                    maximum_steps: None,
                    expected_effects: Vec::new(),
                    prohibit_unexpected_effects: false,
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
    fn saturating_and_widening_arithmetic_agree_across_both_executable_backends() {
        let program = Program::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/executable/arithmetic-envelope.mncs.json"
        )))
        .expect("arithmetic envelope");
        assert!(program.validate().valid);
        let ssa = program.lower_to_ssa().expect("arithmetic SSA");
        let corpus: ExecutionCorpus = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/execution/arithmetic-envelope-corpus.json"
        )))
        .expect("arithmetic corpus");
        let selected = selected_ssa_ref(&ssa);
        for backend in [
            PORTABLE_WASM_MVP_BACKEND_NAME,
            RESEARCH_BYTECODE_BACKEND_NAME,
        ] {
            let plan = plan_for_backend(backend, selected.clone()).expect("backend plan");
            let artifact = lower_with_backend(backend, &program, &ssa, selected.clone(), &plan)
                .artifact
                .expect("arithmetic artifact");
            let comparison = compare_body_ssa_and_backend(&program, &ssa, &artifact, &corpus);
            assert_eq!(
                comparison.status,
                LayeredExecutionStatus::ConsistentOverCorpus,
                "{backend}: {comparison:#?}"
            );
            assert_eq!(comparison.matching_cases, 5);
            assert_eq!(comparison.mismatching_cases, 0);
        }
    }

    #[test]
    fn two_backend_adapters_share_selected_ssa_without_sharing_artifact_shape() {
        let program = program();
        let ssa = program.lower_to_ssa().unwrap();
        let selected = selected_ssa_ref(&ssa);
        let wasm = lower_with_backend(
            "portable-wasm",
            &program,
            &ssa,
            selected.clone(),
            &portable_wasm_plan(selected.clone()),
        )
        .artifact
        .unwrap();
        let bytecode = lower_with_backend(
            "research-bytecode",
            &program,
            &ssa,
            selected.clone(),
            &research_bytecode_plan(selected),
        )
        .artifact
        .unwrap();
        assert_eq!(wasm.input, bytecode.input);
        assert_ne!(wasm.backend, bytecode.backend);
        assert_eq!(wasm.artifact_kind, PORTABLE_WASM_ARTIFACT_KIND);
        assert_eq!(bytecode.artifact_kind, RESEARCH_BYTECODE_ARTIFACT_KIND);
        assert_ne!(wasm.identity, bytecode.identity);
        let wasm_result = execute_backend(&wasm, &request(20, 22));
        let bytecode_result = execute_backend(&bytecode, &request(20, 22));
        assert_eq!(wasm_result.status, ExecutionStatus::Returned);
        assert_eq!(wasm_result.returned, bytecode_result.returned);
    }

    #[test]
    fn backend_capabilities_are_identity_bound_and_non_wasm_shaped() {
        let wasm = portable_wasm_capabilities();
        let bytecode = research_bytecode_capabilities();
        assert!(wasm.identity_is_valid());
        assert!(bytecode.identity_is_valid());
        assert_ne!(wasm.backend, bytecode.backend);
        assert!(bytecode
            .artifact_kinds
            .contains(RESEARCH_BYTECODE_ARTIFACT_KIND));
        assert!(!bytecode
            .artifact_kinds
            .contains(PORTABLE_WASM_ARTIFACT_KIND));
        let matrix = backend_family_matrix();
        assert_eq!(matrix.backends.len(), 5);
        assert!(matrix
            .planned_unimplemented
            .iter()
            .any(|name| name.contains("spir-v")));
        for row in &matrix.backends {
            assert!(!row.artifact_kinds.is_empty());
            assert!(!row.required_target_facts.is_empty());
        }
    }

    fn lower_named(name: &str) -> mncs_model::BackendResult {
        let program = program();
        let ssa = program.lower_to_ssa().unwrap();
        let selected = selected_ssa_ref(&ssa);
        let plan = plan_for_backend(name, selected.clone()).unwrap();
        lower_with_backend(name, &program, &ssa, selected, &plan)
    }

    #[test]
    fn llvm_c11_and_cranelift_lower_checked_add() {
        for name in [LLVM_BACKEND_NAME, C11_BACKEND_NAME, CRANELIFT_BACKEND_NAME] {
            let result = lower_named(name);
            assert_eq!(result.status, TransformationStatus::Pass, "{name}");
            let artifact = result.artifact.unwrap();
            assert!(artifact.identity_is_valid());
            assert_ne!(artifact.artifact_kind, PORTABLE_WASM_ARTIFACT_KIND);
            let executed = execute_backend(&artifact, &request(20, 22));
            if executed.status == ExecutionStatus::Unsupported {
                let reason = executed
                    .failure
                    .as_ref()
                    .map(|failure| failure.reason.as_str())
                    .unwrap_or("");
                assert!(
                    reason.contains("unavailable toolchain")
                        || reason.contains("unsupported target")
                        || reason.contains("Cranelift"),
                    "{name}: {reason}"
                );
                continue;
            }
            assert_eq!(
                executed.status,
                ExecutionStatus::Returned,
                "{name}: {:?}",
                executed.failure
            );
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
            assert_eq!(overflow.status, ExecutionStatus::RuntimeFailure, "{name}");
        }
    }

    #[test]
    fn llvm_withholds_nsw_without_current_no_overflow_evidence() {
        let result = lower_named(LLVM_BACKEND_NAME);
        let artifact = result.artifact.unwrap();
        let ir = String::from_utf8(artifact.bytes().unwrap()).unwrap();
        assert!(ir.contains("llvm.sadd.with.overflow") || ir.contains("with.overflow"));
        assert!(artifact
            .assumptions
            .iter()
            .any(|assumption| assumption.contains("withheld")));
        assert!(!ir.contains(" add nsw i32"));
    }

    #[test]
    fn llvm_emits_nsw_only_with_a_recheckable_constant_range_certificate() {
        let program = Program::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/executable/verified-constant-add.mncs.json"
        )))
        .expect("constant program");
        assert!(program.validate().valid);
        let ssa = program.lower_to_ssa().expect("SSA");
        let selected = selected_ssa_ref(&ssa);
        let result = lower_with_backend(
            LLVM_BACKEND_NAME,
            &program,
            &ssa,
            selected.clone(),
            &plan_for_backend(LLVM_BACKEND_NAME, selected).unwrap(),
        );
        let artifact = result.artifact.expect("LLVM artifact");
        let ir = String::from_utf8(artifact.bytes().unwrap()).unwrap();
        assert!(ir.contains(" add nsw i32"), "{ir}");
        assert!(ir.contains(" add nuw i32"), "{ir}");
        let emitted = artifact
            .promise_decisions
            .iter()
            .find(|decision| decision.permitted)
            .expect("permitted promise decision");
        let certificate = emitted.certificate.as_ref().expect("certificate");
        assert!(certificate.identity_is_valid());
        let instruction = ssa.functions[0].blocks[0]
            .instructions
            .iter()
            .find(|instruction| {
                matches!(
                    instruction.kind,
                    mncs_model::SsaInstructionKind::Integer { .. }
                )
            })
            .expect("integer instruction");
        assert!(crate::promises::verify_constant_no_overflow_certificate(
            &ssa,
            instruction,
            certificate
        ));
        let mut stale = ssa.clone();
        stale.identity = SemanticId("mncs:stale:selected-ssa".to_owned());
        assert!(!crate::promises::verify_constant_no_overflow_certificate(
            &stale,
            instruction,
            certificate
        ));
        for promise in [
            mncs_model::BackendPromise::InBounds,
            mncs_model::BackendPromise::Alignment,
            mncs_model::BackendPromise::NonAliasing,
            mncs_model::BackendPromise::Relaxation,
        ] {
            assert!(artifact
                .promise_decisions
                .iter()
                .any(|decision| decision.promise == promise && !decision.permitted));
        }
        let mut tampered = artifact.clone();
        tampered.promise_decisions[0]
            .reason
            .push_str("; unauthorized mutation");
        assert!(!tampered.identity_is_valid());
    }

    #[test]
    fn artifact_and_backend_mismatch_is_not_returned() {
        let program = program();
        let ssa = program.lower_to_ssa().unwrap();
        let selected = selected_ssa_ref(&ssa);
        let mut artifact = lower_with_backend(
            LLVM_BACKEND_NAME,
            &program,
            &ssa,
            selected.clone(),
            &plan_for_backend(LLVM_BACKEND_NAME, selected).unwrap(),
        )
        .artifact
        .unwrap();
        artifact.backend = research_bytecode_backend();
        let executed = execute_backend(&artifact, &request(1, 1));
        assert_ne!(executed.status, ExecutionStatus::Returned);
        assert_eq!(executed.status, ExecutionStatus::InvalidRequest);
    }

    #[test]
    fn llvm_and_c11_reject_missing_target_facts() {
        let program = program();
        let ssa = program.lower_to_ssa().unwrap();
        let selected = selected_ssa_ref(&ssa);
        let plan = TargetLoweringPlan::with_unknown_facts(
            selected.clone(),
            TargetContractRef::new("mystery-target", BTreeMap::new(), Vec::new(), Vec::new()),
            None,
            vec!["data-layout evidence".to_owned()],
        );
        let llvm = crate::llvm::lower_llvm(&program, &ssa, selected.clone(), &plan);
        let c11 = crate::c11::lower_c11(&program, &ssa, selected, &plan);
        assert_eq!(llvm.status, TransformationStatus::Unknown);
        assert_eq!(c11.status, TransformationStatus::Unknown);
        assert!(llvm.artifact.is_none());
        assert!(c11.artifact.is_none());
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
        "00".clone_into(&mut artifact.bytes_hex);
        let executed = execute_backend(&artifact, &request(1, 1));
        assert_ne!(executed.status, ExecutionStatus::Returned);
    }
}

#[cfg(test)]
mod record_tests {
    use super::*;
    use mncs_model::{ExecutionPolicy, ExecutionTarget};

    fn record_program() -> Program {
        Program::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/executable/record-values.mncs.json"
        )))
        .unwrap()
    }

    fn record_request(value: i128) -> ExecutionRequest {
        ExecutionRequest {
            schema_version: mncs_model::EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
            target: ExecutionTarget {
                module: "examples.profile05_record_values".to_owned(),
                function: "main".to_owned(),
            },
            arguments: vec![ExecutionValue::Integer {
                value,
                ty: IntegerType {
                    bits: 32,
                    signed: true,
                },
            }],
            step_budget: 256,
            policy: ExecutionPolicy::default(),
        }
    }

    #[test]
    fn record_values_forward_through_wasm_and_agree_with_reference_execution() {
        let program = record_program();
        let ssa = program.lower_to_ssa().unwrap();
        let selected = selected_ssa_ref(&ssa);
        let plan = portable_wasm_plan(selected.clone());
        let result = lower_selected_ssa(&program, &ssa, selected, &plan);
        assert_eq!(result.status, TransformationStatus::Pass, "{result:?}");
        let artifact = result.artifact.expect("wasm artifact");
        for value in [0_i128, -5] {
            let wasm = execute_backend(&artifact, &record_request(value));
            assert_eq!(wasm.status, ExecutionStatus::Returned);
            // The reference (language-owned) execution is the semantic oracle.
            let reference = mncs_model::execute(&program, &record_request(value));
            assert_eq!(reference.status, ExecutionStatus::Returned);
            assert_eq!(wasm.returned, reference.returned, "value {value} agrees");
        }
        let overflow = execute_backend(&artifact, &record_request(i128::from(i32::MAX)));
        assert_eq!(overflow.status, ExecutionStatus::RuntimeFailure);
    }
}
