use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::identity::{
    assumption_id, capability_id, contract_id, diff_identities, effect_id, evidence_id,
    function_id, program_id, IdentityKind, SemanticId,
};
use crate::{Program, SemanticDiff, SemanticIdentities, ValidationReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    ContainsFunction,
    OwnsContract,
    DeclaresEffect,
    DeclaresCapability,
    RequiresCapability,
    ConsumesAssumption,
    SupportsProperty,
    HasSubject,
    DependsOn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNode {
    pub identity: SemanticId,
    pub kind: IdentityKind,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: SemanticId,
    pub to: SemanticId,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticGraph {
    pub schema_version: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidationReport {
    pub changed: Vec<SemanticId>,
    pub invalidated_evidence: Vec<SemanticId>,
    pub reasons: Vec<InvalidationReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidationReason {
    pub evidence: SemanticId,
    pub dependency: SemanticId,
    pub edge: EdgeKind,
}

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("cannot construct a semantic graph from an invalid program")]
    InvalidProgram(ValidationReport),
}

impl Program {
    pub fn semantic_graph(&self) -> Result<SemanticGraph, GraphError> {
        if !self.validate().valid {
            return Err(GraphError::InvalidProgram(self.validate()));
        }
        Ok(build_graph(self, &self.semantic_identities()))
    }

    pub fn invalidation_from(&self, after: &Program) -> Result<InvalidationReport, GraphError> {
        let before_graph = self.semantic_graph()?;
        let after_graph = after.semantic_graph()?;
        let diff = diff_identities(&self.semantic_identities(), &after.semantic_identities());
        Ok(invalidate(&before_graph, &after_graph, &diff))
    }
}

impl SemanticGraph {
    pub fn canonical_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn invalidate(&self, changed: &[SemanticId]) -> InvalidationReport {
        let changed: Vec<_> = changed.to_vec();
        let changed_set: BTreeSet<_> = changed.iter().collect();
        let mut reasons = Vec::new();
        for edge in &self.edges {
            let is_dependency_edge = matches!(
                edge.kind,
                EdgeKind::SupportsProperty | EdgeKind::HasSubject | EdgeKind::DependsOn
            );
            if is_dependency_edge && changed_set.contains(&edge.to) {
                reasons.push(InvalidationReason {
                    evidence: edge.from.clone(),
                    dependency: edge.to.clone(),
                    edge: edge.kind,
                });
            }
        }
        reasons.retain(|reason| self.is_evidence(&reason.evidence));
        reasons.sort_by(|left, right| {
            left.evidence
                .cmp(&right.evidence)
                .then(left.dependency.cmp(&right.dependency))
                .then(left.edge.cmp(&right.edge))
        });
        let invalidated_evidence = reasons
            .iter()
            .map(|reason| reason.evidence.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        InvalidationReport {
            changed,
            invalidated_evidence,
            reasons,
        }
    }

    fn is_evidence(&self, identity: &SemanticId) -> bool {
        self.nodes
            .iter()
            .any(|node| node.identity == *identity && node.kind == IdentityKind::Evidence)
    }
}

fn build_graph(program: &Program, identities: &SemanticIdentities) -> SemanticGraph {
    let nodes = identities
        .objects
        .iter()
        .map(|record| GraphNode {
            identity: record.identity.clone(),
            kind: record.kind,
            fingerprint: record.fingerprint.clone(),
        })
        .collect::<Vec<_>>();
    let mut edges = Vec::new();
    let program_identity = program_id(&program.module);

    for function in &program.functions {
        let function_identity = function_id(&program.module, &function.name);
        edges.push(GraphEdge {
            from: program_identity.clone(),
            to: function_identity.clone(),
            kind: EdgeKind::ContainsFunction,
        });
        for contract in &function.contracts {
            edges.push(GraphEdge {
                from: function_identity.clone(),
                to: contract_id(&program.module, &function.name, &contract.id),
                kind: EdgeKind::OwnsContract,
            });
        }
        for capability in &function.capabilities {
            edges.push(GraphEdge {
                from: function_identity.clone(),
                to: capability_id(&program.module, &function.name, capability),
                kind: EdgeKind::DeclaresCapability,
            });
        }
        let mut effect_occurrences = BTreeMap::<String, usize>::new();
        for effect in &function.effects {
            let key = serde_json::to_string(&crate::canonical::canonical_effect(effect))
                .expect("canonical effect");
            let occurrence = effect_occurrences.entry(key.clone()).or_default();
            let effect_identity = effect_id(&program.module, &function.name, &key, *occurrence);
            edges.push(GraphEdge {
                from: function_identity.clone(),
                to: effect_identity.clone(),
                kind: EdgeKind::DeclaresEffect,
            });
            edges.push(GraphEdge {
                from: effect_identity,
                to: capability_id(&program.module, &function.name, &effect.capability),
                kind: EdgeKind::RequiresCapability,
            });
            *occurrence += 1;
        }
        for assumption in &function.assumptions {
            let assumption_identity = assumption_id(&program.module, assumption);
            edges.push(GraphEdge {
                from: function_identity.clone(),
                to: assumption_identity.clone(),
                kind: EdgeKind::ConsumesAssumption,
            });
            for evidence in &function.evidence {
                let key = serde_json::to_string(&crate::canonical::canonical_evidence(evidence))
                    .expect("canonical evidence");
                let occurrence = evidence_occurrence(function, evidence);
                edges.push(GraphEdge {
                    from: evidence_id(&program.module, &function.name, &key, occurrence),
                    to: assumption_identity.clone(),
                    kind: EdgeKind::DependsOn,
                });
            }
        }
        let mut evidence_occurrences = BTreeMap::<String, usize>::new();
        for evidence in &function.evidence {
            let key = serde_json::to_string(&crate::canonical::canonical_evidence(evidence))
                .expect("canonical evidence");
            let occurrence = evidence_occurrences.entry(key.clone()).or_default();
            let evidence_identity = evidence_id(&program.module, &function.name, &key, *occurrence);
            edges.push(GraphEdge {
                from: evidence_identity.clone(),
                to: function_identity.clone(),
                kind: EdgeKind::HasSubject,
            });
            edges.push(GraphEdge {
                from: evidence_identity,
                to: contract_id(&program.module, &function.name, &evidence.property),
                kind: EdgeKind::SupportsProperty,
            });
            *occurrence += 1;
        }
    }
    edges.sort_by(|left, right| {
        left.from
            .cmp(&right.from)
            .then(left.to.cmp(&right.to))
            .then(left.kind.cmp(&right.kind))
    });
    SemanticGraph {
        schema_version: "0.2".to_owned(),
        nodes,
        edges,
    }
}

fn evidence_occurrence(function: &crate::Function, evidence: &crate::EvidenceClaim) -> usize {
    let key = serde_json::to_string(&crate::canonical::canonical_evidence(evidence))
        .expect("canonical evidence");
    function
        .evidence
        .iter()
        .take_while(|candidate| !std::ptr::eq(*candidate, evidence))
        .filter(|candidate| {
            serde_json::to_string(&crate::canonical::canonical_evidence(candidate))
                .expect("canonical evidence")
                == key
        })
        .count()
}

fn invalidate(
    before: &SemanticGraph,
    after: &SemanticGraph,
    diff: &SemanticDiff,
) -> InvalidationReport {
    let mut changed: BTreeSet<_> = diff
        .changed
        .iter()
        .map(|change| change.identity.clone())
        .chain(diff.removed.iter().map(|record| record.identity.clone()))
        .chain(diff.added.iter().map(|record| record.identity.clone()))
        .collect();
    let before_ids: BTreeSet<_> = before
        .nodes
        .iter()
        .map(|node| node.identity.clone())
        .collect();
    let after_ids: BTreeSet<_> = after
        .nodes
        .iter()
        .map(|node| node.identity.clone())
        .collect();
    changed.extend(before_ids.symmetric_difference(&after_ids).cloned());
    before.invalidate(&changed.into_iter().collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use crate::validation::tests::valid_program;
    use crate::EdgeKind;

    #[test]
    fn graph_is_deterministic_and_contains_authority_edges() {
        let graph = valid_program().semantic_graph().expect("graph");
        assert_eq!(graph, valid_program().semantic_graph().expect("graph"));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::RequiresCapability));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.kind == EdgeKind::SupportsProperty));
    }

    #[test]
    fn contract_change_invalidates_supporting_evidence() {
        let before = valid_program();
        let mut after = before.clone();
        "amount >= 0".clone_into(&mut after.functions[0].contracts[0].expression);
        let report = before.invalidation_from(&after).expect("invalidation");
        assert_eq!(report.invalidated_evidence.len(), 1);
    }
}
