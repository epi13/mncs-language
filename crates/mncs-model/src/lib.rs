//! Executable semantic model for the MNCS Language Project.
//!
//! This crate deliberately models semantics before surface syntax. The JSON
//! representation is a transport format for experiments, not a proposed final
//! language grammar.

mod authority;
mod bindings;
mod body;
mod canonical;
mod capability_gap;
mod cfg;
mod compiler;
mod compiler_architecture;
mod core;
mod cost;
mod delta;
mod evidence;
mod execution;
mod experiment;
pub mod fs_resource;
pub mod generics;
mod graph;
mod identity;
mod ir;
mod machine_intent;
mod obligations;
mod proof_dep;
mod proof_kernel;
mod proof_transport;
mod provenance;
mod refinement;
mod representation;
mod ssa;
mod ssa_execution;
mod termination;
mod translation;
mod validation;
mod verifier;

pub use authority::{
    accept_leg, canonical_envelope_bytes, check_issuance_binding, confirm_execution,
    evidence_standing, fold_capability, issuance_signed_bytes, requirement_identity,
    satisfied_by_attested_evidence, verify_decision_digest, verify_session_context, Acceptance,
    AuthorityError, AuthorityVerdict, CapabilityDecision, DecisionStatus, EvidenceStanding,
    EvidenceTerms, IssuerBinding, ParsedIssuance, ProofClass, RequirementLeg, SessionBinding,
    DECISION_DIGEST_ALG, DECISION_SCHEMA, EVIDENCE_SCHEMA, ISSUANCE_SIGNATURE_ALG,
    REQUIREMENT_SCHEMA,
};
pub use bindings::{
    ResolutionProvenance, SemanticBinding, SemanticBindingKind, SemanticBindingTable,
    SemanticNamespace, SemanticReference, SemanticScope, SEMANTIC_BINDING_SCHEMA_VERSION,
};
pub use body::{
    host_call_arity, host_call_effect_kind, BodyBlock, BodyBoundedIteration, BodyCyclePolicy,
    BodyOperation, BodyOperationKind, BodyParameter, BodyTerminator, BodyType, BodyValue,
    BoundedIterationCompletion, BoundsEvidence, FunctionBody, GenericArg, GenericParam,
    GenericParamKind, IterationDomain, LoweringEnvelope, MachineIntentSpec, PortabilityEnvelope,
    PortabilityTarget, RealizationClass, SequenceBound, EXECUTABLE_BODY_SCHEMA_VERSION,
    MAX_SEQUENCE_BOUND, MAX_VECTOR_LANES, MODEL_MAX_ITERATION_BOUND, MODEL_MAX_SEQUENCE_BOUND,
    SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND,
};
pub use canonical::sha256_hex;
pub use canonical::{CanonicalError, CanonicalForm, CANONICAL_SCHEMA_VERSION};
pub use capability_gap::{
    CapabilityGap, CapabilityGapError, GapObstruction, GapStatus, CAPABILITY_GAP_ARTIFACT_KIND,
    CAPABILITY_GAP_CONTRACT_REVISION,
};
pub use cfg::{Cfg, CfgBlock, CFG_SCHEMA_VERSION};
pub use compiler::{
    ArtifactRepresentation, BackendArtifact, BackendCapabilityManifest, BackendConfiguration,
    BackendEvidence, BackendFunctionValueContract, BackendIdentity, BackendResult,
    BackendValueContract, BuildHostIdentity, CompilationEmissions, CompilationEvidenceBundle,
    CompilationRequest, CompilationResult, CompilationStatus, CompilationStudyRequest,
    CompilationStudyResult, CompilerArtifactRef, CompilerDiagnostic, CompilerDiagnosticKind,
    CompilerHostIdentity, CompilerImplementationIdentity, CompilerNodeProfile,
    CompilerPassExecutionObservation, CompilerPassIdentity, CrossHostInvariants,
    FamilyArtifactReference, FamilyCompilerObservation, FamilyCompilerReference,
    ObservationModelRef, PassPipelineIdentity, RealizationRequest, RelationClaim,
    RunEnvironmentRef, TargetContractRef, TargetLoweringPlan, TransformationEdge,
    TransformationStatus, BACKEND_ARTIFACT_SCHEMA_VERSION, BACKEND_CAPABILITY_SCHEMA_VERSION,
    COMPILATION_STUDY_OBSERVATION_INTERPRETATION, COMPILATION_STUDY_RESULT_CONTRACT_ID,
    COMPILER_ARTIFACT_SCHEMA_VERSION, FAMILY_COMPILER_REFERENCE_SCHEMA_VERSION,
    LAYERED_EXECUTION_COMPARISON_INTERPRETATION, PORTABLE_WASM_MVP_BACKEND_NAME,
    PORTABLE_WASM_MVP_BACKEND_VERSION, PORTABLE_WASM_MVP_TARGET,
    REALIZATION_REQUEST_SCHEMA_VERSION,
};
pub use compiler_architecture::{
    CompilerArchitectureContract, CompilerStage, CompilerStageContract, StageAvailability,
    StageIntegration, COMPILER_STAGE_ARCHITECTURE_CONTRACT_ID,
    COMPILER_STAGE_ARCHITECTURE_SCHEMA_VERSION,
};
pub use core::{
    Assumption, AssumptionConfidence, ContractClause, ContractKind, Effect, EvidenceClaim,
    EvidenceStatus, FailureMode, FiniteType, FiniteVariant, Function, GenericSpecializationRecord,
    ParseError, Program, RecordField, RecordType, Value, SUPPORTED_SCHEMA_VERSION,
};
pub use cost::{
    cost_report, record_counter, record_exclusive_stage, record_stage, reset_cost_report,
    CostReport, COST_REPORT_SCHEMA_VERSION,
};
pub use delta::{AuthorityDelta, EvidenceDelta, FailureChange, SemanticChangeSet, SemanticDelta};
pub use evidence::{
    EvidenceFreshness, EvidenceManifest, EvidenceReceipt, EvidenceReceiptOutcome, EvidenceRecord,
    EvidenceState, EvidenceStatusReport, EVIDENCE_RECEIPT_SCHEMA_VERSION,
};
pub use execution::{
    compare as compare_execution, compare_floats, compare_stateful_results, evaluate_float,
    execute, execute_stateful_case, execute_stateful_case_owned,
    execute_stateful_case_with_checkpoint, execute_stateful_case_with_checkpoint_scoped,
    execute_with_policy, execution_corpus_schema_supported, float_value, lint_corpus,
    stateful_prefix_identity, BodyExecutionSession, ComparisonStatus, CorpusLintCase,
    CorpusLintReport, EffectExecutionPolicy, ExecutionCase, ExecutionComparison, ExecutionCorpus,
    ExecutionEffectEvent, ExecutionFailure, ExecutionPolicy, ExecutionProperty, ExecutionRequest,
    ExecutionResult, ExecutionStatus, ExecutionSubject, ExecutionTarget, ExecutionTraceEntry,
    ExecutionValue, ExpectedEffectObservation, HostGrant, StatefulArgument, StatefulCallResult,
    StatefulExecutionCase, StatefulExecutionCheckpoint, StatefulExecutionComparison,
    StatefulExecutionMismatch, StatefulExecutionResult, StatefulExecutionStep,
    StatefulStepObservation, CORPUS_LINT_REPORT_SCHEMA_VERSION,
    EXECUTION_COMPARISON_SCHEMA_VERSION, EXECUTION_CORPUS_SCHEMA_VERSION,
    EXECUTION_CORPUS_SCHEMA_VERSION_0_2, EXECUTION_CORPUS_SCHEMA_VERSION_0_3,
    EXECUTION_REQUEST_SCHEMA_VERSION, EXECUTION_RESULT_SCHEMA_VERSION, HOST_GRANT_MAX_BYTES,
    MAX_EXECUTION_BUDGET, MAX_STATEFUL_CALLS, STATEFUL_EXECUTION_COMPARISON_SCHEMA_VERSION,
    STATEFUL_EXECUTION_SCHEMA_VERSION,
};
pub use experiment::{
    BoundedRefinementCandidateDecision, BoundedRefinementCycle, FamilyExperimentObservation,
    FamilyExperimentReference, LanguageExperimentCaseObservation, LanguageExperimentComparison,
    LanguageExperimentDefinition, LanguageExperimentPropertyObservation, LanguageExperimentResult,
    LanguageExperimentStatefulCaseObservation, LanguageExperimentStatus, ValidatorRequirement,
    BOUNDED_REFINEMENT_CYCLE_SCHEMA_VERSION, FAMILY_EXPERIMENT_REFERENCE_SCHEMA_VERSION,
    LANGUAGE_EXPERIMENT_DEFINITION_CONTRACT_ID, LANGUAGE_EXPERIMENT_INTERPRETATION,
    LANGUAGE_EXPERIMENT_RESULT_CONTRACT_ID, LANGUAGE_EXPERIMENT_SCHEMA_VERSION,
};
pub use graph::{
    EdgeKind, GraphEdge, GraphError, GraphNode, InvalidationReason, InvalidationReport,
    SemanticGraph,
};
pub use identity::{
    binding_id, binding_id_for, contract_id, finite_type_id, finite_variant_id, function_id,
    generic_param_id, instantiation_id, module_id, record_field_id, record_type_id, reference_id,
    scope_id, specialization_id, IdentityChange, IdentityKind, IdentityRecord, SemanticDiff,
    SemanticId, SemanticIdentities,
};
pub use ir::{
    CapabilityUse, FailurePathKind, HighLevelIr, IrBlock, IrBoundedIteration, IrError, IrFunction,
    IrOperation, IrOperationKind, IrStateRegion, IrTransition, IrType, IrValue, MachineIntentLinks,
    PathKind, StateRegionKind, TraceEntry, TraceMap, HIGH_LEVEL_IR_SCHEMA_VERSION,
};
pub use machine_intent::{
    arithmetic_result_type, minimum_widening_bits, AlignmentCapability, ArithmeticIntent,
    BackendPromise, BackendPromiseCertificate, BackendPromiseDecision, DisjointCapability, Fact,
    FloatType, HighLevelIrNode, IntegerEvaluation, IntegerOperation, IntegerType, Intent,
    MachineIntentExpression, MachinePreference, MemoryRange, Obligation, ObligationStatus,
    Preference, Requirement, BACKEND_PROMISE_CERTIFICATE_SCHEMA_VERSION,
};
pub use obligations::{
    generate_machine_intent_obligations, ObligationGeneration, ObligationRecord,
    OBLIGATION_SCHEMA_VERSION,
};
pub use proof_dep::{
    corroborate_proof, dep_assumption_set, dep_assumptions, dep_check, dep_probe_defeq,
    dep_probe_eval, parse_proof_dep_corpus, run_dep_case, CorroborationPolicy, DepArtifact,
    DepAssumptionSet, DepAssumptionUse, DepBuffer, DepCell, DepCellSer, DepCorpusCase,
    DepCorroboration, DepEntry, DepTag, DepVerdict, Fuel, PROOF_DEP_BUFFER_CAPACITY,
    PROOF_DEP_FUEL, PROOF_DEP_KERNEL_ID, PROOF_DEP_MAX_UNIVERSE,
};
pub use proof_kernel::{
    kernel_backed_range_result, parse_proof_corpus, reference_check, ProofArtifact, ProofBinding,
    ProofCell, ProofCorpusCase, ProofTag, ProofVerdict, PROOF_ARTIFACT_SCHEMA_VERSION,
    PROOF_BUFFER_CAPACITY, PROOF_KERNEL_ID, PROOF_MAX_UNIVERSE,
};
pub use proof_transport::{
    validate_relationship_for_use, ProofAdmissionEvidence, ProofBindingRef, ProofRelationship,
    TransportMismatch, PROOF_DEPENDENCY_SLOTS, PROOF_EVIDENCE_RULE,
    PROOF_RELATIONSHIP_SCHEMA_VERSION,
};
pub use provenance::{
    evidence_is_current, Realization, RealizationError, RealizationSelection, TargetIdentity,
    TransformationRecord, PROVENANCE_SCHEMA_VERSION,
};
pub use refinement::{
    AuthorityCapability, AuthorityKind, AuthorityScope, CandidateEvaluation, CandidateState,
    CausalSlice, CausalSliceEdge, Confidence, DiagnosticCategory, DiagnosticObligation,
    PatchOperation, PromotionDecision, PromotionDisposition, ProtectedPropertyEvaluation,
    ProtectedPropertyResult, ProtectedPropertyStatus, RefinementBudget, RefinementError,
    RepairProposal, ResourceLimits, SemanticChange, SemanticPatch, VerificationPlan,
};
pub use representation::{
    select_representation, LayoutFamily, RepresentationCandidate, RepresentationContext,
    RepresentationDecision, RepresentationObservation, SpatialFacts,
};
pub use ssa::{
    SsaBlock, SsaBoundedIteration, SsaDiagnostic, SsaError, SsaFunction, SsaInstruction,
    SsaInstructionKind, SsaModule, SsaTerminator, SsaTraceEntry, SsaTraceMap,
    SsaTransformationDecision, SsaValidationReport, SsaValue, SSA_SCHEMA_VERSION,
};
pub use ssa_execution::{
    compare_body_and_ssa, execute_ssa, execute_ssa_module, execute_ssa_module_prevalidated,
    LoweringDivergenceContext, LoweringExecutionComparison, LoweringExecutionMismatch,
    LoweringExecutionStatus, SsaExecutionResult, SsaExecutionSession, SsaExecutionTraceEntry,
    LOWERING_EXECUTION_COMPARISON_SCHEMA_VERSION, SSA_EXECUTION_RESULT_SCHEMA_VERSION,
};
pub use termination::{
    parse_structural_decrease_claim, specialized_bounds_over_ceiling, verify_structural_decrease,
    StructuralDecreaseClaim, StructuralDecreaseLink, MODEL_MAX_CALL_DEPTH,
    STRUCTURAL_DECREASE_PROPERTY,
};
pub use translation::{
    TranslationCounterexample, TranslationJudgement, TranslationValidationResult,
    TRANSLATION_VALIDATION_CONTRACT_ID, TRANSLATION_VALIDATION_SCHEMA_VERSION,
};
pub use validation::{Diagnostic, ValidationReport, ValidationSummary};
pub use verifier::{
    AlignmentVerifierInput, CapabilityVerifierInput, DeterministicVerifier, EvidenceAuthorityClass,
    IntegerVerifierInput, MicroVerifier, VerifierArtifactError, VerifierIdentity,
    VerifierIndependence, VerifierInput, VerifierMethod, VerifierRequest, VerifierResult,
};
