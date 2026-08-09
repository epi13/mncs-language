use std::collections::{BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::canonical::sha256_hex;
use crate::identity::{contract_id, program_id};
use crate::{
    ArithmeticIntent, BodyOperation, BodyOperationKind, EdgeKind, EvidenceState, GraphError,
    MicroVerifier, ObligationGeneration, Program, SemanticDelta, SemanticGraph, SemanticId,
    VerifierResult,
};

pub const REFINEMENT_SCHEMA_VERSION: &str = "0.2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCategory {
    Validation,
    Authority,
    Verification,
    Evidence,
    Lowering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticObligation {
    pub schema_version: String,
    pub identity: SemanticId,
    pub subject: SemanticId,
    pub property: Option<SemanticId>,
    pub obligation: Option<SemanticId>,
    pub category: DiagnosticCategory,
    pub evidence_state: EvidenceState,
    pub assumptions: Vec<SemanticId>,
    pub dependencies: Vec<SemanticId>,
    pub severity: u8,
    pub confidence: Confidence,
    pub fallback: Option<String>,
    pub message: String,
}

impl DiagnosticObligation {
    pub fn new(identity: SemanticId, subject: SemanticId, category: DiagnosticCategory) -> Self {
        Self {
            schema_version: REFINEMENT_SCHEMA_VERSION.to_owned(),
            identity,
            subject,
            property: None,
            obligation: None,
            category,
            evidence_state: EvidenceState::Unknown,
            assumptions: Vec::new(),
            dependencies: Vec::new(),
            severity: 1,
            confidence: Confidence::Low,
            fallback: None,
            message: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalSlice {
    pub schema_version: String,
    pub root: SemanticId,
    pub nodes: Vec<SemanticId>,
    pub edges: Vec<CausalSliceEdge>,
    pub complete: bool,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalSliceEdge {
    pub from: SemanticId,
    pub to: SemanticId,
    pub kind: EdgeKind,
}

impl SemanticGraph {
    /// Return the currently defensible backward semantic neighborhood.
    ///
    /// Edges are followed from their target to their source, so a slice rooted
    /// at an obligation or operation contains the values, effects, contracts,
    /// and authority declarations that can contribute to it. This is still
    /// conservative: the graph does not yet encode complete path-sensitive
    /// control/data-flow or verifier-artifact causality.
    pub fn causal_slice(&self, root: &SemanticId) -> CausalSlice {
        let known: BTreeSet<_> = self
            .nodes
            .iter()
            .map(|node| node.identity.clone())
            .collect();
        let mut nodes = BTreeSet::new();
        let mut queue = VecDeque::new();
        nodes.insert(root.clone());
        queue.push_back(root.clone());
        while let Some(current) = queue.pop_front() {
            for edge in &self.edges {
                if edge.to == current && nodes.insert(edge.from.clone()) {
                    queue.push_back(edge.from.clone());
                }
            }
        }
        let mut edges = self
            .edges
            .iter()
            .filter(|edge| nodes.contains(&edge.from) && nodes.contains(&edge.to))
            .map(|edge| CausalSliceEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
                kind: edge.kind,
            })
            .collect::<Vec<_>>();
        edges.sort_by(|left, right| {
            left.from
                .cmp(&right.from)
                .then(left.to.cmp(&right.to))
                .then(left.kind.cmp(&right.kind))
        });
        let mut limitations = vec![
            "slice follows conservative backward semantic dependencies".to_owned(),
            "path-sensitive control-flow and complete verifier/artifact causality are not represented"
                .to_owned(),
        ];
        if !known.contains(root) {
            limitations.push("root is not a current semantic graph node".to_owned());
        }
        CausalSlice {
            schema_version: REFINEMENT_SCHEMA_VERSION.to_owned(),
            root: root.clone(),
            nodes: nodes.into_iter().collect(),
            edges,
            complete: false,
            limitations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_steps: Option<u64>,
    pub max_bytes: Option<u64>,
    pub max_wall_time_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefinementBudget {
    pub max_iterations: u32,
    pub max_candidates: u32,
    pub max_verifier_calls: u32,
    pub mutation_scope: Vec<SemanticId>,
    pub allowed_capabilities: Vec<AuthorityCapability>,
    pub resources: ResourceLimits,
}

impl Default for RefinementBudget {
    fn default() -> Self {
        Self {
            max_iterations: 1,
            max_candidates: 1,
            max_verifier_calls: 1,
            mutation_scope: Vec::new(),
            allowed_capabilities: Vec::new(),
            resources: ResourceLimits {
                max_steps: Some(10_000),
                max_bytes: Some(16 * 1024 * 1024),
                max_wall_time_ms: Some(30_000),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityKind {
    Observe,
    Mutate,
    Execute,
    Verify,
    Generate,
    Promote,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "scope_kind", content = "subject")]
pub enum AuthorityScope {
    Exact(SemanticId),
    Function(SemanticId),
    Module(SemanticId),
    Global,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityCapability {
    pub identity: SemanticId,
    pub kind: AuthorityKind,
    pub scope: AuthorityScope,
}

impl AuthorityCapability {
    pub fn permits(&self, kind: AuthorityKind, subject: &SemanticId) -> bool {
        self.kind == kind && self.scope.covers(subject)
    }

    pub fn broadens_from(&self, previous: &AuthorityCapability) -> bool {
        self.kind == previous.kind && self.scope.rank() > previous.scope.rank()
    }
}

impl AuthorityScope {
    fn rank(&self) -> u8 {
        match self {
            Self::Exact(_) => 0,
            Self::Function(_) => 1,
            Self::Module(_) => 2,
            Self::Global => 3,
        }
    }

    fn covers(&self, subject: &SemanticId) -> bool {
        match self {
            Self::Exact(value) => value == subject,
            Self::Function(value) => subject.0.starts_with(&format!("{}::", value.0)),
            Self::Module(value) => subject.0.starts_with(&format!("{}::", value.0)),
            Self::Global => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticChange {
    pub kind: String,
    pub subject: SemanticId,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticPatch {
    pub schema_version: String,
    pub identity: SemanticId,
    pub subject: SemanticId,
    pub precondition: Option<String>,
    pub operation: PatchOperation,
    pub required_authority: AuthorityCapability,
    pub expected_invalidation: Vec<SemanticId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchOperation {
    ReplaceContractExpression {
        before: String,
        after: String,
    },
    AddAssumptionReference {
        function: String,
        assumption: String,
    },
    RemoveAssumptionReference {
        function: String,
        assumption: String,
    },
    ChangeIntegerIntent {
        before: ArithmeticIntent,
        after: ArithmeticIntent,
    },
    AddRuntimeCheck {
        block: String,
        operation: Box<BodyOperation>,
    },
    SelectRealization {
        realization: SemanticId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectedPropertyStatus {
    Preserved,
    Regressed,
    Unknown,
    NotEvaluated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtectedPropertyResult {
    pub property: SemanticId,
    pub status: ProtectedPropertyStatus,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtectedPropertyEvaluation {
    pub schema_version: String,
    pub results: Vec<ProtectedPropertyResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationPlan {
    pub verifier_classes: Vec<String>,
    pub required_obligations: Vec<SemanticId>,
    pub independent_authority_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairProposal {
    pub schema_version: String,
    pub identity: SemanticId,
    pub generator: SemanticId,
    pub baseline_identity: SemanticId,
    pub baseline_fingerprint: String,
    pub objective: String,
    pub protected_properties: Vec<SemanticId>,
    pub permitted_mutation_region: Vec<SemanticId>,
    pub required_capabilities: Vec<AuthorityCapability>,
    pub changes: Vec<SemanticChange>,
    pub predicted_invalidation: Vec<SemanticId>,
    pub verification_plan: VerificationPlan,
    pub budget: RefinementBudget,
    pub trusted: bool,
    #[serde(default)]
    pub patches: Vec<SemanticPatch>,
}

impl RepairProposal {
    pub fn is_untrusted(&self) -> bool {
        !self.trusted
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionDisposition {
    Rejected,
    ReviewRequired,
    AcceptedWithConstraints,
    Accepted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionDecision {
    pub schema_version: String,
    pub baseline_identity: SemanticId,
    pub candidate_identity: SemanticId,
    pub policy_identity: SemanticId,
    pub evidence_consumed: Vec<SemanticId>,
    pub protected_property_result: String,
    pub accepted_tradeoffs: Vec<String>,
    pub approving_authority: Option<AuthorityCapability>,
    pub disposition: PromotionDisposition,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateState {
    pub schema_version: String,
    pub identity: SemanticId,
    pub baseline_identity: SemanticId,
    pub program: Program,
    pub delta: SemanticDelta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateEvaluation {
    pub candidate_identity: SemanticId,
    pub delta: SemanticDelta,
    pub obligations: ObligationGeneration,
    pub verifier_results: Vec<VerifierResult>,
}

#[derive(Debug, thiserror::Error)]
pub enum RefinementError {
    #[error("repair proposal baseline identity or fingerprint does not match trusted baseline")]
    BaselineMismatch,
    #[error("repair proposal mutation exceeds its declared region or bounded budget")]
    MutationNotPermitted,
    #[error("repair proposal has no explicit mutation capability for {0}")]
    MutationAuthorityMissing(SemanticId),
    #[error("unsupported semantic repair change kind {0:?}")]
    UnsupportedChange(String),
    #[error("repair precondition does not match current semantic content")]
    PreconditionMismatch,
    #[error("candidate semantic delta failed: {0}")]
    Delta(#[from] GraphError),
}

impl RepairProposal {
    /// Apply the deliberately tiny supported repair form to an isolated clone.
    /// The trusted `Program` is borrowed and never mutated or promoted.
    pub fn apply_isolated(&self, baseline: &Program) -> Result<CandidateState, RefinementError> {
        let baseline_identity = program_id(&baseline.module);
        let baseline_fingerprint = baseline
            .content_fingerprint()
            .map_err(|_| RefinementError::BaselineMismatch)?;
        if self.baseline_identity != baseline_identity
            || self.baseline_fingerprint != baseline_fingerprint
        {
            return Err(RefinementError::BaselineMismatch);
        }
        if self.budget.max_iterations == 0
            || self.budget.max_candidates == 0
            || self.budget.max_verifier_calls == 0
            || (self.patches.len() != 1 && self.changes.len() != 1)
            || (!self.patches.is_empty() && !self.changes.is_empty())
            || (self.patches.is_empty() && self.changes.len() != 1)
        {
            return Err(RefinementError::MutationNotPermitted);
        }
        if self.patches.len() > 1 || (!self.patches.is_empty() && !self.changes.is_empty()) {
            return Err(RefinementError::MutationNotPermitted);
        }
        if let Some(patch) = self.patches.first() {
            if !self.permitted_mutation_region.contains(&patch.subject)
                || !self
                    .required_capabilities
                    .iter()
                    .any(|capability| capability.permits(AuthorityKind::Mutate, &patch.subject))
            {
                return Err(RefinementError::MutationNotPermitted);
            }
            return patch.apply_isolated(baseline);
        }
        if self.changes.len() != 1 {
            return Err(RefinementError::MutationNotPermitted);
        }
        let change = &self.changes[0];
        if !self.permitted_mutation_region.contains(&change.subject) {
            return Err(RefinementError::MutationNotPermitted);
        }
        if !self
            .required_capabilities
            .iter()
            .any(|capability| capability.permits(AuthorityKind::Mutate, &change.subject))
        {
            return Err(RefinementError::MutationAuthorityMissing(
                change.subject.clone(),
            ));
        }
        if change.kind != "contract_expression" {
            return Err(RefinementError::UnsupportedChange(change.kind.clone()));
        }
        let Some(after_expression) = change.after.as_ref() else {
            return Err(RefinementError::UnsupportedChange(
                "contract removal".to_owned(),
            ));
        };
        let mut candidate = baseline.clone();
        let module = candidate.module.clone();
        let mut applied = false;
        for function in &mut candidate.functions {
            for contract in &mut function.contracts {
                let identity = contract_id(&module, &function.name, &contract.id);
                if identity != change.subject {
                    continue;
                }
                if change
                    .before
                    .as_deref()
                    .is_some_and(|before| before != contract.expression)
                {
                    return Err(RefinementError::PreconditionMismatch);
                }
                after_expression.clone_into(&mut contract.expression);
                applied = true;
            }
        }
        if !applied {
            return Err(RefinementError::UnsupportedChange(
                "unknown contract subject".to_owned(),
            ));
        }
        let delta = baseline.semantic_delta(&candidate)?;
        let candidate_fingerprint = candidate
            .content_fingerprint()
            .map_err(|_| RefinementError::BaselineMismatch)?;
        let candidate_identity = SemanticId(format!(
            "mncs:0.2:candidate:{}",
            sha256_hex(format!("{}:{}", self.identity, candidate_fingerprint).as_bytes())
        ));
        Ok(CandidateState {
            schema_version: REFINEMENT_SCHEMA_VERSION.to_owned(),
            identity: candidate_identity,
            baseline_identity,
            program: candidate,
            delta,
        })
    }
}

impl SemanticPatch {
    pub fn apply_isolated(&self, baseline: &Program) -> Result<CandidateState, RefinementError> {
        if !self
            .required_authority
            .permits(AuthorityKind::Mutate, &self.subject)
        {
            return Err(RefinementError::MutationAuthorityMissing(
                self.subject.clone(),
            ));
        }
        let mut candidate = baseline.clone();
        match &self.operation {
            PatchOperation::ReplaceContractExpression { before, after } => {
                let mut applied = false;
                for function in &mut candidate.functions {
                    for contract in &mut function.contracts {
                        let identity = contract_id(&candidate.module, &function.name, &contract.id);
                        if identity != self.subject {
                            continue;
                        }
                        if &contract.expression != before {
                            return Err(RefinementError::PreconditionMismatch);
                        }
                        after.clone_into(&mut contract.expression);
                        applied = true;
                    }
                }
                if !applied {
                    return Err(RefinementError::UnsupportedChange(
                        "unknown contract subject".to_owned(),
                    ));
                }
            }
            PatchOperation::AddAssumptionReference {
                function,
                assumption,
            } => {
                let function_identity = crate::identity::function_id(&candidate.module, function);
                if self.subject != function_identity {
                    return Err(RefinementError::MutationNotPermitted);
                }
                if !candidate
                    .assumptions
                    .iter()
                    .any(|item| item.id == *assumption)
                {
                    return Err(RefinementError::PreconditionMismatch);
                }
                let Some(target) = candidate
                    .functions
                    .iter_mut()
                    .find(|item| item.name == *function)
                else {
                    return Err(RefinementError::UnsupportedChange(
                        "unknown function subject".to_owned(),
                    ));
                };
                if !target.assumptions.contains(assumption) {
                    target.assumptions.push(assumption.clone());
                }
            }
            PatchOperation::RemoveAssumptionReference {
                function,
                assumption,
            } => {
                let function_identity = crate::identity::function_id(&candidate.module, function);
                if self.subject != function_identity {
                    return Err(RefinementError::MutationNotPermitted);
                }
                let Some(target) = candidate
                    .functions
                    .iter_mut()
                    .find(|item| item.name == *function)
                else {
                    return Err(RefinementError::UnsupportedChange(
                        "unknown function subject".to_owned(),
                    ));
                };
                if !target.assumptions.contains(assumption) {
                    return Err(RefinementError::PreconditionMismatch);
                }
                target.assumptions.retain(|item| item != assumption);
            }
            PatchOperation::ChangeIntegerIntent { before, after } => {
                let mut applied = false;
                for function in &mut candidate.functions {
                    if let Some(body) = &mut function.body {
                        for block in &mut body.blocks {
                            for operation in &mut block.operations {
                                if operation.identity(&candidate.module, &function.name, &block.id)
                                    != self.subject
                                {
                                    continue;
                                }
                                let BodyOperationKind::Integer { intent, .. } = &mut operation.kind
                                else {
                                    return Err(RefinementError::UnsupportedChange(
                                        "subject is not an integer operation".to_owned(),
                                    ));
                                };
                                if intent != before {
                                    return Err(RefinementError::PreconditionMismatch);
                                }
                                *intent = *after;
                                applied = true;
                            }
                        }
                    }
                }
                if !applied {
                    return Err(RefinementError::UnsupportedChange(
                        "unknown integer operation subject".to_owned(),
                    ));
                }
            }
            PatchOperation::AddRuntimeCheck { block, operation } => {
                let mut applied = false;
                for function in &mut candidate.functions {
                    if let Some(body) = &mut function.body {
                        for body_block in &mut body.blocks {
                            let identity =
                                crate::identity::block_id(&candidate.module, &function.name, block);
                            if identity
                                != crate::identity::block_id(
                                    &candidate.module,
                                    &function.name,
                                    &body_block.id,
                                )
                            {
                                continue;
                            }
                            body_block.operations.push(operation.as_ref().clone());
                            applied = true;
                        }
                    }
                }
                if !applied {
                    return Err(RefinementError::UnsupportedChange(
                        "unknown block subject".to_owned(),
                    ));
                }
            }
            PatchOperation::SelectRealization { .. } => {
                return Err(RefinementError::UnsupportedChange(
                    "realization selection is represented but not mutable in this subset"
                        .to_owned(),
                ));
            }
        }
        let delta = baseline.semantic_delta(&candidate)?;
        let candidate_fingerprint = candidate
            .content_fingerprint()
            .map_err(|_| RefinementError::BaselineMismatch)?;
        let candidate_identity = SemanticId(format!(
            "mncs:0.2:candidate:{}",
            sha256_hex(format!("{}:{}", self.identity, candidate_fingerprint).as_bytes())
        ));
        Ok(CandidateState {
            schema_version: REFINEMENT_SCHEMA_VERSION.to_owned(),
            identity: candidate_identity,
            baseline_identity: program_id(&baseline.module),
            program: candidate,
            delta,
        })
    }
}

impl CandidateState {
    pub fn evaluate<V: MicroVerifier>(&self, verifier: &V) -> CandidateEvaluation {
        CandidateEvaluation {
            candidate_identity: self.identity.clone(),
            delta: self.delta.clone(),
            obligations: self.program.generate_obligations(),
            verifier_results: self.program.verify_obligations(verifier),
        }
    }

    pub fn evaluate_protected_properties(
        &self,
        properties: &[SemanticId],
    ) -> ProtectedPropertyEvaluation {
        let evidence = self.program.evidence_manifest().ok();
        let results = properties
            .iter()
            .map(|property| {
                let changed = self
                    .delta
                    .identities
                    .changed
                    .iter()
                    .any(|change| change.identity == *property)
                    || self
                        .delta
                        .identities
                        .removed
                        .iter()
                        .any(|record| record.identity == *property);
                if changed {
                    return ProtectedPropertyResult {
                        property: property.clone(),
                        status: ProtectedPropertyStatus::Regressed,
                        reason: "candidate changed or removed the protected identity".to_owned(),
                    };
                }
                let stale = self.delta.invalidated_evidence.iter().any(|evidence_id| {
                    evidence.as_ref().is_some_and(|manifest| {
                        manifest.evidence.iter().any(|record| {
                            &record.identity == evidence_id && record.property == *property
                        })
                    })
                });
                if stale {
                    return ProtectedPropertyResult {
                        property: property.clone(),
                        status: ProtectedPropertyStatus::Unknown,
                        reason: "supporting evidence was invalidated".to_owned(),
                    };
                }
                if evidence.as_ref().is_some_and(|manifest| {
                    manifest.evidence.iter().any(|record| {
                        record.property == *property
                            && record.freshness == crate::EvidenceFreshness::Current
                    })
                }) {
                    ProtectedPropertyResult {
                        property: property.clone(),
                        status: ProtectedPropertyStatus::Preserved,
                        reason: "current evidence remains bound to the protected property"
                            .to_owned(),
                    }
                } else {
                    ProtectedPropertyResult {
                        property: property.clone(),
                        status: ProtectedPropertyStatus::NotEvaluated,
                        reason: "no current evidence established preservation".to_owned(),
                    }
                }
            })
            .collect();
        ProtectedPropertyEvaluation {
            schema_version: REFINEMENT_SCHEMA_VERSION.to_owned(),
            results,
        }
    }
}

impl PromotionDecision {
    pub fn is_explicitly_authorized(&self) -> bool {
        matches!(
            self.disposition,
            PromotionDisposition::Accepted | PromotionDisposition::AcceptedWithConstraints
        ) && self
            .approving_authority
            .as_ref()
            .is_some_and(|authority| authority.kind == AuthorityKind::Promote)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::tests::valid_program;

    #[test]
    fn causal_slice_is_deterministic_and_marks_incompleteness() {
        let graph = valid_program().semantic_graph().expect("graph");
        let root = graph
            .nodes
            .iter()
            .find(|node| node.kind == crate::IdentityKind::Contract)
            .expect("contract")
            .identity
            .clone();
        let left = graph.causal_slice(&root);
        assert_eq!(left, graph.causal_slice(&root));
        assert!(!left.complete);
        assert!(left.nodes.contains(&root));
        assert!(!left.edges.is_empty());
    }

    #[test]
    fn authority_kinds_do_not_imply_each_other_and_scope_broadening_is_visible() {
        let subject = SemanticId("mncs:0.2:function:Banking.Transfer::transfer".to_owned());
        let observe = AuthorityCapability {
            identity: SemanticId("authority:observe".to_owned()),
            kind: AuthorityKind::Observe,
            scope: AuthorityScope::Exact(subject.clone()),
        };
        assert!(observe.permits(AuthorityKind::Observe, &subject));
        assert!(!observe.permits(AuthorityKind::Mutate, &subject));
        let broad = AuthorityCapability {
            identity: SemanticId("authority:module-observe".to_owned()),
            kind: AuthorityKind::Observe,
            scope: AuthorityScope::Module(SemanticId(
                "mncs:0.2:function:Banking.Transfer".to_owned(),
            )),
        };
        assert!(broad.broadens_from(&observe));
    }

    #[test]
    fn default_budget_is_finite_and_proposals_are_untrusted() {
        let budget = RefinementBudget::default();
        assert_eq!(budget.max_iterations, 1);
        let proposal = RepairProposal {
            schema_version: REFINEMENT_SCHEMA_VERSION.to_owned(),
            identity: SemanticId("proposal:one".to_owned()),
            generator: SemanticId("generator:one".to_owned()),
            baseline_identity: SemanticId("program:one".to_owned()),
            baseline_fingerprint: "fingerprint".to_owned(),
            objective: "repair".to_owned(),
            protected_properties: Vec::new(),
            permitted_mutation_region: Vec::new(),
            required_capabilities: Vec::new(),
            changes: Vec::new(),
            predicted_invalidation: Vec::new(),
            verification_plan: VerificationPlan {
                verifier_classes: vec!["deterministic".to_owned()],
                required_obligations: Vec::new(),
                independent_authority_required: true,
            },
            budget,
            trusted: false,
            patches: Vec::new(),
        };
        assert!(proposal.is_untrusted());
    }

    #[test]
    fn isolated_candidate_changes_contract_without_mutating_baseline_or_promoting() {
        let baseline = valid_program();
        let subject = contract_id(&baseline.module, "transfer", "positive_amount");
        let proposal = RepairProposal {
            schema_version: REFINEMENT_SCHEMA_VERSION.to_owned(),
            identity: SemanticId("proposal:contract-repair".to_owned()),
            generator: SemanticId("generator:diagnostic-agent".to_owned()),
            baseline_identity: program_id(&baseline.module),
            baseline_fingerprint: baseline.content_fingerprint().expect("fingerprint"),
            objective: "tighten contract".to_owned(),
            protected_properties: vec![subject.clone()],
            permitted_mutation_region: vec![subject.clone()],
            required_capabilities: vec![AuthorityCapability {
                identity: SemanticId("authority:mutate-contract".to_owned()),
                kind: AuthorityKind::Mutate,
                scope: AuthorityScope::Exact(subject.clone()),
            }],
            changes: vec![SemanticChange {
                kind: "contract_expression".to_owned(),
                subject,
                before: Some("amount > 0".to_owned()),
                after: Some("amount >= 1".to_owned()),
            }],
            predicted_invalidation: Vec::new(),
            verification_plan: VerificationPlan {
                verifier_classes: vec!["deterministic".to_owned()],
                required_obligations: Vec::new(),
                independent_authority_required: true,
            },
            budget: RefinementBudget::default(),
            trusted: false,
            patches: Vec::new(),
        };
        let baseline_fingerprint = baseline.content_fingerprint().expect("fingerprint");
        let candidate = proposal.apply_isolated(&baseline).expect("candidate");
        assert_eq!(
            baseline.content_fingerprint().expect("fingerprint"),
            baseline_fingerprint
        );
        assert_ne!(
            candidate
                .program
                .content_fingerprint()
                .expect("fingerprint"),
            baseline_fingerprint
        );
        assert!(!candidate.delta.invalidated_evidence.is_empty());
        assert!(!proposal.trusted);

        let evaluation = candidate.evaluate(&crate::DeterministicVerifier::default());
        assert_eq!(evaluation.candidate_identity, candidate.identity);
        assert!(evaluation
            .verifier_results
            .iter()
            .all(|result| result.status == crate::ObligationStatus::Pass));
        let decision = PromotionDecision {
            schema_version: REFINEMENT_SCHEMA_VERSION.to_owned(),
            baseline_identity: candidate.baseline_identity,
            candidate_identity: candidate.identity,
            policy_identity: SemanticId("policy:review-required".to_owned()),
            evidence_consumed: Vec::new(),
            protected_property_result: "not independently reviewed".to_owned(),
            accepted_tradeoffs: Vec::new(),
            approving_authority: None,
            disposition: PromotionDisposition::ReviewRequired,
            reasons: vec!["generator output requires separate promotion authority".to_owned()],
        };
        assert!(!decision.is_explicitly_authorized());
    }

    #[test]
    fn isolated_candidate_requires_mutation_authority() {
        let baseline = valid_program();
        let subject = contract_id(&baseline.module, "transfer", "positive_amount");
        let mut proposal = RepairProposal {
            schema_version: REFINEMENT_SCHEMA_VERSION.to_owned(),
            identity: SemanticId("proposal:unauthorized".to_owned()),
            generator: SemanticId("generator:one".to_owned()),
            baseline_identity: program_id(&baseline.module),
            baseline_fingerprint: baseline.content_fingerprint().expect("fingerprint"),
            objective: "repair".to_owned(),
            protected_properties: Vec::new(),
            permitted_mutation_region: vec![subject.clone()],
            required_capabilities: Vec::new(),
            changes: vec![SemanticChange {
                kind: "contract_expression".to_owned(),
                subject,
                before: Some("amount > 0".to_owned()),
                after: Some("amount >= 1".to_owned()),
            }],
            predicted_invalidation: Vec::new(),
            verification_plan: VerificationPlan {
                verifier_classes: Vec::new(),
                required_obligations: Vec::new(),
                independent_authority_required: true,
            },
            budget: RefinementBudget::default(),
            trusted: false,
            patches: Vec::new(),
        };
        proposal.budget.max_candidates = 1;
        assert!(matches!(
            proposal.apply_isolated(&baseline),
            Err(RefinementError::MutationAuthorityMissing(_))
        ));
    }

    #[test]
    fn typed_patch_isolated_application_and_protected_property_evaluation_are_conservative() {
        let baseline = valid_program();
        let subject = contract_id(&baseline.module, "transfer", "positive_amount");
        let patch = SemanticPatch {
            schema_version: REFINEMENT_SCHEMA_VERSION.to_owned(),
            identity: SemanticId("patch:tighten-contract".to_owned()),
            subject: subject.clone(),
            precondition: Some("amount > 0".to_owned()),
            operation: PatchOperation::ReplaceContractExpression {
                before: "amount > 0".to_owned(),
                after: "amount >= 1".to_owned(),
            },
            required_authority: AuthorityCapability {
                identity: SemanticId("authority:contract-mutation".to_owned()),
                kind: AuthorityKind::Mutate,
                scope: AuthorityScope::Exact(subject.clone()),
            },
            expected_invalidation: Vec::new(),
        };
        let candidate = patch.apply_isolated(&baseline).expect("candidate");
        assert_ne!(
            candidate
                .program
                .content_fingerprint()
                .expect("candidate fingerprint"),
            baseline
                .content_fingerprint()
                .expect("baseline fingerprint")
        );
        let evaluation = candidate.evaluate_protected_properties(&[subject]);
        assert_eq!(
            evaluation.results[0].status,
            ProtectedPropertyStatus::Regressed
        );
    }
}
