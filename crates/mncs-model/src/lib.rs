//! Executable semantic model for the MNCS Language Project.
//!
//! This crate deliberately models semantics before surface syntax. The JSON
//! representation is a transport format for experiments, not a proposed final
//! language grammar.

mod body;
mod canonical;
mod cfg;
mod compiler;
mod compiler_architecture;
mod core;
mod delta;
mod evidence;
mod execution;
mod experiment;
mod graph;
mod identity;
mod ir;
mod machine_intent;
mod obligations;
mod provenance;
mod refinement;
mod ssa;
mod ssa_execution;
mod translation;
mod validation;
mod verifier;

pub use body::{
    BodyBlock, BodyBoundedIteration, BodyCyclePolicy, BodyOperation, BodyOperationKind,
    BodyParameter, BodyTerminator, BodyType, BodyValue, BoundedIterationCompletion, FunctionBody,
    LoweringEnvelope, MachineIntentSpec, PortabilityEnvelope, PortabilityTarget, RealizationClass,
    EXECUTABLE_BODY_SCHEMA_VERSION, SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND,
};
pub use canonical::{CanonicalError, CanonicalForm, CANONICAL_SCHEMA_VERSION};
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
    EvidenceStatus, FailureMode, FiniteType, FiniteVariant, Function, ParseError, Program,
    RecordField, RecordType, Value, SUPPORTED_SCHEMA_VERSION,
};
pub use delta::{AuthorityDelta, EvidenceDelta, FailureChange, SemanticChangeSet, SemanticDelta};
pub use evidence::{
    EvidenceFreshness, EvidenceManifest, EvidenceRecord, EvidenceState, EvidenceStatusReport,
};
pub use execution::{
    compare as compare_execution, execute, execute_with_policy, execution_corpus_schema_supported,
    ComparisonStatus, EffectExecutionPolicy, ExecutionCase, ExecutionComparison, ExecutionCorpus,
    ExecutionEffectEvent, ExecutionFailure, ExecutionPolicy, ExecutionProperty, ExecutionRequest,
    ExecutionResult, ExecutionStatus, ExecutionSubject, ExecutionTarget, ExecutionTraceEntry,
    ExecutionValue, ExpectedEffectObservation, EXECUTION_COMPARISON_SCHEMA_VERSION,
    EXECUTION_CORPUS_SCHEMA_VERSION, EXECUTION_CORPUS_SCHEMA_VERSION_0_2,
    EXECUTION_REQUEST_SCHEMA_VERSION, EXECUTION_RESULT_SCHEMA_VERSION, MAX_EXECUTION_BUDGET,
};
pub use experiment::{
    BoundedRefinementCandidateDecision, BoundedRefinementCycle, FamilyExperimentObservation,
    FamilyExperimentReference, LanguageExperimentCaseObservation, LanguageExperimentComparison,
    LanguageExperimentDefinition, LanguageExperimentPropertyObservation, LanguageExperimentResult,
    LanguageExperimentStatus, ValidatorRequirement, BOUNDED_REFINEMENT_CYCLE_SCHEMA_VERSION,
    FAMILY_EXPERIMENT_REFERENCE_SCHEMA_VERSION, LANGUAGE_EXPERIMENT_DEFINITION_CONTRACT_ID,
    LANGUAGE_EXPERIMENT_INTERPRETATION, LANGUAGE_EXPERIMENT_RESULT_CONTRACT_ID,
    LANGUAGE_EXPERIMENT_SCHEMA_VERSION,
};
pub use graph::{
    EdgeKind, GraphEdge, GraphError, GraphNode, InvalidationReason, InvalidationReport,
    SemanticGraph,
};
pub use identity::{
    contract_id, finite_type_id, finite_variant_id, function_id, record_type_id, IdentityChange,
    IdentityKind, IdentityRecord, SemanticDiff, SemanticId, SemanticIdentities,
};
pub use ir::{
    CapabilityUse, FailurePathKind, HighLevelIr, IrBlock, IrBoundedIteration, IrError, IrFunction,
    IrOperation, IrOperationKind, IrStateRegion, IrTransition, IrType, IrValue, MachineIntentLinks,
    PathKind, StateRegionKind, TraceEntry, TraceMap, HIGH_LEVEL_IR_SCHEMA_VERSION,
};
pub use machine_intent::{
    arithmetic_result_type, minimum_widening_bits, AlignmentCapability, ArithmeticIntent,
    BackendPromise, BackendPromiseCertificate, BackendPromiseDecision, Fact, HighLevelIrNode,
    IntegerEvaluation, IntegerOperation, IntegerType, Intent, MachineIntentExpression,
    MachinePreference, Obligation, ObligationStatus, Preference, Requirement,
    BACKEND_PROMISE_CERTIFICATE_SCHEMA_VERSION,
};
pub use obligations::{
    generate_machine_intent_obligations, ObligationGeneration, ObligationRecord,
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
pub use ssa::{
    SsaBlock, SsaBoundedIteration, SsaDiagnostic, SsaError, SsaFunction, SsaInstruction,
    SsaInstructionKind, SsaModule, SsaTerminator, SsaTraceEntry, SsaTraceMap,
    SsaTransformationDecision, SsaValidationReport, SsaValue, SSA_SCHEMA_VERSION,
};
pub use ssa_execution::{
    compare_body_and_ssa, execute_ssa, execute_ssa_module, LoweringDivergenceContext,
    LoweringExecutionComparison, LoweringExecutionMismatch, LoweringExecutionStatus,
    SsaExecutionResult, SsaExecutionTraceEntry, LOWERING_EXECUTION_COMPARISON_SCHEMA_VERSION,
    SSA_EXECUTION_RESULT_SCHEMA_VERSION,
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
