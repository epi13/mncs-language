//! A deliberately small, deterministic, evidence-aware SSA artifact.
//!
//! This is an inspectable intermediate representation, not a backend and not
//! a proof that lowering is correct.  Block parameters provide the phi-like
//! join mechanism for the currently supported executable-body subset.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::canonical::sha256_hex;
use crate::identity::{function_id, parameter_id};
use crate::ir::{CapabilityUse, HighLevelIr, IrOperationKind, MachineIntentLinks, PathKind};
use crate::provenance::{TargetIdentity, TransformationRecord};
use crate::{
    ArithmeticIntent, BodyOperationKind, BodyTerminator, EvidenceFreshness, FailureMode, Function,
    ObligationRecord, ObligationStatus, Program, SemanticId, VerifierResult,
};

pub const SSA_SCHEMA_VERSION: &str = "0.5";
/// The previous SSA schema version. Artifacts carrying `"0.4"` store
/// non-nominal types as `IrType::Named` spellings; they deserialize via
/// the `BodyType` tag aliases and must pass through
/// [`SsaModule::normalize_legacy_types`] before validation or execution.
pub const SSA_SCHEMA_VERSION_PRE_TYPED: &str = "0.4";
pub const SSA_FUNCTION_VERIFICATION_FACT_SCHEMA_VERSION: &str =
    "mncs.language-ssa-function-verification-fact/1";
pub const SSA_FUNCTION_VERIFICATION_FACT_VERIFIER_ID: &str =
    "mncs-language-ssa-function-validator/1";
pub const SSA_FUNCTION_VERIFICATION_FACT_POLICY_ID: &str = "mncs-language-ssa-function-policy/1";
pub const SSA_FUNCTION_VERIFICATION_LANGUAGE_PROFILE: &str = "mncs-language-ssa-profile/0.5";
pub const SSA_FUNCTION_ARTIFACT_CACHE_SCHEMA_VERSION: &str =
    "mncs.language-ssa-function-artifact-cache/1";
pub const SSA_FUNCTION_ARTIFACT_CACHE_OBSERVATION_SCHEMA_VERSION: &str =
    "mncs.language-ssa-function-artifact-cache-observation/1";
/// Bump when the transformation from a verified HIR function to its SSA
/// function changes without changing the SSA artifact schema.
pub const SSA_FUNCTION_LOWERER_IDENTITY: &str = "mncs-language-ssa-function-lowerer/1";
pub const SSA_FUNCTION_LOWERING_POLICY_IDENTITY: &str =
    "mncs-language-ssa-function-lowering-policy/1";

const MAX_SSA_FUNCTION_ARTIFACT_CACHE_ENTRIES: usize = 256;
const MAX_SSA_FUNCTION_ARTIFACT_CACHE_BYTES: usize = 32 * 1024 * 1024;

fn trace_timing(stage: &str, started: Instant) {
    crate::record_stage(stage, started.elapsed());
    if std::env::var_os("MNCS_TIMINGS").is_some() {
        eprintln!(
            "mncs-timing stage={} elapsed_ms={}",
            stage,
            started.elapsed().as_millis()
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaValue {
    pub identity: SemanticId,
    pub semantic_identity: Option<SemanticId>,
    pub ty: crate::BodyType,
    pub producer: Option<SemanticId>,
    pub block: SemanticId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SsaInstructionKind {
    Constant {
        value: i128,
        ty: crate::BodyType,
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
    FloatConstant {
        bits: u64,
        ty: crate::FloatType,
    },
    Float {
        operator: String,
    },
    FloatCompare {
        predicate: String,
    },
    FloatIntrinsic {
        function: String,
    },
    BooleanOp {
        operator: String,
    },
    /// Boolean equality comparison (CP-0004): `eq` | `ne` over bools.
    BooleanCompare {
        predicate: String,
    },
    /// Boolean negation (CP-0004) over one bool.
    BooleanNot,
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
    /// Checked index against a sequence length (Profile 0.14): the result
    /// is the candidate index, verified below the sequence length. The
    /// carried bound resolves the runtime length on every backend.
    BoundCheck {
        bound: crate::SequenceBound,
    },
    /// Total functional bounded span copy (Profile 0.14): a new sequence
    /// equal to the destination with one span replaced from the source.
    SequenceCopy {
        element_type: crate::BodyType,
        dst_bound: crate::SequenceBound,
        src_bound: crate::SequenceBound,
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
    /// Checked view narrowing to a smaller static capacity (Profile 0.14).
    ViewNarrow {
        source_cap: u32,
        new_cap: u32,
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
    /// A value-producing host-realized operation. Reference executors
    /// realize it from an explicit `HostGrant`; every other backend must
    /// refuse it with Unsupported rather than manufacturing a value.
    HostCall {
        capability: String,
        operation: String,
    },
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
    /// The source sequence bound, when this is a sequence traversal. Keeping
    /// it in selected SSA lets the lowering boundary reject symbolic bounds
    /// instead of relying only on the profile ceiling in `bound`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence_bound: Option<crate::SequenceBound>,
    pub state_type: crate::BodyType,
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
    pub fields: Vec<(String, crate::BodyType)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaModule {
    pub schema_version: String,
    pub identity: SemanticId,
    pub semantic_identity: SemanticId,
    pub hir_fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_table: Option<crate::SemanticBindingTable>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub record_types: Vec<SsaRecordType>,
    pub functions: Vec<SsaFunction>,
    pub obligations: Vec<ObligationRecord>,
    pub trace: SsaTraceMap,
    pub transformations: Vec<TransformationRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generic_specializations: Vec<crate::GenericSpecializationRecord>,
    /// Admitted tranche-0.2 proof relationships validated against this SSA.
    /// Skipped when empty so proof-free fingerprints are unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proof_relationships: Vec<crate::ProofRelationship>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SsaFunctionFactDependencyRole {
    CallableIdentity,
    ObligationIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SsaFunctionFactDependency {
    pub role: SsaFunctionFactDependencyRole,
    pub identity: SemanticId,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "identity")]
pub enum SsaFunctionFactInvalidator {
    SubjectFunctionChanged(SemanticId),
    CallableAvailabilityChanged(SemanticId),
    ObligationAvailabilityChanged(SemanticId),
    SsaSchemaChanged,
    VerifierChanged,
    PolicyChanged,
    LanguageProfileChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SsaFunctionFactResult {
    Verified,
}

/// Language-owned evidence that one exact SSA function passed the structural
/// verifier. The module validator rechecks the current function body and its
/// direct callable and obligation references before accepting this fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaFunctionVerificationFact {
    pub schema_version: String,
    pub subject: SemanticId,
    pub subject_fingerprint: String,
    pub verifier_identity: String,
    pub policy_identity: String,
    pub language_profile: String,
    pub ssa_schema: String,
    pub dependencies: Vec<SsaFunctionFactDependency>,
    pub result: SsaFunctionFactResult,
    pub verification_witness_identity: String,
    pub invalidating_changes: Vec<SsaFunctionFactInvalidator>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SsaFunctionFactReuseMiss {
    Missing,
    DuplicatePriorFacts,
    DuplicateSubjectFunctions,
    UnsupportedSchema,
    InvalidWitness,
    SubjectChanged,
    DependenciesChanged,
    VerifierChanged,
    PolicyChanged,
    LanguageProfileChanged,
    SsaSchemaChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaFunctionFactRecomputation {
    pub subject: SemanticId,
    pub reason: SsaFunctionFactReuseMiss,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaFunctionFactReuseObservation {
    pub schema_version: String,
    /// Function validators actually skipped after Language checked the
    /// subject, verifier, policy, profile, and dependency identities.
    pub reused: Vec<SemanticId>,
    /// Function validators actually run because prior evidence was missing
    /// or no longer matched the current SSA snapshot.
    pub recomputed: Vec<SsaFunctionFactRecomputation>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaFunctionFactRequest {
    #[serde(default)]
    pub prior_facts: Vec<SsaFunctionVerificationFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaFunctionFactValidation {
    pub report: SsaValidationReport,
    pub facts: Vec<SsaFunctionVerificationFact>,
    pub reuse: SsaFunctionFactReuseObservation,
}

/// A bounded caller-owned cache of complete, previously verified SSA
/// functions. Entries remain advisory: the current HIR lowering inputs,
/// lowerer policy, function dependencies, artifact digest, and Language
/// verification witness must all match before a function is reused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaFunctionArtifactCache {
    #[serde(default = "ssa_function_artifact_cache_schema")]
    schema_version: String,
    #[serde(default)]
    entries: Vec<SsaFunctionArtifactCacheEntry>,
}

impl Default for SsaFunctionArtifactCache {
    fn default() -> Self {
        Self {
            schema_version: ssa_function_artifact_cache_schema(),
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SsaFunctionArtifactCacheEntry {
    schema_version: String,
    lowerer_identity: String,
    lowering_policy_identity: String,
    input_fingerprint: String,
    artifact_fingerprint: String,
    function: SsaFunction,
    trace_entries: Vec<SsaTraceEntry>,
    transformations: Vec<TransformationRecord>,
    verification_fact: SsaFunctionVerificationFact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SsaFunctionArtifactReuseMiss {
    Missing,
    DuplicateCacheEntries,
    UnsupportedSchema,
    LowererChanged,
    LoweringPolicyChanged,
    InputChanged,
    ArtifactIntegrityFailed,
    VerificationEvidenceChanged,
    DependenciesChanged,
    VerifierChanged,
    PolicyChanged,
    LanguageProfileChanged,
    SsaSchemaChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaFunctionArtifactRecomputation {
    pub subject: SemanticId,
    pub reason: SsaFunctionArtifactReuseMiss,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaFunctionArtifactReuseObservation {
    pub schema_version: String,
    /// Complete SSA function bodies actually taken from the validated cache.
    pub reused: Vec<SemanticId>,
    /// Complete SSA functions lowered during this call and the reason.
    pub recomputed: Vec<SsaFunctionArtifactRecomputation>,
    pub rejected_entry_count: usize,
    pub evicted_entry_count: usize,
    pub published_entry_count: usize,
}

impl Default for SsaFunctionArtifactReuseObservation {
    fn default() -> Self {
        Self {
            schema_version: SSA_FUNCTION_ARTIFACT_CACHE_OBSERVATION_SCHEMA_VERSION.to_owned(),
            reused: Vec::new(),
            recomputed: Vec::new(),
            rejected_entry_count: 0,
            evicted_entry_count: 0,
            published_entry_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaLoweringFactResult {
    pub module: SsaModule,
    pub facts: Vec<SsaFunctionVerificationFact>,
    pub reuse: SsaFunctionFactReuseObservation,
    pub artifact_reuse: SsaFunctionArtifactReuseObservation,
}

#[derive(Debug, Error)]
pub enum SsaError {
    #[error("cannot lower an invalid program to SSA")]
    InvalidProgram(crate::ValidationReport),
    #[error("high-level IR lowering failed: {0}")]
    Ir(#[from] crate::IrError),
    #[error("high-level IR does not match the supplied semantic program")]
    IrMismatch { expected: String, actual: String },
    #[error("high-level IR content binding is stale or laundered: {0}")]
    IrIntegrity(String),
    #[error("SSA artifact is invalid")]
    Invalid(SsaValidationReport),
    #[error("proof relationship refused by current SSA state: {0}")]
    ProofTransport(String),
}

impl SsaFunctionArtifactCache {
    fn prepare(&mut self, observation: &mut SsaFunctionArtifactReuseObservation) {
        if self.schema_version != SSA_FUNCTION_ARTIFACT_CACHE_SCHEMA_VERSION {
            observation.rejected_entry_count += self.entries.len();
            self.entries.clear();
            self.schema_version = SSA_FUNCTION_ARTIFACT_CACHE_SCHEMA_VERSION.to_owned();
        }
        observation.evicted_entry_count += self.trim_bounded();
    }

    fn lookup(
        &self,
        subject: &SemanticId,
        input_fingerprint: &str,
        semantic_function_ids: &BTreeSet<SemanticId>,
        obligation_ids: &BTreeSet<SemanticId>,
    ) -> Result<SsaFunctionArtifactCacheEntry, SsaFunctionArtifactReuseMiss> {
        let mut matching = self
            .entries
            .iter()
            .filter(|entry| entry.verification_fact.subject == *subject);
        let Some(entry) = matching.next() else {
            return Err(SsaFunctionArtifactReuseMiss::Missing);
        };
        if matching.next().is_some() {
            return Err(SsaFunctionArtifactReuseMiss::DuplicateCacheEntries);
        }
        if entry.schema_version != SSA_FUNCTION_ARTIFACT_CACHE_SCHEMA_VERSION {
            return Err(SsaFunctionArtifactReuseMiss::UnsupportedSchema);
        }
        if entry.lowerer_identity != SSA_FUNCTION_LOWERER_IDENTITY {
            return Err(SsaFunctionArtifactReuseMiss::LowererChanged);
        }
        if entry.lowering_policy_identity != SSA_FUNCTION_LOWERING_POLICY_IDENTITY {
            return Err(SsaFunctionArtifactReuseMiss::LoweringPolicyChanged);
        }
        if entry.input_fingerprint != input_fingerprint {
            return Err(SsaFunctionArtifactReuseMiss::InputChanged);
        }
        if entry.function.semantic_identity != *subject
            || entry.function.identity != ssa_identity("function", subject, 0)
            || ssa_function_artifact_cache_fingerprint(
                &entry.input_fingerprint,
                &entry.function,
                &entry.trace_entries,
                &entry.transformations,
                &entry.verification_fact,
            )
            .as_deref()
                != Some(entry.artifact_fingerprint.as_str())
        {
            return Err(SsaFunctionArtifactReuseMiss::ArtifactIntegrityFailed);
        }
        let (subject_fingerprint, dependencies) =
            ssa_function_fact_basis(&entry.function, semantic_function_ids, obligation_ids);
        let Some(reason) = ssa_function_fact_reuse_miss(
            &entry.verification_fact,
            subject_fingerprint.as_deref(),
            &dependencies,
        ) else {
            return Ok(entry.clone());
        };
        Err(match reason {
            SsaFunctionFactReuseMiss::DependenciesChanged => {
                SsaFunctionArtifactReuseMiss::DependenciesChanged
            }
            SsaFunctionFactReuseMiss::VerifierChanged => {
                SsaFunctionArtifactReuseMiss::VerifierChanged
            }
            SsaFunctionFactReuseMiss::PolicyChanged => SsaFunctionArtifactReuseMiss::PolicyChanged,
            SsaFunctionFactReuseMiss::LanguageProfileChanged => {
                SsaFunctionArtifactReuseMiss::LanguageProfileChanged
            }
            SsaFunctionFactReuseMiss::SsaSchemaChanged => {
                SsaFunctionArtifactReuseMiss::SsaSchemaChanged
            }
            _ => SsaFunctionArtifactReuseMiss::VerificationEvidenceChanged,
        })
    }

    fn validation_facts(&self) -> Vec<SsaFunctionVerificationFact> {
        self.entries
            .iter()
            .filter(|entry| {
                entry.schema_version == SSA_FUNCTION_ARTIFACT_CACHE_SCHEMA_VERSION
                    && entry.lowerer_identity == SSA_FUNCTION_LOWERER_IDENTITY
                    && entry.lowering_policy_identity == SSA_FUNCTION_LOWERING_POLICY_IDENTITY
                    && entry.function.semantic_identity == entry.verification_fact.subject
                    && entry.function.identity
                        == ssa_identity("function", &entry.verification_fact.subject, 0)
                    && ssa_function_artifact_cache_fingerprint(
                        &entry.input_fingerprint,
                        &entry.function,
                        &entry.trace_entries,
                        &entry.transformations,
                        &entry.verification_fact,
                    )
                    .as_deref()
                        == Some(entry.artifact_fingerprint.as_str())
                    && ssa_function_fact_witness_identity(&entry.verification_fact).as_deref()
                        == Some(
                            entry
                                .verification_fact
                                .verification_witness_identity
                                .as_str(),
                        )
            })
            .map(|entry| entry.verification_fact.clone())
            .collect()
    }

    fn publish_program_artifacts(
        &mut self,
        program: &Program,
        ir: &HighLevelIr,
        module: &SsaModule,
        facts: &[SsaFunctionVerificationFact],
        observation: &mut SsaFunctionArtifactReuseObservation,
    ) -> usize {
        let facts = facts
            .iter()
            .map(|fact| (fact.subject.clone(), fact))
            .collect::<BTreeMap<_, _>>();
        let mut next = self.clone();
        let mut published = 0;
        for function in &program.functions {
            if !function.generic_params.is_empty() {
                continue;
            }
            let Some(body) = &function.body else {
                continue;
            };
            let subject = function_id(function.identity_namespace(&program.module), &function.name);
            let Some(hir_function) = ir
                .functions
                .iter()
                .find(|candidate| candidate.semantic_identity == subject)
            else {
                continue;
            };
            let Some(ssa_function) = module
                .functions
                .iter()
                .find(|candidate| candidate.semantic_identity == subject)
            else {
                continue;
            };
            let Some(fact) = facts.get(&subject) else {
                continue;
            };
            let Some(input_fingerprint) =
                ssa_function_lowering_input_fingerprint(program, function, body, hir_function, ir)
            else {
                continue;
            };
            let (trace_entries, transformations) =
                function_artifact_provenance(program, function, body, module);
            let Some(entry) = make_ssa_function_artifact_cache_entry(
                input_fingerprint,
                ssa_function.clone(),
                trace_entries,
                transformations,
                (*fact).clone(),
            ) else {
                observation.rejected_entry_count += 1;
                continue;
            };
            next.entries
                .retain(|previous| previous.verification_fact.subject != subject);
            next.entries.push(entry);
            published += 1;
        }
        next.schema_version = SSA_FUNCTION_ARTIFACT_CACHE_SCHEMA_VERSION.to_owned();
        *self = next;
        published
    }

    fn trim_bounded(&mut self) -> usize {
        let mut removed = 0;
        while self.entries.len() > MAX_SSA_FUNCTION_ARTIFACT_CACHE_ENTRIES {
            self.entries.remove(0);
            removed += 1;
        }
        let mut sizes = self
            .entries
            .iter()
            .map(|entry| {
                serde_json::to_vec(entry)
                    .map(|bytes| bytes.len())
                    .unwrap_or(usize::MAX)
            })
            .collect::<Vec<_>>();
        let mut total = sizes
            .iter()
            .fold(0u64, |total, size| total.saturating_add(*size as u64));
        while total > MAX_SSA_FUNCTION_ARTIFACT_CACHE_BYTES as u64 {
            if self.entries.is_empty() {
                break;
            }
            total -= sizes.remove(0) as u64;
            self.entries.remove(0);
            removed += 1;
        }
        removed
    }
}

fn ssa_function_artifact_cache_schema() -> String {
    SSA_FUNCTION_ARTIFACT_CACHE_SCHEMA_VERSION.to_owned()
}

impl SsaModule {
    pub fn canonical_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn fingerprint(&self) -> Result<String, serde_json::Error> {
        Ok(sha256_hex(self.canonical_json()?.as_bytes()))
    }

    /// The module identity binds the semantic program and the exact HIR
    /// fingerprint used to produce this SSA artifact. The full SSA
    /// fingerprint remains the content boundary for executable reuse.
    pub fn identity_is_valid(&self) -> bool {
        !self.hir_fingerprint.is_empty()
            && self.identity == ssa_module_identity(&self.semantic_identity, &self.hir_fingerprint)
    }

    /// Normalize every type carried by this SSA module from its legacy
    /// (pre-0.5) `IrType::Named` spelling to the resolved semantic type:
    /// values, constants, iteration state, iteration domains, record-field
    /// tables, and generic specialization arguments. Structural spellings
    /// normalize deterministically; nominal short names that cannot resolve
    /// stay `Named` so the lowering-boundary check rejects them explicitly.
    /// Idempotent: current typed modules are unchanged except for the schema
    /// upgrade stamp. [`SsaExecutionSession::from_shared`] applies this to
    /// 0.4 modules automatically; direct [`SsaModule::validate`] callers must
    /// normalize first (0.4 fails `SSA000` explicitly).
    pub fn normalize_legacy_types(&mut self) {
        fn normalize_value(value: &mut SsaValue) {
            value.ty.normalize_legacy_ir_named();
        }
        for record in &mut self.record_types {
            for (_, field_type) in &mut record.fields {
                field_type.normalize_legacy_ir_named();
            }
        }
        for function in &mut self.functions {
            for value in function
                .inputs
                .iter_mut()
                .chain(function.outputs.iter_mut())
            {
                normalize_value(value);
            }
            for iteration in &mut function.bounded_iterations {
                iteration.state_type.normalize_legacy_ir_named();
                if let crate::IterationDomain::OverSequence { element_type } = &mut iteration.domain
                {
                    element_type.normalize_legacy_ir_named();
                }
            }
            for block in &mut function.blocks {
                for value in block.parameters.iter_mut() {
                    normalize_value(value);
                }
                for instruction in &mut block.instructions {
                    for value in &mut instruction.outputs {
                        normalize_value(value);
                    }
                    if let SsaInstructionKind::Constant { ty, .. } = &mut instruction.kind {
                        ty.normalize_legacy_ir_named();
                    }
                }
            }
        }
        for specialization in &mut self.generic_specializations {
            for arg in &mut specialization.args {
                if let crate::GenericArg::Type { ty } = arg {
                    ty.normalize_legacy_ir_named();
                }
            }
        }
        self.schema_version = SSA_SCHEMA_VERSION.to_owned();
    }

    pub fn validate(&self) -> SsaValidationReport {
        self.validate_with_verified_facts(&SsaFunctionFactRequest::default())
            .report
    }

    /// Validate the module while reusing only Language-issued facts whose
    /// exact SSA function and actual callable/obligation dependencies still
    /// match this snapshot. Module identities and trace tables are always
    /// checked again because they are not function-local facts.
    pub fn validate_with_verified_facts(
        &self,
        request: &SsaFunctionFactRequest,
    ) -> SsaFunctionFactValidation {
        let mut errors = Vec::new();
        if self.schema_version != SSA_SCHEMA_VERSION {
            errors.push(diagnostic(
                "SSA000",
                "schema_version",
                "unsupported SSA schema version",
            ));
        }
        let mut function_ids = BTreeSet::new();
        let semantic_function_ids = self
            .functions
            .iter()
            .map(|function| function.semantic_identity.clone())
            .collect::<BTreeSet<_>>();
        let obligation_ids = self
            .obligations
            .iter()
            .map(|obligation| obligation.identity.clone())
            .collect::<BTreeSet<_>>();
        let mut prior_facts = BTreeMap::<SemanticId, Vec<&SsaFunctionVerificationFact>>::new();
        for fact in &request.prior_facts {
            prior_facts
                .entry(fact.subject.clone())
                .or_default()
                .push(fact);
        }
        let mut semantic_identity_counts = BTreeMap::<SemanticId, usize>::new();
        let mut ssa_identity_counts = BTreeMap::<SemanticId, usize>::new();
        for function in &self.functions {
            *semantic_identity_counts
                .entry(function.semantic_identity.clone())
                .or_default() += 1;
            *ssa_identity_counts
                .entry(function.identity.clone())
                .or_default() += 1;
        }
        let mut facts = Vec::new();
        let mut reused = Vec::new();
        let mut recomputed = Vec::new();
        for (function_index, function) in self.functions.iter().enumerate() {
            let path = format!("functions[{function_index}]");
            if !function_ids.insert(function.identity.clone()) {
                errors.push(diagnostic(
                    "SSA001",
                    &path,
                    "duplicate SSA function identity",
                ));
            }
            let (subject_fingerprint, dependencies) =
                ssa_function_fact_basis(function, &semantic_function_ids, &obligation_ids);
            let prior = prior_facts.get(&function.semantic_identity);
            let reuse_miss = if self.schema_version != SSA_SCHEMA_VERSION {
                Some(SsaFunctionFactReuseMiss::SsaSchemaChanged)
            } else if semantic_identity_counts.get(&function.semantic_identity) != Some(&1)
                || ssa_identity_counts.get(&function.identity) != Some(&1)
            {
                Some(SsaFunctionFactReuseMiss::DuplicateSubjectFunctions)
            } else {
                match prior.map(Vec::as_slice) {
                    Some([fact]) => ssa_function_fact_reuse_miss(
                        fact,
                        subject_fingerprint.as_deref(),
                        &dependencies,
                    ),
                    Some([]) | None => Some(SsaFunctionFactReuseMiss::Missing),
                    Some(_) => Some(SsaFunctionFactReuseMiss::DuplicatePriorFacts),
                }
            };
            if reuse_miss.is_none() {
                reused.push(function.semantic_identity.clone());
                if let Some(fact) = prior.and_then(|rows| rows.first()) {
                    facts.push((*fact).clone());
                }
                continue;
            }

            let mut function_errors = Vec::new();
            validate_function(
                function,
                &semantic_function_ids,
                &self.obligations,
                &path,
                &mut function_errors,
            );
            errors.extend(function_errors.iter().cloned());
            recomputed.push(SsaFunctionFactRecomputation {
                subject: function.semantic_identity.clone(),
                reason: reuse_miss.expect("the prior fact was not reusable"),
            });
            if function_errors.is_empty()
                && self.schema_version == SSA_SCHEMA_VERSION
                && semantic_identity_counts.get(&function.semantic_identity) == Some(&1)
                && ssa_identity_counts.get(&function.identity) == Some(&1)
            {
                if let Some(subject_fingerprint) = subject_fingerprint {
                    facts.push(make_ssa_function_verification_fact(
                        function.semantic_identity.clone(),
                        subject_fingerprint,
                        dependencies,
                    ));
                }
            }
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
        SsaFunctionFactValidation {
            report: SsaValidationReport {
                valid: errors.is_empty(),
                errors,
            },
            facts,
            reuse: SsaFunctionFactReuseObservation {
                schema_version: SSA_FUNCTION_VERIFICATION_FACT_SCHEMA_VERSION.to_owned(),
                reused,
                recomputed,
            },
        }
    }

    /// Validate the contract that must hold before any executable backend is
    /// entered. Generic templates may remain in the semantic program, but no
    /// template, generic argument, parameterized bound, or unresolved type
    /// may be present in selected SSA. This check is intentionally repeated at
    /// the backend boundary so direct adapter callers fail closed as well.
    pub fn validate_lowering_boundary(&self, program: &Program) -> SsaValidationReport {
        self.validate_lowering_boundary_with_verified_facts(
            program,
            &SsaFunctionFactRequest::default(),
        )
        .report
    }

    /// Validate the backend boundary with reusable function-local structural
    /// SSA facts. Program-dependent generic and concrete-type checks remain
    /// fresh for every call.
    pub fn validate_lowering_boundary_with_verified_facts(
        &self,
        program: &Program,
        request: &SsaFunctionFactRequest,
    ) -> SsaFunctionFactValidation {
        let mut validation = self.validate_with_verified_facts(request);
        let mut errors = std::mem::take(&mut validation.report.errors);
        for (function_index, function) in self.functions.iter().enumerate() {
            let path = format!("functions[{function_index}]");
            let source = program.functions.iter().find(|candidate| {
                function_id(
                    candidate.identity_namespace(&program.module),
                    &candidate.name,
                ) == function.semantic_identity
            });
            let Some(source) = source else {
                errors.push(diagnostic(
                    "SSA040",
                    &path,
                    "selected SSA function does not resolve to a program function",
                ));
                continue;
            };
            if !source.generic_params.is_empty() {
                errors.push(diagnostic(
                    "SSA041",
                    &path,
                    "generic function template reached the backend lowering boundary",
                ));
            }
            let generic_names = source
                .generic_params
                .iter()
                .map(|param| param.name.clone())
                .collect::<BTreeSet<_>>();
            for (value_index, value) in function.inputs.iter().enumerate() {
                check_body_type(
                    &value.ty,
                    &generic_names,
                    &format!("{path}.inputs[{value_index}].type"),
                    &mut errors,
                );
            }
            for (value_index, value) in function.outputs.iter().enumerate() {
                check_body_type(
                    &value.ty,
                    &generic_names,
                    &format!("{path}.outputs[{value_index}].type"),
                    &mut errors,
                );
            }
            for (iteration_index, iteration) in function.bounded_iterations.iter().enumerate() {
                check_body_type(
                    &iteration.state_type,
                    &generic_names,
                    &format!("{path}.bounded_iterations[{iteration_index}].state_type"),
                    &mut errors,
                );
                if let crate::IterationDomain::OverSequence { element_type } = &iteration.domain {
                    check_body_type(
                        element_type,
                        &generic_names,
                        &format!("{path}.bounded_iterations[{iteration_index}].domain"),
                        &mut errors,
                    );
                }
                if let Some(sequence_bound) = &iteration.sequence_bound {
                    check_bound(
                        sequence_bound,
                        &format!("{path}.bounded_iterations[{iteration_index}].sequence_bound"),
                        &mut errors,
                    );
                }
            }
            for (block_index, block) in function.blocks.iter().enumerate() {
                let block_path = format!("{path}.blocks[{block_index}]");
                for (value_index, value) in block.parameters.iter().enumerate() {
                    check_body_type(
                        &value.ty,
                        &generic_names,
                        &format!("{block_path}.parameters[{value_index}].type"),
                        &mut errors,
                    );
                }
                for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                    check_instruction_types(
                        &instruction.kind,
                        &generic_names,
                        &format!("{block_path}.instructions[{instruction_index}]"),
                        &mut errors,
                    );
                    for (value_index, value) in instruction.outputs.iter().enumerate() {
                        check_body_type(
                            &value.ty,
                            &generic_names,
                            &format!(
                                "{block_path}.instructions[{instruction_index}].outputs[{value_index}].type"
                            ),
                            &mut errors,
                        );
                    }
                }
            }
        }

        for (record_index, record) in self.generic_specializations.iter().enumerate() {
            let path = format!("generic_specializations[{record_index}]");
            let generic = program.functions.iter().find(|function| {
                function_id(function.identity_namespace(&program.module), &function.name)
                    == record.generic_function
            });
            if generic.is_none_or(|function| function.generic_params.is_empty()) {
                errors.push(diagnostic(
                    "SSA042",
                    &path,
                    "specialization record does not reference a generic declaration",
                ));
            }
            let specialization = program.functions.iter().find(|function| {
                function_id(function.identity_namespace(&program.module), &function.name)
                    == record.specialization_function
            });
            if specialization.is_none_or(|function| !function.generic_params.is_empty()) {
                errors.push(diagnostic(
                    "SSA043",
                    &path,
                    "specialization record does not reference a concrete function",
                ));
            }
            if record.args.iter().any(|arg| !arg.is_concrete()) {
                errors.push(diagnostic(
                    "SSA044",
                    &path,
                    "specialization record retains a symbolic generic argument",
                ));
            }
        }

        validation.report.valid = errors.is_empty();
        validation.report.errors = errors;
        validation
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

    /// Proof-gated variant of the no-overflow elision experiment (RFC 0007).
    ///
    /// In addition to the exact current verifier PASS, this entry point
    /// requires a proof binding that names the same obligation, carries the
    /// current kernel version, records a `Pass` outcome, and matches the
    /// supplied dependency fingerprints exactly. The proof identity is
    /// recorded in the transformation record, so the consumed formal evidence
    /// is explicit instead of an ad-hoc certificate claim. Any mismatch
    /// withholds the transformation.
    pub fn attempt_no_overflow_elision_with_proof(
        &self,
        operation: &SemanticId,
        obligation: &SemanticId,
        evidence: &[VerifierResult],
        binding: &crate::ProofBinding,
        dependency_fingerprints: &std::collections::BTreeMap<SemanticId, String>,
    ) -> SsaTransformationDecision {
        let proof_ok = binding.reusable_if(
            &binding.proof,
            crate::PROOF_KERNEL_ID,
            obligation,
            &binding.assumptions,
            dependency_fingerprints,
            crate::ProofVerdict::Pass,
        );
        let mut decision = self.attempt_no_overflow_elision(operation, obligation, evidence);
        if !proof_ok {
            decision.permitted = false;
            decision.reason =
                "missing exact RFC 0007 proof binding conservatively withholds the transformation"
                    .to_owned();
            decision.record.backend_promises_permitted.clear();
            decision
                .record
                .backend_promises_withheld
                .push("no-overflow".to_owned());
            decision.record.evidence_consumed.clear();
            return decision;
        }
        if decision.permitted {
            decision
                .record
                .evidence_consumed
                .push(binding.proof.clone());
            decision.reason = format!(
                "exact current verifier PASS plus proof {} permits the bounded transformation",
                binding.proof.0
            );
        } else {
            decision.reason = format!(
                "{}; proof {} alone does not replace the required verifier PASS",
                decision.reason, binding.proof.0
            );
        }
        decision
    }
}

fn check_bound(bound: &crate::SequenceBound, path: &str, errors: &mut Vec<SsaDiagnostic>) {
    if bound.is_generic() {
        errors.push(diagnostic(
            "SSA046",
            path,
            "parameterized sequence bound reached the backend lowering boundary",
        ));
    }
}

fn check_body_type(
    ty: &crate::BodyType,
    generic_names: &BTreeSet<String>,
    path: &str,
    errors: &mut Vec<SsaDiagnostic>,
) {
    match ty {
        crate::BodyType::GenericParam { .. } => errors.push(diagnostic(
            "SSA045",
            path,
            "unresolved type reached the backend lowering boundary",
        )),
        crate::BodyType::Named(name) if generic_names.contains(name) => errors.push(diagnostic(
            "SSA045",
            path,
            "unresolved type reached the backend lowering boundary",
        )),
        // Any other unresolved name (including the `"invalid"` error poison)
        // must not reach lowering either. Legacy `Named("bool")` is the only
        // spelling still accepted here for pre-0.3 artifacts.
        crate::BodyType::Named(name) if name != "bool" => errors.push(diagnostic(
            "SSA045",
            path,
            "unresolved type reached the backend lowering boundary",
        )),
        crate::BodyType::Sequence { element, bound } => {
            check_body_type(element, generic_names, path, errors);
            check_bound(bound, path, errors);
        }
        crate::BodyType::Vector { element, .. } => {
            check_body_type(element, generic_names, path, errors);
        }
        _ => {}
    }
}

fn check_instruction_types(
    kind: &SsaInstructionKind,
    generic_names: &BTreeSet<String>,
    path: &str,
    errors: &mut Vec<SsaDiagnostic>,
) {
    match kind {
        SsaInstructionKind::Constant { ty, .. } => check_body_type(ty, generic_names, path, errors),
        SsaInstructionKind::Select { operand_type }
        | SsaInstructionKind::SequenceReplace {
            element_type: operand_type,
            ..
        }
        | SsaInstructionKind::SequenceCopy {
            element_type: operand_type,
            ..
        }
        | SsaInstructionKind::VectorConstruct {
            element_type: operand_type,
            ..
        }
        | SsaInstructionKind::VectorSplat {
            element_type: operand_type,
            ..
        }
        | SsaInstructionKind::VectorExtract {
            element_type: operand_type,
            ..
        }
        | SsaInstructionKind::VectorReplace {
            element_type: operand_type,
            ..
        }
        | SsaInstructionKind::VectorBinary {
            element_type: operand_type,
            ..
        }
        | SsaInstructionKind::VectorCompare {
            element_type: operand_type,
            ..
        }
        | SsaInstructionKind::VectorReduce {
            element_type: operand_type,
            ..
        } => check_body_type(operand_type, generic_names, path, errors),
        SsaInstructionKind::Convert { from, to } => {
            check_body_type(from, generic_names, path, errors);
            check_body_type(to, generic_names, path, errors);
        }
        SsaInstructionKind::SequenceConstruct { element_type, .. } => {
            check_body_type(element_type, generic_names, path, errors)
        }
        SsaInstructionKind::SequenceProject { bound, .. }
        | SsaInstructionKind::SequenceLength { bound } => check_bound(bound, path, errors),
        SsaInstructionKind::ViewConstruct {
            source_bound,
            view_bound,
        } => {
            check_bound(source_bound, path, errors);
            check_bound(view_bound, path, errors);
        }
        _ => {}
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
        crate::record_counter("ssa_build");
        let started = Instant::now();
        let report = self.validate();
        if !report.valid {
            return Err(SsaError::InvalidProgram(report));
        }
        trace_timing("ssa-validate", started);
        let ir = self.lower_to_ir()?;
        trace_timing("ssa-ir", started);
        self.lower_to_ssa_from_ir_inner(&ir, started, &SsaFunctionFactRequest::default(), None)
            .map(|result| result.module)
    }

    /// Lower selected SSA from a HIR module already produced for this exact
    /// semantic program. Compiler drivers use this to avoid lowering HIR a
    /// second time merely because they also need the SSA emission.
    pub fn lower_to_ssa_from_ir(&self, ir: &HighLevelIr) -> Result<SsaModule, SsaError> {
        self.lower_to_ssa_from_ir_with_verified_facts(ir, &SsaFunctionFactRequest::default())
            .map(|result| result.module)
    }

    /// Lower SSA and reuse verified per-function structural facts when the
    /// current function bodies and their direct callable/obligation
    /// dependencies match the Language-issued evidence.
    pub fn lower_to_ssa_from_ir_with_verified_facts(
        &self,
        ir: &HighLevelIr,
        request: &SsaFunctionFactRequest,
    ) -> Result<SsaLoweringFactResult, SsaError> {
        self.lower_to_ssa_from_ir_with_reuse(ir, request, None)
    }

    /// Lower SSA while reusing complete Language-verified function artifacts
    /// from a caller-owned cache. The current program, HIR, module invariants,
    /// dependency availability, and lowering boundary are still checked.
    pub fn lower_to_ssa_from_ir_with_artifact_cache(
        &self,
        ir: &HighLevelIr,
        cache: &mut SsaFunctionArtifactCache,
    ) -> Result<SsaLoweringFactResult, SsaError> {
        self.lower_to_ssa_from_ir_with_reuse(ir, &SsaFunctionFactRequest::default(), Some(cache))
    }

    /// Lower SSA using both caller-supplied validation facts and complete
    /// cached function artifacts. Freshly lowered units are still checked
    /// against any supplied facts before the cache is extended.
    pub fn lower_to_ssa_from_ir_with_verified_facts_and_artifact_cache(
        &self,
        ir: &HighLevelIr,
        request: &SsaFunctionFactRequest,
        cache: &mut SsaFunctionArtifactCache,
    ) -> Result<SsaLoweringFactResult, SsaError> {
        self.lower_to_ssa_from_ir_with_reuse(ir, request, Some(cache))
    }

    fn lower_to_ssa_from_ir_with_reuse(
        &self,
        ir: &HighLevelIr,
        request: &SsaFunctionFactRequest,
        cache: Option<&mut SsaFunctionArtifactCache>,
    ) -> Result<SsaLoweringFactResult, SsaError> {
        crate::record_counter("ssa_build");
        let started = Instant::now();
        let report = self.validate();
        if !report.valid {
            return Err(SsaError::InvalidProgram(report));
        }
        let expected = self
            .content_fingerprint()
            .expect("validated semantic program is canonicalizable");
        if ir.semantic_identity != crate::identity::program_id(&self.module)
            || ir.semantic_fingerprint != expected
        {
            return Err(SsaError::IrMismatch {
                expected,
                actual: ir.semantic_fingerprint.clone(),
            });
        }
        if !ir.integrity_is_valid() {
            return Err(SsaError::IrIntegrity(
                "complete HIR content binding does not match the reusable artifact".to_owned(),
            ));
        }
        trace_timing("ssa-validate", started);
        trace_timing("ssa-ir-reused", started);
        self.lower_to_ssa_from_ir_inner(ir, started, request, cache)
    }

    /// Lower SSA from already-lowered HIR and attach admitted tranche-0.2
    /// proof relationships, validated against the lowered SSA with the exact
    /// current dependency fingerprints. Any refusal fails the whole lowering
    /// (fail-closed): a stale relationship never reaches lowering evidence.
    pub fn lower_to_ssa_from_ir_with_proofs(
        &self,
        ir: &HighLevelIr,
        relationships: &[crate::ProofRelationship],
        dependencies: &[String],
    ) -> Result<SsaModule, SsaError> {
        let mut ssa = self.lower_to_ssa_from_ir(ir)?;
        for relationship in relationships {
            ssa.attach_proof_relationship(relationship.clone(), dependencies)
                .map_err(|mismatch| SsaError::ProofTransport(mismatch.to_string()))?;
        }
        Ok(ssa)
    }

    fn lower_to_ssa_from_ir_inner(
        &self,
        ir: &HighLevelIr,
        started: Instant,
        request: &SsaFunctionFactRequest,
        mut artifact_cache: Option<&mut SsaFunctionArtifactCache>,
    ) -> Result<SsaLoweringFactResult, SsaError> {
        let semantic_identity = crate::identity::program_id(&self.module);
        let mut functions = Vec::new();
        let mut trace_entries = BTreeMap::<SemanticId, SsaTraceEntry>::new();
        let mut transformations = Vec::new();
        let mut semantic_functions = self.functions.iter().collect::<Vec<_>>();
        semantic_functions.sort_by(|left, right| left.name.cmp(&right.name));
        let current_function_ids = semantic_functions
            .iter()
            .filter(|function| function.generic_params.is_empty())
            .map(|function| function_id(function.identity_namespace(&self.module), &function.name))
            .collect::<BTreeSet<_>>();
        let obligation_ids = ir
            .obligations
            .iter()
            .map(|obligation| obligation.identity.clone())
            .collect::<BTreeSet<_>>();
        let mut artifact_reuse = SsaFunctionArtifactReuseObservation::default();
        if let Some(cache) = artifact_cache.as_deref_mut() {
            cache.prepare(&mut artifact_reuse);
        }
        for function in semantic_functions {
            if !function.generic_params.is_empty() {
                // Generic templates are not directly lowered; their specializations are concrete functions.
                continue;
            }
            let semantic_function =
                function_id(function.identity_namespace(&self.module), &function.name);
            let Some(body) = &function.body else {
                functions.push(declaration_function(self, function, &semantic_function));
                continue;
            };
            let Some(hir_function) = ir
                .functions
                .iter()
                .find(|candidate| candidate.semantic_identity == semantic_function)
            else {
                return Err(SsaError::IrIntegrity(format!(
                    "reusable HIR is missing executable function {semantic_function}"
                )));
            };
            let input_fingerprint = if artifact_cache.is_some() {
                Some(
                    ssa_function_lowering_input_fingerprint(self, function, body, hir_function, ir)
                        .ok_or_else(|| {
                            SsaError::IrIntegrity(
                                "function lowering inputs could not be fingerprinted".to_owned(),
                            )
                        })?,
                )
            } else {
                None
            };
            let cache_match = match (artifact_cache.as_deref(), input_fingerprint.as_deref()) {
                (Some(cache), Some(input_fingerprint)) => cache.lookup(
                    &semantic_function,
                    input_fingerprint,
                    &current_function_ids,
                    &obligation_ids,
                ),
                _ => Err(SsaFunctionArtifactReuseMiss::Missing),
            };
            match cache_match {
                Ok(entry) => {
                    functions.push(entry.function.clone());
                    transformations.extend(entry.transformations.clone());
                    merge_trace_entries(&mut trace_entries, entry.trace_entries.clone());
                    artifact_reuse.reused.push(semantic_function.clone());
                }
                Err(reason) => {
                    let (lowered, entries, records) =
                        lower_body(self, function, body, hir_function, ir);
                    functions.push(lowered);
                    transformations.extend(records);
                    merge_trace_entries(&mut trace_entries, entries);
                    if artifact_cache.is_some() {
                        artifact_reuse
                            .recomputed
                            .push(SsaFunctionArtifactRecomputation {
                                subject: semantic_function,
                                reason,
                            });
                    }
                }
            }
        }
        trace_timing("ssa-functions", started);
        let hir_fingerprint = ir.fingerprint().expect("HIR is serializable");
        let identity = ssa_module_identity(&semantic_identity, &hir_fingerprint);
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
                        // Single authority with every other syntax-to-type
                        // boundary: program-aware semantic resolution. Field
                        // spellings that name nothing stay `Named` for the
                        // lowering-boundary check to reject explicitly.
                        let field_type = crate::TypeSyntax::from(field.field_type.as_str())
                            .resolve_against(self);
                        (field.name.clone(), field_type)
                    })
                    .collect(),
            })
            .collect();
        let module = SsaModule {
            schema_version: SSA_SCHEMA_VERSION.to_owned(),
            identity,
            semantic_identity,
            hir_fingerprint,
            binding_table: self.binding_table.clone(),
            record_types,
            functions,
            obligations: ir.obligations.clone(),
            trace: SsaTraceMap {
                entries: trace_entries.into_values().collect(),
            },
            transformations,
            generic_specializations: ir.generic_specializations.clone(),
            proof_relationships: Vec::new(),
        };
        let mut validation_request = request.clone();
        if let Some(cache) = artifact_cache.as_deref() {
            let prior_subjects = validation_request
                .prior_facts
                .iter()
                .map(|fact| fact.subject.clone())
                .collect::<BTreeSet<_>>();
            validation_request.prior_facts.extend(
                cache
                    .validation_facts()
                    .into_iter()
                    .filter(|fact| !prior_subjects.contains(&fact.subject)),
            );
        }
        let validation = module.validate_with_verified_facts(&validation_request);
        trace_timing("ssa-total", started);
        if validation.report.valid {
            if let Some(cache) = artifact_cache.as_deref_mut() {
                let published = cache.publish_program_artifacts(
                    self,
                    ir,
                    &module,
                    &validation.facts,
                    &mut artifact_reuse,
                );
                artifact_reuse.published_entry_count += published;
                artifact_reuse.evicted_entry_count += cache.trim_bounded();
            }
            Ok(SsaLoweringFactResult {
                module,
                facts: validation.facts,
                reuse: validation.reuse,
                artifact_reuse,
            })
        } else {
            Err(SsaError::Invalid(validation.report))
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
            ty: crate::TypeSyntax::from(value.value_type.as_str()).resolve_against(program),
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

fn ssa_function_lowering_input_fingerprint(
    program: &Program,
    function: &Function,
    body: &crate::FunctionBody,
    hir_function: &crate::IrFunction,
    ir: &HighLevelIr,
) -> Option<String> {
    let home = function.identity_namespace(&program.module);
    let block_trace_inputs = body
        .blocks
        .iter()
        .map(|block| {
            let identity = body.block_identity(home, &function.name, &block.id);
            (
                identity.clone(),
                ir.trace_for(&identity)
                    .map(|entry| entry.ir_identities.clone())
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    let material = (
        SSA_FUNCTION_ARTIFACT_CACHE_SCHEMA_VERSION,
        SSA_FUNCTION_LOWERER_IDENTITY,
        SSA_FUNCTION_LOWERING_POLICY_IDENTITY,
        SSA_SCHEMA_VERSION,
        ir.schema_version.as_str(),
        program.module.as_str(),
        function,
        body,
        hir_function,
        block_trace_inputs,
    );
    serde_json::to_vec(&material)
        .ok()
        .map(|bytes| format!("sha256:{}", sha256_hex(&bytes)))
}

fn function_artifact_provenance(
    program: &Program,
    function: &Function,
    body: &crate::FunctionBody,
    module: &SsaModule,
) -> (Vec<SsaTraceEntry>, Vec<TransformationRecord>) {
    let home = function.identity_namespace(&program.module);
    let mut trace_identities = BTreeSet::new();
    let mut operation_identities = BTreeSet::new();
    for block in &body.blocks {
        trace_identities.insert(body.block_identity(home, &function.name, &block.id));
        for operation in &block.operations {
            let identity = operation.identity(home, &function.name, &block.id);
            trace_identities.insert(identity.clone());
            operation_identities.insert(identity);
        }
    }
    let trace_entries = module
        .trace
        .entries
        .iter()
        .filter(|entry| trace_identities.contains(&entry.semantic_identity))
        .cloned()
        .collect();
    let transformations = module
        .transformations
        .iter()
        .filter(|record| operation_identities.contains(&record.operation))
        .cloned()
        .collect();
    (trace_entries, transformations)
}

fn make_ssa_function_artifact_cache_entry(
    input_fingerprint: String,
    function: SsaFunction,
    trace_entries: Vec<SsaTraceEntry>,
    transformations: Vec<TransformationRecord>,
    verification_fact: SsaFunctionVerificationFact,
) -> Option<SsaFunctionArtifactCacheEntry> {
    let artifact_fingerprint = ssa_function_artifact_cache_fingerprint(
        &input_fingerprint,
        &function,
        &trace_entries,
        &transformations,
        &verification_fact,
    )?;
    Some(SsaFunctionArtifactCacheEntry {
        schema_version: SSA_FUNCTION_ARTIFACT_CACHE_SCHEMA_VERSION.to_owned(),
        lowerer_identity: SSA_FUNCTION_LOWERER_IDENTITY.to_owned(),
        lowering_policy_identity: SSA_FUNCTION_LOWERING_POLICY_IDENTITY.to_owned(),
        input_fingerprint,
        artifact_fingerprint,
        function,
        trace_entries,
        transformations,
        verification_fact,
    })
}

fn ssa_function_artifact_cache_fingerprint(
    input_fingerprint: &str,
    function: &SsaFunction,
    trace_entries: &[SsaTraceEntry],
    transformations: &[TransformationRecord],
    verification_fact: &SsaFunctionVerificationFact,
) -> Option<String> {
    let material = (
        SSA_FUNCTION_ARTIFACT_CACHE_SCHEMA_VERSION,
        SSA_FUNCTION_LOWERER_IDENTITY,
        SSA_FUNCTION_LOWERING_POLICY_IDENTITY,
        input_fingerprint,
        function,
        trace_entries,
        transformations,
        verification_fact,
    );
    serde_json::to_vec(&material)
        .ok()
        .map(|bytes| format!("sha256:{}", sha256_hex(&bytes)))
}

fn merge_trace_entries(
    target: &mut BTreeMap<SemanticId, SsaTraceEntry>,
    entries: impl IntoIterator<Item = SsaTraceEntry>,
) {
    for entry in entries {
        target
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
                ty: parameter.ty.clone(),
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
                    ty: parameter.ty.clone(),
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
                        ty: result.ty.clone(),
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
                sequence_bound: iteration.sequence_bound.clone(),
                identity,
                bound: iteration.bound,
                state_type: iteration.state_type.clone(),
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

fn ssa_function_fact_basis(
    function: &SsaFunction,
    semantic_function_ids: &BTreeSet<SemanticId>,
    obligation_ids: &BTreeSet<SemanticId>,
) -> (Option<String>, Vec<SsaFunctionFactDependency>) {
    let subject_fingerprint = serde_json::to_vec(function)
        .ok()
        .map(|bytes| format!("sha256:{}", sha256_hex(&bytes)));
    let mut dependencies = BTreeSet::new();
    for block in &function.blocks {
        for instruction in &block.instructions {
            if let SsaInstructionKind::Call { function, .. } = &instruction.kind {
                dependencies.insert(SsaFunctionFactDependency {
                    role: SsaFunctionFactDependencyRole::CallableIdentity,
                    identity: function.clone(),
                    fingerprint: if semantic_function_ids.contains(function) {
                        "present".to_owned()
                    } else {
                        "missing".to_owned()
                    },
                });
            }
            for obligation in &instruction.obligations {
                dependencies.insert(SsaFunctionFactDependency {
                    role: SsaFunctionFactDependencyRole::ObligationIdentity,
                    identity: obligation.clone(),
                    fingerprint: if obligation_ids.contains(obligation) {
                        "present".to_owned()
                    } else {
                        "missing".to_owned()
                    },
                });
            }
        }
    }
    for iteration in &function.bounded_iterations {
        for obligation in &iteration.obligations {
            dependencies.insert(SsaFunctionFactDependency {
                role: SsaFunctionFactDependencyRole::ObligationIdentity,
                identity: obligation.clone(),
                fingerprint: if obligation_ids.contains(obligation) {
                    "present".to_owned()
                } else {
                    "missing".to_owned()
                },
            });
        }
    }
    (subject_fingerprint, dependencies.into_iter().collect())
}

fn make_ssa_function_verification_fact(
    subject: SemanticId,
    subject_fingerprint: String,
    dependencies: Vec<SsaFunctionFactDependency>,
) -> SsaFunctionVerificationFact {
    let mut invalidating_changes = vec![
        SsaFunctionFactInvalidator::SubjectFunctionChanged(subject.clone()),
        SsaFunctionFactInvalidator::SsaSchemaChanged,
        SsaFunctionFactInvalidator::VerifierChanged,
        SsaFunctionFactInvalidator::PolicyChanged,
        SsaFunctionFactInvalidator::LanguageProfileChanged,
    ];
    for dependency in &dependencies {
        invalidating_changes.push(match dependency.role {
            SsaFunctionFactDependencyRole::CallableIdentity => {
                SsaFunctionFactInvalidator::CallableAvailabilityChanged(dependency.identity.clone())
            }
            SsaFunctionFactDependencyRole::ObligationIdentity => {
                SsaFunctionFactInvalidator::ObligationAvailabilityChanged(
                    dependency.identity.clone(),
                )
            }
        });
    }
    let mut fact = SsaFunctionVerificationFact {
        schema_version: SSA_FUNCTION_VERIFICATION_FACT_SCHEMA_VERSION.to_owned(),
        subject,
        subject_fingerprint,
        verifier_identity: SSA_FUNCTION_VERIFICATION_FACT_VERIFIER_ID.to_owned(),
        policy_identity: SSA_FUNCTION_VERIFICATION_FACT_POLICY_ID.to_owned(),
        language_profile: SSA_FUNCTION_VERIFICATION_LANGUAGE_PROFILE.to_owned(),
        ssa_schema: SSA_SCHEMA_VERSION.to_owned(),
        dependencies,
        result: SsaFunctionFactResult::Verified,
        verification_witness_identity: String::new(),
        invalidating_changes,
    };
    fact.verification_witness_identity = ssa_function_fact_witness_identity(&fact)
        .expect("Language SSA verification facts serialize deterministically");
    fact
}

fn ssa_function_fact_witness_identity(fact: &SsaFunctionVerificationFact) -> Option<String> {
    let material = (
        fact.schema_version.as_str(),
        &fact.subject,
        fact.subject_fingerprint.as_str(),
        fact.verifier_identity.as_str(),
        fact.policy_identity.as_str(),
        fact.language_profile.as_str(),
        fact.ssa_schema.as_str(),
        &fact.dependencies,
        fact.result,
        &fact.invalidating_changes,
    );
    serde_json::to_vec(&material).ok().map(|bytes| {
        let mut witness = b"mncs.language.ssa-function-verification-witness/1\0".to_vec();
        witness.extend_from_slice(&bytes);
        format!("sha256:{}", sha256_hex(&witness))
    })
}

fn ssa_function_fact_reuse_miss(
    fact: &SsaFunctionVerificationFact,
    subject_fingerprint: Option<&str>,
    dependencies: &[SsaFunctionFactDependency],
) -> Option<SsaFunctionFactReuseMiss> {
    if fact.schema_version != SSA_FUNCTION_VERIFICATION_FACT_SCHEMA_VERSION {
        return Some(SsaFunctionFactReuseMiss::UnsupportedSchema);
    }
    if fact.subject_fingerprint != subject_fingerprint.unwrap_or_default() {
        return Some(SsaFunctionFactReuseMiss::SubjectChanged);
    }
    if fact.verifier_identity != SSA_FUNCTION_VERIFICATION_FACT_VERIFIER_ID {
        return Some(SsaFunctionFactReuseMiss::VerifierChanged);
    }
    if fact.policy_identity != SSA_FUNCTION_VERIFICATION_FACT_POLICY_ID {
        return Some(SsaFunctionFactReuseMiss::PolicyChanged);
    }
    if fact.language_profile != SSA_FUNCTION_VERIFICATION_LANGUAGE_PROFILE {
        return Some(SsaFunctionFactReuseMiss::LanguageProfileChanged);
    }
    if fact.ssa_schema != SSA_SCHEMA_VERSION {
        return Some(SsaFunctionFactReuseMiss::SsaSchemaChanged);
    }
    if fact.dependencies != dependencies {
        return Some(SsaFunctionFactReuseMiss::DependenciesChanged);
    }
    if ssa_function_fact_witness_identity(fact).as_deref()
        != Some(fact.verification_witness_identity.as_str())
    {
        return Some(SsaFunctionFactReuseMiss::InvalidWitness);
    }
    None
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
                && block.parameters[1].ty
                    == crate::BodyType::Integer(crate::IntegerType {
                        bits: 64,
                        signed: false,
                    })
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
    let mut definitions = BTreeMap::<SemanticId, (Option<SemanticId>, crate::BodyType)>::new();
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
    available: &BTreeMap<SemanticId, crate::BodyType>,
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
        IrOperationKind::BooleanCompare { predicate } => SsaInstructionKind::BooleanCompare {
            predicate: predicate.clone(),
        },
        IrOperationKind::BooleanNot => SsaInstructionKind::BooleanNot,
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
            bound: bound.clone(),
            evidence: evidence.clone(),
        },
        IrOperationKind::BoundCheck { bound } => SsaInstructionKind::BoundCheck {
            bound: bound.clone(),
        },
        IrOperationKind::SequenceCopy {
            element_type,
            dst_bound,
            src_bound,
            evidence,
        } => SsaInstructionKind::SequenceCopy {
            element_type: element_type.as_ref().clone(),
            dst_bound: dst_bound.clone(),
            src_bound: src_bound.clone(),
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
                bound: bound.clone(),
                evidence: evidence.clone(),
            }
        }
        IrOperationKind::SequenceLength { bound } => SsaInstructionKind::SequenceLength {
            bound: bound.clone(),
        },
        IrOperationKind::ViewConstruct {
            source_bound,
            view_bound,
        } => SsaInstructionKind::ViewConstruct {
            source_bound: source_bound.clone(),
            view_bound: view_bound.clone(),
        },
        IrOperationKind::ViewNarrow {
            source_cap,
            new_cap,
        } => SsaInstructionKind::ViewNarrow {
            source_cap: *source_cap,
            new_cap: *new_cap,
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
        IrOperationKind::FloatConstant { bits, ty } => SsaInstructionKind::FloatConstant {
            bits: *bits,
            ty: *ty,
        },
        IrOperationKind::Float { operator } => SsaInstructionKind::Float {
            operator: operator.clone(),
        },
        IrOperationKind::FloatCompare { predicate } => SsaInstructionKind::FloatCompare {
            predicate: predicate.clone(),
        },
        IrOperationKind::FloatIntrinsic { function } => SsaInstructionKind::FloatIntrinsic {
            function: function.clone(),
        },
        IrOperationKind::HostCall {
            capability,
            operation,
        } => SsaInstructionKind::HostCall {
            capability: capability.clone(),
            operation: operation.clone(),
        },
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
            ty: ty.clone(),
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
        BodyOperationKind::FloatConstant { bits, ty } => SsaInstructionKind::FloatConstant {
            bits: *bits,
            ty: *ty,
        },
        BodyOperationKind::Float { operator } => SsaInstructionKind::Float {
            operator: operator.clone(),
        },
        BodyOperationKind::FloatCompare { predicate } => SsaInstructionKind::FloatCompare {
            predicate: predicate.clone(),
        },
        BodyOperationKind::FloatIntrinsic { function } => SsaInstructionKind::FloatIntrinsic {
            function: function.clone(),
        },
        BodyOperationKind::BooleanOp { operator } => SsaInstructionKind::BooleanOp {
            operator: operator.clone(),
        },
        BodyOperationKind::BooleanCompare { predicate } => SsaInstructionKind::BooleanCompare {
            predicate: predicate.clone(),
        },
        BodyOperationKind::BooleanNot => SsaInstructionKind::BooleanNot,
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
            bound: bound.clone(),
            evidence: evidence.clone(),
        },
        BodyOperationKind::BoundCheck { bound } => SsaInstructionKind::BoundCheck {
            bound: bound.clone(),
        },
        BodyOperationKind::SequenceCopy {
            element_type,
            dst_bound,
            src_bound,
            evidence,
        } => SsaInstructionKind::SequenceCopy {
            element_type: element_type.as_ref().clone(),
            dst_bound: dst_bound.clone(),
            src_bound: src_bound.clone(),
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
                bound: bound.clone(),
                evidence: evidence.clone(),
            }
        }
        BodyOperationKind::SequenceLength { bound } => SsaInstructionKind::SequenceLength {
            bound: bound.clone(),
        },
        BodyOperationKind::ViewConstruct {
            source_bound,
            view_bound,
        } => SsaInstructionKind::ViewConstruct {
            source_bound: source_bound.clone(),
            view_bound: view_bound.clone(),
        },
        BodyOperationKind::ViewNarrow {
            source_cap,
            new_cap,
        } => SsaInstructionKind::ViewNarrow {
            source_cap: *source_cap,
            new_cap: *new_cap,
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
        BodyOperationKind::HostCall {
            capability,
            operation,
        } => SsaInstructionKind::HostCall {
            capability: capability.clone(),
            operation: operation.clone(),
        },
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

fn ssa_identity(kind: &str, subject: &SemanticId, ordinal: usize) -> SemanticId {
    SemanticId(format!(
        "mncs:0.4:ssa:{kind}:{}",
        sha256_hex(format!("{subject}:{ordinal}").as_bytes())
    ))
}

fn ssa_module_identity(semantic_identity: &SemanticId, hir_fingerprint: &str) -> SemanticId {
    SemanticId(format!(
        "mncs:0.4:ssa:module:{}",
        sha256_hex(format!("{}:{}", semantic_identity, hir_fingerprint).as_bytes())
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
    fn language_ssa_function_facts_reuse_only_exact_verified_units() {
        let program = executable_program();
        let hir = program.lower_to_ir().expect("HIR");
        let cold = program
            .lower_to_ssa_from_ir_with_verified_facts(&hir, &SsaFunctionFactRequest::default())
            .expect("cold SSA lowering");
        assert_eq!(cold.reuse.reused, Vec::<SemanticId>::new());
        assert_eq!(cold.facts.len(), 1);
        assert_eq!(
            cold.reuse.recomputed[0].reason,
            SsaFunctionFactReuseMiss::Missing
        );
        assert!(cold.module.validate().valid);

        let warm = program
            .lower_to_ssa_from_ir_with_verified_facts(
                &hir,
                &SsaFunctionFactRequest {
                    prior_facts: cold.facts.clone(),
                },
            )
            .expect("warm SSA lowering");
        assert_eq!(warm.module, cold.module);
        assert_eq!(warm.reuse.reused, vec![cold.facts[0].subject.clone()]);
        assert!(warm.reuse.recomputed.is_empty());

        let mut edited_body = cold.module.clone();
        edited_body.functions[0].blocks[0].instructions[0]
            .inputs
            .push(SemanticId("mncs:missing-value".to_owned()));
        let body_edit = edited_body.validate_with_verified_facts(&SsaFunctionFactRequest {
            prior_facts: cold.facts.clone(),
        });
        assert!(!body_edit.report.valid);
        assert!(body_edit.reuse.reused.is_empty());
        assert_eq!(
            body_edit.reuse.recomputed[0].reason,
            SsaFunctionFactReuseMiss::SubjectChanged
        );
        assert!(body_edit
            .report
            .errors
            .iter()
            .any(|error| error.code == "SSA006"));
        assert!(body_edit.facts.is_empty());

        let referenced_obligation = cold.module.functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .flat_map(|instruction| &instruction.obligations)
            .next()
            .cloned()
            .expect("function references an obligation");
        let mut removed_dependency = cold.module.clone();
        removed_dependency
            .obligations
            .retain(|obligation| obligation.identity != referenced_obligation);
        let dependency_edit =
            removed_dependency.validate_with_verified_facts(&SsaFunctionFactRequest {
                prior_facts: cold.facts.clone(),
            });
        assert!(!dependency_edit.report.valid);
        assert!(dependency_edit.reuse.reused.is_empty());
        assert_eq!(
            dependency_edit.reuse.recomputed[0].reason,
            SsaFunctionFactReuseMiss::DependenciesChanged
        );
        assert!(dependency_edit
            .report
            .errors
            .iter()
            .any(|error| error.code == "SSA011"));
        assert!(dependency_edit.facts.is_empty());

        let mut forged_witness = cold.facts[0].clone();
        forged_witness.verifier_identity = "untrusted-verifier/1".to_owned();
        let forged = cold
            .module
            .validate_with_verified_facts(&SsaFunctionFactRequest {
                prior_facts: vec![forged_witness],
            });
        assert!(forged.reuse.reused.is_empty());
        assert_eq!(
            forged.reuse.recomputed[0].reason,
            SsaFunctionFactReuseMiss::VerifierChanged
        );

        let mut duplicate_subject = cold.module.clone();
        let mut duplicate = duplicate_subject.functions[0].clone();
        duplicate.identity = SemanticId("mncs:test:duplicate-ssa-function".to_owned());
        duplicate_subject.functions.push(duplicate);
        let duplicate_check =
            duplicate_subject.validate_with_verified_facts(&SsaFunctionFactRequest {
                prior_facts: cold.facts.clone(),
            });
        assert!(duplicate_check.reuse.reused.is_empty());
        assert!(duplicate_check
            .reuse
            .recomputed
            .iter()
            .all(|row| { row.reason == SsaFunctionFactReuseMiss::DuplicateSubjectFunctions }));
        assert!(duplicate_check.facts.is_empty());
    }

    #[test]
    fn language_ssa_artifact_cache_reuses_unchanged_functions_after_restart() {
        let mut program = executable_program();
        let mut independent = program.functions[0].clone();
        independent.name = "independent".to_owned();
        program.functions.push(independent);

        let hir = program.lower_to_ir().expect("HIR");
        let mut cache = SsaFunctionArtifactCache::default();
        let cold = program
            .lower_to_ssa_from_ir_with_artifact_cache(&hir, &mut cache)
            .expect("cold SSA lowering");
        assert!(cold.artifact_reuse.reused.is_empty());
        assert_eq!(cold.artifact_reuse.recomputed.len(), 2);
        assert!(cold
            .artifact_reuse
            .recomputed
            .iter()
            .all(|entry| { entry.reason == SsaFunctionArtifactReuseMiss::Missing }));
        assert_eq!(cold.artifact_reuse.published_entry_count, 2);
        assert!(cold.module.validate().valid);

        // A fresh process restores the Language-owned cache and can reuse the
        // complete SSA function bodies without running their lowering loop.
        let serialized = serde_json::to_vec(&cache).expect("artifact cache serializes");
        let mut cache: SsaFunctionArtifactCache =
            serde_json::from_slice(&serialized).expect("artifact cache restores");
        let warm = program
            .lower_to_ssa_from_ir_with_artifact_cache(&hir, &mut cache)
            .expect("warm SSA lowering");
        assert_eq!(warm.module, cold.module);
        assert_eq!(warm.artifact_reuse.recomputed.len(), 0);
        assert_eq!(warm.artifact_reuse.reused.len(), 2);
        assert_eq!(warm.reuse.reused.len(), 2);
        assert!(warm.reuse.recomputed.is_empty());

        let mut tampered_cache = cache.clone();
        tampered_cache.entries[0].artifact_fingerprint = "sha256:invalid".to_owned();
        let tampered = program
            .lower_to_ssa_from_ir_with_artifact_cache(&hir, &mut tampered_cache)
            .expect("tampered cache falls back to fresh lowering");
        assert_eq!(tampered.module, cold.module);
        assert_eq!(tampered.artifact_reuse.reused.len(), 1);
        assert_eq!(tampered.artifact_reuse.recomputed.len(), 1);
        assert_eq!(
            tampered.artifact_reuse.recomputed[0].reason,
            SsaFunctionArtifactReuseMiss::ArtifactIntegrityFailed
        );
        assert_eq!(tampered.artifact_reuse.published_entry_count, 2);

        let cached_before_failure = serde_json::to_vec(&cache).expect("cache serializes");
        let mut invalid_program = program.clone();
        invalid_program.schema_version = "mncs.invalid/0".to_owned();
        assert!(invalid_program
            .lower_to_ssa_from_ir_with_artifact_cache(&hir, &mut cache)
            .is_err());
        assert_eq!(
            serde_json::to_vec(&cache).expect("cache serializes"),
            cached_before_failure,
            "failed verification cannot publish partial cache entries"
        );

        let mut changed = program;
        let operation =
            &mut changed.functions[0].body.as_mut().expect("body").blocks[0].operations[0];
        match &mut operation.kind {
            BodyOperationKind::Constant { value, .. } => *value = 2,
            other => panic!("expected a constant operation, got {other:?}"),
        }
        let changed_hir = changed.lower_to_ir().expect("changed HIR");
        let incremental = changed
            .lower_to_ssa_from_ir_with_artifact_cache(&changed_hir, &mut cache)
            .expect("incremental SSA lowering");
        assert!(incremental.module.validate().valid);
        assert_ne!(
            incremental.module.hir_fingerprint,
            warm.module.hir_fingerprint
        );
        assert_eq!(incremental.artifact_reuse.reused.len(), 1);
        assert!(incremental.artifact_reuse.reused[0]
            .as_str()
            .ends_with("::independent"));
        assert_eq!(incremental.artifact_reuse.recomputed.len(), 1);
        assert_eq!(
            incremental.artifact_reuse.recomputed[0].reason,
            SsaFunctionArtifactReuseMiss::InputChanged
        );
        assert_eq!(incremental.reuse.reused.len(), 1);
        assert_eq!(incremental.reuse.recomputed.len(), 1);
        assert_eq!(incremental.artifact_reuse.published_entry_count, 2);
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
