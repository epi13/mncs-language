//! Executable semantic model for the MNCS Language Project.
//!
//! This crate deliberately models semantics before surface syntax. The JSON
//! representation is a transport format for experiments, not a proposed final
//! language grammar.

mod body;
mod canonical;
mod cfg;
mod core;
mod delta;
mod evidence;
mod graph;
mod identity;
mod ir;
mod machine_intent;
mod obligations;
mod provenance;
mod refinement;
mod ssa;
mod validation;
mod verifier;

pub use body::{
    BodyBlock, BodyOperation, BodyOperationKind, BodyParameter, BodyTerminator, BodyType,
    BodyValue, FunctionBody, LoweringEnvelope, MachineIntentSpec, PortabilityEnvelope,
    PortabilityTarget, RealizationClass, EXECUTABLE_BODY_SCHEMA_VERSION,
};
pub use canonical::{CanonicalError, CanonicalForm, CANONICAL_SCHEMA_VERSION};
pub use cfg::{Cfg, CfgBlock, CFG_SCHEMA_VERSION};
pub use core::{
    Assumption, AssumptionConfidence, ContractClause, ContractKind, Effect, EvidenceClaim,
    EvidenceStatus, FailureMode, Function, ParseError, Program, Value, SUPPORTED_SCHEMA_VERSION,
};
pub use delta::{AuthorityDelta, EvidenceDelta, FailureChange, SemanticChangeSet, SemanticDelta};
pub use evidence::{
    EvidenceFreshness, EvidenceManifest, EvidenceRecord, EvidenceState, EvidenceStatusReport,
};
pub use graph::{
    EdgeKind, GraphEdge, GraphError, GraphNode, InvalidationReason, InvalidationReport,
    SemanticGraph,
};
pub use identity::{
    IdentityChange, IdentityKind, IdentityRecord, SemanticDiff, SemanticId, SemanticIdentities,
};
pub use ir::{
    CapabilityUse, FailurePathKind, HighLevelIr, IrBlock, IrError, IrFunction, IrOperation,
    IrOperationKind, IrStateRegion, IrTransition, IrType, IrValue, MachineIntentLinks, PathKind,
    StateRegionKind, TraceEntry, TraceMap,
};
pub use machine_intent::{
    AlignmentCapability, ArithmeticIntent, BackendPromise, BackendPromiseDecision, Fact,
    HighLevelIrNode, IntegerEvaluation, IntegerOperation, IntegerType, Intent,
    MachineIntentExpression, MachinePreference, Obligation, ObligationStatus, Preference,
    Requirement,
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
    SsaBlock, SsaDiagnostic, SsaError, SsaFunction, SsaInstruction, SsaInstructionKind, SsaModule,
    SsaTerminator, SsaTraceEntry, SsaTraceMap, SsaTransformationDecision, SsaValidationReport,
    SsaValue, SSA_SCHEMA_VERSION,
};
pub use validation::{Diagnostic, ValidationReport, ValidationSummary};
pub use verifier::{
    AlignmentVerifierInput, CapabilityVerifierInput, DeterministicVerifier, EvidenceAuthorityClass,
    IntegerVerifierInput, MicroVerifier, VerifierArtifactError, VerifierIdentity,
    VerifierIndependence, VerifierInput, VerifierMethod, VerifierRequest, VerifierResult,
};
