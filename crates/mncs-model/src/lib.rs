//! Executable semantic model for the MNCS Language Project.
//!
//! This crate deliberately models semantics before surface syntax. The JSON
//! representation is a transport format for experiments, not a proposed final
//! language grammar.

mod canonical;
mod core;
mod delta;
mod evidence;
mod graph;
mod identity;
mod ir;
mod machine_intent;
mod obligations;
mod refinement;
mod validation;
mod verifier;

pub use canonical::{CanonicalError, CanonicalForm, CANONICAL_SCHEMA_VERSION};
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
pub use refinement::{
    AuthorityCapability, AuthorityKind, AuthorityScope, CandidateEvaluation, CandidateState,
    CausalSlice, CausalSliceEdge, Confidence, DiagnosticCategory, DiagnosticObligation,
    PromotionDecision, PromotionDisposition, RefinementBudget, RefinementError, RepairProposal,
    ResourceLimits, SemanticChange, VerificationPlan,
};
pub use validation::{Diagnostic, ValidationReport, ValidationSummary};
pub use verifier::{
    AlignmentVerifierInput, CapabilityVerifierInput, DeterministicVerifier, IntegerVerifierInput,
    MicroVerifier, VerifierIdentity, VerifierInput, VerifierMethod, VerifierRequest,
    VerifierResult,
};
