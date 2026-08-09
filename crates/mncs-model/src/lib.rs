//! Executable semantic model for the MNCS Language Project.
//!
//! This crate deliberately models semantics before surface syntax. The JSON
//! representation is a transport format for experiments, not a proposed final
//! language grammar.

mod canonical;
mod core;
mod evidence;
mod graph;
mod identity;
mod machine_intent;
mod validation;

pub use canonical::{CanonicalError, CanonicalForm, CANONICAL_SCHEMA_VERSION};
pub use core::{
    Assumption, AssumptionConfidence, ContractClause, ContractKind, Effect, EvidenceClaim,
    EvidenceStatus, FailureMode, Function, ParseError, Program, Value, SUPPORTED_SCHEMA_VERSION,
};
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
pub use machine_intent::{
    AlignmentCapability, ArithmeticIntent, BackendPromise, BackendPromiseDecision, Fact,
    HighLevelIrNode, IntegerEvaluation, IntegerOperation, IntegerType, Intent,
    MachineIntentExpression, MachinePreference, Obligation, ObligationStatus, Preference,
    Requirement,
};
pub use validation::{Diagnostic, ValidationReport, ValidationSummary};
