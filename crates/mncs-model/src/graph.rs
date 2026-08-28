use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::identity::{
    assumption_id, capability_id, contract_id, diff_identities, effect_id, evidence_id,
    function_id, program_id, IdentityKind, SemanticId,
};
use crate::{BoundsEvidence, Program, SemanticDiff, SemanticIdentities, ValidationReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    ContainsFiniteType,
    ContainsFiniteVariant,
    ContainsFunction,
    ContainsBody,
    ContainsIteration,
    IterationContainsBlock,
    CarriesValue,
    ContainsBlock,
    ContainsOperation,
    ContainsValue,
    OwnsContract,
    DeclaresEffect,
    DeclaresCapability,
    RequiresCapability,
    ConsumesAssumption,
    SupportsProperty,
    HasSubject,
    DependsOn,
    ConsumesValue,
    ProducesValue,
    PerformsEffect,
    UsesCapability,
    Calls,
    ConstructsVariant,
    TestsVariant,
    RequiresObligation,
    EstablishesFact,
    ReferencesContract,
    TransitionsTo,
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

    for finite_type in &program.finite_types {
        edges.push(GraphEdge {
            from: program_identity.clone(),
            to: finite_type.identity.clone(),
            kind: EdgeKind::ContainsFiniteType,
        });
        for variant in &finite_type.variants {
            edges.push(GraphEdge {
                from: finite_type.identity.clone(),
                to: variant.identity.clone(),
                kind: EdgeKind::ContainsFiniteVariant,
            });
        }
    }

    for function in &program.functions {
        let namespace = function.identity_namespace(&program.module);
        let function_identity = function_id(namespace, &function.name);
        edges.push(GraphEdge {
            from: program_identity.clone(),
            to: function_identity.clone(),
            kind: EdgeKind::ContainsFunction,
        });
        for contract in &function.contracts {
            edges.push(GraphEdge {
                from: function_identity.clone(),
                to: contract_id(namespace, &function.name, &contract.id),
                kind: EdgeKind::OwnsContract,
            });
        }
        for capability in &function.capabilities {
            edges.push(GraphEdge {
                from: function_identity.clone(),
                to: capability_id(namespace, &function.name, capability),
                kind: EdgeKind::DeclaresCapability,
            });
        }
        let mut effect_occurrences = BTreeMap::<String, usize>::new();
        for effect in &function.effects {
            let key = serde_json::to_string(&crate::canonical::canonical_effect(effect))
                .expect("canonical effect");
            let occurrence = effect_occurrences.entry(key.clone()).or_default();
            let effect_identity = effect_id(namespace, &function.name, &key, *occurrence);
            edges.push(GraphEdge {
                from: function_identity.clone(),
                to: effect_identity.clone(),
                kind: EdgeKind::DeclaresEffect,
            });
            edges.push(GraphEdge {
                from: effect_identity,
                to: capability_id(namespace, &function.name, &effect.capability),
                kind: EdgeKind::RequiresCapability,
            });
            *occurrence += 1;
        }
        for assumption in &function.assumptions {
            let assumption_identity = assumption_id(namespace, assumption);
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
                    from: evidence_id(namespace, &function.name, &key, occurrence),
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
            let evidence_identity = evidence_id(namespace, &function.name, &key, *occurrence);
            edges.push(GraphEdge {
                from: evidence_identity.clone(),
                to: function_identity.clone(),
                kind: EdgeKind::HasSubject,
            });
            edges.push(GraphEdge {
                from: evidence_identity,
                to: contract_id(namespace, &function.name, &evidence.property),
                kind: EdgeKind::SupportsProperty,
            });
            *occurrence += 1;
        }
        if let Some(body) = &function.body {
            let body_identity = crate::identity::body_id(namespace, &function.name);
            edges.push(GraphEdge {
                from: function_identity.clone(),
                to: body_identity.clone(),
                kind: EdgeKind::ContainsBody,
            });
            let mut values = BTreeMap::new();
            for parameter in &body.parameters {
                let identity =
                    crate::identity::parameter_id(namespace, &function.name, &parameter.id);
                values.insert(parameter.id.clone(), identity.clone());
                edges.push(GraphEdge {
                    from: body_identity.clone(),
                    to: identity,
                    kind: EdgeKind::ContainsValue,
                });
            }
            for block in &body.blocks {
                let block_identity =
                    crate::identity::block_id(namespace, &function.name, &block.id);
                edges.push(GraphEdge {
                    from: body_identity.clone(),
                    to: block_identity.clone(),
                    kind: EdgeKind::ContainsBlock,
                });
                for parameter in &block.parameters {
                    let identity = crate::identity::value_id(
                        namespace,
                        &function.name,
                        &block.id,
                        "parameter",
                        &parameter.id,
                    );
                    values.insert(parameter.id.clone(), identity.clone());
                    edges.push(GraphEdge {
                        from: block_identity.clone(),
                        to: identity,
                        kind: EdgeKind::ContainsValue,
                    });
                }
                for operation in &block.operations {
                    let operation_identity =
                        operation.identity(namespace, &function.name, &block.id);
                    edges.push(GraphEdge {
                        from: block_identity.clone(),
                        to: operation_identity.clone(),
                        kind: EdgeKind::ContainsOperation,
                    });
                    for operand in &operation.operands {
                        if let Some(value) = values.get(operand) {
                            edges.push(GraphEdge {
                                from: operation_identity.clone(),
                                to: value.clone(),
                                kind: EdgeKind::ConsumesValue,
                            });
                        }
                    }
                    for result in &operation.results {
                        let value = operation.result_identity(
                            namespace,
                            &function.name,
                            &block.id,
                            &result.id,
                        );
                        values.insert(result.id.clone(), value.clone());
                        edges.push(GraphEdge {
                            from: operation_identity.clone(),
                            to: value,
                            kind: EdgeKind::ProducesValue,
                        });
                    }
                    if let crate::BodyOperationKind::Effect { effect, .. } = &operation.kind {
                        let canonical =
                            serde_json::to_string(&crate::canonical::canonical_effect(effect))
                                .expect("body effect");
                        let effect_identity = effect_id(
                            namespace,
                            &function.name,
                            &canonical,
                            function
                                .effects
                                .iter()
                                .position(|declared| declared == effect)
                                .unwrap_or(0),
                        );
                        edges.push(GraphEdge {
                            from: operation_identity.clone(),
                            to: effect_identity,
                            kind: EdgeKind::PerformsEffect,
                        });
                        edges.push(GraphEdge {
                            from: operation_identity.clone(),
                            to: capability_id(namespace, &function.name, &effect.capability),
                            kind: EdgeKind::UsesCapability,
                        });
                    }
                    match &operation.kind {
                        crate::BodyOperationKind::FiniteConstruct {
                            variant_identity, ..
                        } => edges.push(GraphEdge {
                            from: operation_identity.clone(),
                            to: variant_identity.clone(),
                            kind: EdgeKind::ConstructsVariant,
                        }),
                        crate::BodyOperationKind::FiniteIsVariant {
                            variant_identity, ..
                        } => edges.push(GraphEdge {
                            from: operation_identity.clone(),
                            to: variant_identity.clone(),
                            kind: EdgeKind::TestsVariant,
                        }),
                        crate::BodyOperationKind::Call {
                            function: callee,
                            required_capabilities,
                            effects,
                            ..
                        } => {
                            edges.push(GraphEdge {
                                from: operation_identity.clone(),
                                to: callee.clone(),
                                kind: EdgeKind::Calls,
                            });
                            for capability in required_capabilities {
                                edges.push(GraphEdge {
                                    from: operation_identity.clone(),
                                    to: capability_id(namespace, &function.name, capability),
                                    kind: EdgeKind::UsesCapability,
                                });
                            }
                            if let Some(callee_function) =
                                program.functions.iter().find(|candidate| {
                                    function_id(
                                        candidate.identity_namespace(&program.module),
                                        &candidate.name,
                                    ) == *callee
                                })
                            {
                                for effect in effects {
                                    let canonical = serde_json::to_string(
                                        &crate::canonical::canonical_effect(effect),
                                    )
                                    .expect("call effect");
                                    edges.push(GraphEdge {
                                        from: operation_identity.clone(),
                                        to: effect_id(
                                            callee_function.identity_namespace(&program.module),
                                            &callee_function.name,
                                            &canonical,
                                            callee_function
                                                .effects
                                                .iter()
                                                .position(|declared| declared == effect)
                                                .unwrap_or(0),
                                        ),
                                        kind: EdgeKind::PerformsEffect,
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                    for contract in &operation.contracts {
                        edges.push(GraphEdge {
                            from: operation_identity.clone(),
                            to: contract_id(namespace, &function.name, contract),
                            kind: EdgeKind::ReferencesContract,
                        });
                    }
                    if let Some(machine_intent) = &operation.machine_intent {
                        for requirement in &machine_intent.requirements {
                            edges.push(GraphEdge {
                                from: operation_identity.clone(),
                                to: crate::obligations::body_obligation_id(
                                    "machine-intent",
                                    &requirement.identity,
                                ),
                                kind: EdgeKind::RequiresObligation,
                            });
                        }
                        for obligation in &machine_intent.obligations {
                            edges.push(GraphEdge {
                                from: operation_identity.clone(),
                                to: obligation.identity.clone(),
                                kind: EdgeKind::RequiresObligation,
                            });
                        }
                    }
                    let generated_obligation = match &operation.kind {
                        crate::BodyOperationKind::Integer { .. }
                        | crate::BodyOperationKind::VectorBinary { .. }
                        | crate::BodyOperationKind::VectorReduce { .. } => {
                            Some(crate::obligations::body_obligation_id(
                                "integer-overflow",
                                &operation_identity,
                            ))
                        }
                        crate::BodyOperationKind::IntegerCompare { .. }
                        | crate::BodyOperationKind::BooleanOp { .. }
                        | crate::BodyOperationKind::FiniteConstruct { .. }
                        | crate::BodyOperationKind::FinitePayloadProject { .. }
                        | crate::BodyOperationKind::FiniteIsVariant { .. }
                        | crate::BodyOperationKind::RecordConstruct { .. }
                        | crate::BodyOperationKind::RecordProject { .. }
                        | crate::BodyOperationKind::ByteBitwise { .. }
                        | crate::BodyOperationKind::ByteShift { .. }
                        | crate::BodyOperationKind::ByteCompare { .. }
                        | crate::BodyOperationKind::Convert { .. }
                        | crate::BodyOperationKind::Select { .. }
                        | crate::BodyOperationKind::VectorConstruct { .. }
                        | crate::BodyOperationKind::VectorSplat { .. }
                        | crate::BodyOperationKind::VectorCompare { .. }
                        | crate::BodyOperationKind::MaskBinary { .. }
                        | crate::BodyOperationKind::MaskNot { .. }
                        | crate::BodyOperationKind::MaskReduce { .. }
                        | crate::BodyOperationKind::VectorExtract {
                            evidence: BoundsEvidence::StaticExact | BoundsEvidence::TraversalDomain,
                            ..
                        }
                        | crate::BodyOperationKind::VectorReplace {
                            evidence: BoundsEvidence::StaticExact | BoundsEvidence::TraversalDomain,
                            ..
                        }
                        | crate::BodyOperationKind::SequenceReplace {
                            evidence: BoundsEvidence::StaticExact,
                            ..
                        }
                        | crate::BodyOperationKind::SequenceReplace {
                            evidence: BoundsEvidence::TraversalDomain,
                            ..
                        }
                        | crate::BodyOperationKind::SequenceConstruct { .. }
                        | crate::BodyOperationKind::SequenceLength { .. } => None,
                        crate::BodyOperationKind::SequenceReplace {
                            evidence: BoundsEvidence::RuntimeChecked { .. },
                            ..
                        } => Some(crate::obligations::body_obligation_id(
                            "sequence-replace-bounds",
                            &operation_identity,
                        )),
                        crate::BodyOperationKind::VectorExtract {
                            evidence: BoundsEvidence::RuntimeChecked { .. },
                            ..
                        }
                        | crate::BodyOperationKind::VectorReplace {
                            evidence: BoundsEvidence::RuntimeChecked { .. },
                            ..
                        } => Some(crate::obligations::body_obligation_id(
                            "vector-lane-bounds",
                            &operation_identity,
                        )),
                        crate::BodyOperationKind::SequenceProject {
                            evidence: BoundsEvidence::RuntimeChecked { .. },
                            ..
                        } => Some(crate::obligations::body_obligation_id(
                            "sequence-index-bounds",
                            &operation_identity,
                        )),
                        crate::BodyOperationKind::SequenceProject { .. } => None,
                        crate::BodyOperationKind::ViewConstruct { .. } => {
                            Some(crate::obligations::body_obligation_id(
                                "view-range-valid",
                                &operation_identity,
                            ))
                        }
                        crate::BodyOperationKind::Call { .. } => {
                            Some(crate::obligations::body_obligation_id(
                                "call-authority-closure",
                                &operation_identity,
                            ))
                        }
                        crate::BodyOperationKind::Effect { .. } => {
                            Some(crate::obligations::body_obligation_id(
                                "effect-authorized",
                                &operation_identity,
                            ))
                        }
                        crate::BodyOperationKind::RuntimeCheck { .. } => {
                            Some(crate::obligations::body_obligation_id(
                                "runtime-check",
                                &operation_identity,
                            ))
                        }
                        crate::BodyOperationKind::Constant { .. } => None,
                    };
                    if let Some(obligation) = generated_obligation {
                        edges.push(GraphEdge {
                            from: operation_identity.clone(),
                            to: obligation,
                            kind: EdgeKind::RequiresObligation,
                        });
                    }
                    if let crate::BodyOperationKind::RuntimeCheck {
                        fact, obligation, ..
                    } = &operation.kind
                    {
                        edges.push(GraphEdge {
                            from: operation_identity.clone(),
                            to: fact.identity.clone(),
                            kind: EdgeKind::EstablishesFact,
                        });
                        edges.push(GraphEdge {
                            from: operation_identity.clone(),
                            to: obligation.clone(),
                            kind: EdgeKind::RequiresObligation,
                        });
                    }
                }
                match &block.terminator {
                    crate::BodyTerminator::Branch {
                        target, arguments, ..
                    } => {
                        edges.push(GraphEdge {
                            from: block_identity.clone(),
                            to: crate::identity::block_id(namespace, &function.name, target),
                            kind: EdgeKind::TransitionsTo,
                        });
                        for argument in arguments {
                            if let Some(value) = values.get(argument) {
                                edges.push(GraphEdge {
                                    from: block_identity.clone(),
                                    to: value.clone(),
                                    kind: EdgeKind::ConsumesValue,
                                });
                            }
                        }
                    }
                    crate::BodyTerminator::ConditionalBranch {
                        then_target,
                        else_target,
                        then_arguments,
                        else_arguments,
                        ..
                    } => {
                        for target in [then_target, else_target] {
                            edges.push(GraphEdge {
                                from: block_identity.clone(),
                                to: crate::identity::block_id(namespace, &function.name, target),
                                kind: EdgeKind::TransitionsTo,
                            });
                        }
                        for argument in then_arguments.iter().chain(else_arguments) {
                            if let Some(value) = values.get(argument) {
                                edges.push(GraphEdge {
                                    from: block_identity.clone(),
                                    to: value.clone(),
                                    kind: EdgeKind::ConsumesValue,
                                });
                            }
                        }
                    }
                    crate::BodyTerminator::Return { .. }
                    | crate::BodyTerminator::Failure { .. } => {}
                }
            }
            for iteration in &body.bounded_iterations {
                let iteration_identity =
                    crate::identity::iteration_id(namespace, &function.name, &iteration.id);
                edges.push(GraphEdge {
                    from: body_identity.clone(),
                    to: iteration_identity.clone(),
                    kind: EdgeKind::ContainsIteration,
                });
                let mut region_blocks = vec![
                    iteration.preheader.clone(),
                    iteration.header.clone(),
                    iteration.body_entry.clone(),
                    iteration.backedge.clone(),
                    iteration.exit.clone(),
                ];
                region_blocks.extend(iteration.body_blocks.clone());
                region_blocks.sort();
                region_blocks.dedup();
                for block in region_blocks {
                    edges.push(GraphEdge {
                        from: iteration_identity.clone(),
                        to: crate::identity::block_id(namespace, &function.name, &block),
                        kind: EdgeKind::IterationContainsBlock,
                    });
                }
                for carried in [
                    &iteration.initial_value,
                    &iteration.header_state,
                    &iteration.exit_state,
                ] {
                    if let Some(value) = values.get(carried) {
                        edges.push(GraphEdge {
                            from: iteration_identity.clone(),
                            to: value.clone(),
                            kind: EdgeKind::CarriesValue,
                        });
                    }
                }
                for callee in &iteration.callees {
                    edges.push(GraphEdge {
                        from: iteration_identity.clone(),
                        to: callee.clone(),
                        kind: EdgeKind::Calls,
                    });
                }
                for capability in &iteration.required_capabilities {
                    edges.push(GraphEdge {
                        from: iteration_identity.clone(),
                        to: capability_id(namespace, &function.name, capability),
                        kind: EdgeKind::UsesCapability,
                    });
                }
                for obligation_kind in [
                    "iteration-bound-valid",
                    "iteration-resource-ceiling",
                    "iteration-exact-resource-cost",
                    "iteration-authority-closure",
                    "iteration-state-preservation",
                    "iteration-completion-modes",
                ] {
                    edges.push(GraphEdge {
                        from: iteration_identity.clone(),
                        to: crate::obligations::body_obligation_id(
                            obligation_kind,
                            &iteration_identity,
                        ),
                        kind: EdgeKind::RequiresObligation,
                    });
                }
            }
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
