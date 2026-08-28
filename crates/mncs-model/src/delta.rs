use serde::{Deserialize, Serialize};

use crate::identity::{IdentityChange, IdentityKind, IdentityRecord, SemanticDiff, SemanticId};
use crate::{FailureMode, GraphError, Program};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticChangeSet {
    pub added_bodies: Vec<IdentityRecord>,
    pub removed_bodies: Vec<IdentityRecord>,
    pub changed_bodies: Vec<IdentityChange>,
    pub added_operations: Vec<IdentityRecord>,
    pub removed_operations: Vec<IdentityRecord>,
    pub changed_operations: Vec<IdentityChange>,
    pub added_contracts: Vec<IdentityRecord>,
    pub removed_contracts: Vec<IdentityRecord>,
    pub changed_contracts: Vec<IdentityChange>,
    pub added_effects: Vec<IdentityRecord>,
    pub removed_effects: Vec<IdentityRecord>,
    pub changed_effects: Vec<IdentityChange>,
    pub added_capabilities: Vec<IdentityRecord>,
    pub removed_capabilities: Vec<IdentityRecord>,
    pub changed_assumptions: Vec<IdentityChange>,
    pub changed_failure_modes: Vec<FailureChange>,
    pub changed_machine_intents: Vec<IdentityChange>,
    pub changed_obligations: Vec<IdentityChange>,
    pub backend_promise_consequences: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureChange {
    pub function: SemanticId,
    pub before: FailureMode,
    pub after: FailureMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityDelta {
    pub added_capabilities: Vec<SemanticId>,
    pub removed_capabilities: Vec<SemanticId>,
    pub added_effects: Vec<SemanticId>,
    pub removed_effects: Vec<SemanticId>,
    pub authority_broadened: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceDelta {
    pub added: Vec<IdentityRecord>,
    pub removed: Vec<IdentityRecord>,
    pub changed: Vec<IdentityChange>,
    pub invalidated: Vec<SemanticId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticDelta {
    pub schema_version: String,
    pub identities: SemanticDiff,
    pub semantic: SemanticChangeSet,
    pub authority: AuthorityDelta,
    pub evidence: EvidenceDelta,
    pub invalidated_evidence: Vec<SemanticId>,
}

impl Program {
    pub fn semantic_delta(&self, after: &Program) -> Result<SemanticDelta, GraphError> {
        let before_identities = self.semantic_identities();
        let after_identities = after.semantic_identities();
        let identities = self.semantic_diff(after);
        let invalidation = self.invalidation_from(after)?;
        let semantic = SemanticChangeSet {
            added_bodies: by_kind(&identities.added, IdentityKind::Body),
            removed_bodies: by_kind(&identities.removed, IdentityKind::Body),
            changed_bodies: changed_by_kind(
                &identities.changed,
                &before_identities,
                &after_identities,
                IdentityKind::Body,
            ),
            added_operations: by_kind(&identities.added, IdentityKind::Operation),
            removed_operations: by_kind(&identities.removed, IdentityKind::Operation),
            changed_operations: changed_by_kind(
                &identities.changed,
                &before_identities,
                &after_identities,
                IdentityKind::Operation,
            ),
            added_contracts: by_kind(&identities.added, IdentityKind::Contract),
            removed_contracts: by_kind(&identities.removed, IdentityKind::Contract),
            changed_contracts: changed_by_kind(
                &identities.changed,
                &before_identities,
                &after_identities,
                IdentityKind::Contract,
            ),
            added_effects: by_kind(&identities.added, IdentityKind::Effect),
            removed_effects: by_kind(&identities.removed, IdentityKind::Effect),
            changed_effects: changed_by_kind(
                &identities.changed,
                &before_identities,
                &after_identities,
                IdentityKind::Effect,
            ),
            added_capabilities: by_kind(&identities.added, IdentityKind::Capability),
            removed_capabilities: by_kind(&identities.removed, IdentityKind::Capability),
            changed_assumptions: changed_by_kind(
                &identities.changed,
                &before_identities,
                &after_identities,
                IdentityKind::Assumption,
            ),
            changed_failure_modes: failure_changes(self, after),
            changed_machine_intents: changed_by_kind(
                &identities.changed,
                &before_identities,
                &after_identities,
                IdentityKind::MachineIntent,
            ),
            changed_obligations: changed_by_kind(
                &identities.changed,
                &before_identities,
                &after_identities,
                IdentityKind::Obligation,
            ),
            backend_promise_consequences: backend_promise_consequences(&identities),
        };
        let authority = AuthorityDelta {
            added_capabilities: semantic
                .added_capabilities
                .iter()
                .map(|record| record.identity.clone())
                .collect(),
            removed_capabilities: semantic
                .removed_capabilities
                .iter()
                .map(|record| record.identity.clone())
                .collect(),
            added_effects: semantic
                .added_effects
                .iter()
                .map(|record| record.identity.clone())
                .collect(),
            removed_effects: semantic
                .removed_effects
                .iter()
                .map(|record| record.identity.clone())
                .collect(),
            authority_broadened: !semantic.added_capabilities.is_empty()
                || !semantic.added_effects.is_empty(),
            reasons: authority_reasons(&semantic),
        };
        let evidence = EvidenceDelta {
            added: by_kind(&identities.added, IdentityKind::Evidence),
            removed: by_kind(&identities.removed, IdentityKind::Evidence),
            changed: changed_by_kind(
                &identities.changed,
                &before_identities,
                &after_identities,
                IdentityKind::Evidence,
            ),
            invalidated: invalidation.invalidated_evidence.clone(),
        };
        Ok(SemanticDelta {
            schema_version: "0.2".to_owned(),
            identities,
            semantic,
            authority,
            invalidated_evidence: invalidation.invalidated_evidence,
            evidence,
        })
    }
}

fn by_kind(records: &[IdentityRecord], kind: IdentityKind) -> Vec<IdentityRecord> {
    records
        .iter()
        .filter(|record| record.kind == kind)
        .cloned()
        .collect()
}

fn changed_by_kind(
    changes: &[IdentityChange],
    before: &crate::SemanticIdentities,
    after: &crate::SemanticIdentities,
    kind: IdentityKind,
) -> Vec<IdentityChange> {
    changes
        .iter()
        .filter(|change| {
            before.record(&change.identity).map(|record| record.kind) == Some(kind)
                || after.record(&change.identity).map(|record| record.kind) == Some(kind)
        })
        .cloned()
        .collect()
}

fn failure_changes(before: &Program, after: &Program) -> Vec<FailureChange> {
    let mut changes = Vec::new();
    for left in &before.functions {
        if let Some(right) = after.functions.iter().find(|function| {
            function.name == left.name
                && function.identity_namespace(&after.module)
                    == left.identity_namespace(&before.module)
        }) {
            if left.failure != right.failure {
                changes.push(FailureChange {
                    function: crate::identity::function_id(
                        left.identity_namespace(&before.module),
                        &left.name,
                    ),
                    before: left.failure.clone(),
                    after: right.failure.clone(),
                });
            }
        }
    }
    changes
}

fn authority_reasons(semantic: &SemanticChangeSet) -> Vec<String> {
    let mut reasons = Vec::new();
    if !semantic.added_capabilities.is_empty() {
        reasons.push("new capability declaration may broaden authority".to_owned());
    }
    if !semantic.added_effects.is_empty() {
        reasons.push("new effect declaration broadens the observable effect boundary".to_owned());
    }
    reasons
}

fn backend_promise_consequences(identities: &SemanticDiff) -> Vec<String> {
    let mut consequences = Vec::new();
    if !identities.added.is_empty()
        || !identities.removed.is_empty()
        || !identities.changed.is_empty()
    {
        consequences
            .push("backend promises must be re-evaluated for changed semantic subjects".to_owned());
    }
    if !identities.removed.is_empty() {
        consequences.push("removed semantic subjects cannot retain backend promises".to_owned());
    }
    consequences
}

#[cfg(test)]
mod tests {
    use crate::validation::tests::valid_program;

    #[test]
    fn delta_separates_semantic_authority_and_evidence_changes() {
        let before = valid_program();
        let mut after = before.clone();
        after.functions[0]
            .capabilities
            .push("NetworkPublish".to_owned());
        after.functions[0].effects.push(crate::Effect {
            kind: "network".to_owned(),
            target: "PublishingService".to_owned(),
            capability: "NetworkPublish".to_owned(),
        });
        let delta = before.semantic_delta(&after).expect("delta");
        assert!(delta.authority.authority_broadened);
        assert_eq!(delta.authority.added_capabilities.len(), 1);
        assert_eq!(delta.authority.added_effects.len(), 1);
        assert!(!delta.invalidated_evidence.is_empty());
        assert!(delta.semantic.added_contracts.is_empty());
    }
}
