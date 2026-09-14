use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::canonical::sha256_hex;
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
    ContainsTest,
    ContainsTestCase,
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

/// A bounded compiler-owned semantic neighborhood for change impact.
///
/// This is intentionally smaller than a graph dump: consumers receive the
/// changed roots, reverse dependents, and test identities reachable through
/// affected functions. `complete` means the bounded traversal completed for
/// the current graph; `limitations` states what this projection does not
/// claim, such as cross-repository edges or path-sensitive causality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticImpact {
    pub schema_version: String,
    pub graph_identity: String,
    pub roots: Vec<SemanticId>,
    pub nodes: Vec<ImpactNode>,
    pub edges: Vec<GraphEdge>,
    pub direct_dependents: Vec<SemanticId>,
    pub test_identities: Vec<SemanticId>,
    pub risk_flags: Vec<ImpactRisk>,
    pub complete: bool,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactNode {
    pub identity: SemanticId,
    pub kind: IdentityKind,
    pub distance: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactRisk {
    PublicContract,
    SharedType,
    EffectSemantics,
    AbiBoundary,
    HighConnectivity,
    UnknownRoot,
    Truncated,
}

pub const SEMANTIC_IMPACT_SCHEMA_VERSION: &str = "mncs.semantic-impact/1";

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

    /// Build a bounded reverse-dependency neighborhood for the supplied
    /// semantic roots. Reverse traversal follows graph edges from a target to
    /// operations/functions that consume it, which is the direction needed
    /// for verification selection. The method does not guess source paths,
    /// test names, or cross-repository relationships.
    pub fn impact_neighborhood(
        &self,
        roots: &[SemanticId],
        max_depth: usize,
        max_nodes: usize,
    ) -> SemanticImpact {
        let max_depth = max_depth.max(1);
        let max_nodes = max_nodes.max(1);
        let node_index: BTreeMap<SemanticId, &GraphNode> = self
            .nodes
            .iter()
            .map(|node| (node.identity.clone(), node))
            .collect();
        let mut roots = roots.to_vec();
        roots.sort();
        roots.dedup();
        let known_roots: BTreeSet<_> = roots
            .iter()
            .filter(|root| node_index.contains_key(*root))
            .cloned()
            .collect();
        let mut distances: BTreeMap<SemanticId, usize> = BTreeMap::new();
        let mut queue = std::collections::VecDeque::new();
        for root in &known_roots {
            distances.insert(root.clone(), 0);
            queue.push_back(root.clone());
        }
        let mut truncated = false;
        while let Some(current) = queue.pop_front() {
            let distance = distances[&current];
            if distance >= max_depth {
                continue;
            }
            for edge in self.edges.iter().filter(|edge| edge.to == current) {
                if distances.contains_key(&edge.from) {
                    continue;
                }
                if distances.len() >= max_nodes {
                    truncated = true;
                    break;
                }
                distances.insert(edge.from.clone(), distance + 1);
                queue.push_back(edge.from.clone());
            }
            if truncated {
                break;
            }
        }
        let affected: BTreeSet<_> = distances.keys().cloned().collect();
        let mut edges = self
            .edges
            .iter()
            .filter(|edge| affected.contains(&edge.from) && affected.contains(&edge.to))
            .cloned()
            .collect::<Vec<_>>();
        edges.sort_by(|left, right| {
            left.from
                .cmp(&right.from)
                .then(left.to.cmp(&right.to))
                .then(left.kind.cmp(&right.kind))
        });
        let nodes = distances
            .iter()
            .filter_map(|(identity, distance)| {
                node_index.get(identity).map(|node| ImpactNode {
                    identity: identity.clone(),
                    kind: node.kind,
                    distance: *distance,
                })
            })
            .collect::<Vec<_>>();
        let direct_dependents = self
            .edges
            .iter()
            .filter(|edge| known_roots.contains(&edge.to) && affected.contains(&edge.from))
            .map(|edge| edge.from.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let function_identities = nodes
            .iter()
            .filter(|node| node.kind == IdentityKind::Function)
            .map(|node| node.identity.clone())
            .chain(known_roots.iter().filter_map(|root| {
                node_index
                    .get(root)
                    .and_then(|node| (node.kind == IdentityKind::Function).then(|| root.clone()))
            }))
            .collect::<BTreeSet<_>>();
        let test_declarations = self
            .edges
            .iter()
            .filter(|edge| {
                function_identities.contains(&edge.from) && edge.kind == EdgeKind::ContainsTest
            })
            .map(|edge| edge.to.clone())
            .collect::<BTreeSet<_>>();
        let test_identities = self
            .edges
            .iter()
            .filter(|edge| {
                test_declarations.contains(&edge.from) && edge.kind == EdgeKind::ContainsTestCase
            })
            .map(|edge| edge.to.clone())
            .filter(|identity| {
                node_index
                    .get(identity)
                    .is_some_and(|node| node.kind == IdentityKind::TestCase)
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut risk_flags = BTreeSet::new();
        for root in &roots {
            match node_index.get(root).map(|node| node.kind) {
                Some(
                    IdentityKind::Contract
                    | IdentityKind::Function
                    | IdentityKind::FiniteType
                    | IdentityKind::RecordType
                    | IdentityKind::RecordField,
                ) => {
                    risk_flags.insert(ImpactRisk::PublicContract);
                }
                Some(IdentityKind::Effect | IdentityKind::Capability) => {
                    risk_flags.insert(ImpactRisk::EffectSemantics);
                }
                Some(IdentityKind::Requirement | IdentityKind::Obligation) => {
                    risk_flags.insert(ImpactRisk::AbiBoundary);
                }
                Some(_) => {}
                None => {
                    risk_flags.insert(ImpactRisk::UnknownRoot);
                }
            }
        }
        for root in &known_roots {
            let degree = self.edges.iter().filter(|edge| edge.to == *root).count();
            if degree > 8 {
                risk_flags.insert(ImpactRisk::HighConnectivity);
                break;
            }
        }
        if truncated {
            risk_flags.insert(ImpactRisk::Truncated);
        }
        if roots.iter().any(|root| !known_roots.contains(root)) {
            risk_flags.insert(ImpactRisk::UnknownRoot);
        }
        let mut limitations = vec![
            "neighborhood is compiler-exact only for the current semantic graph".to_owned(),
            "cross-repository edges and path-sensitive control flow are not represented".to_owned(),
        ];
        if max_depth < 2 {
            limitations.push("reverse traversal depth was intentionally bounded".to_owned());
        }
        if truncated {
            limitations.push("node budget truncated the reverse dependency traversal".to_owned());
        }
        if roots.iter().any(|root| !known_roots.contains(root)) {
            limitations.push("one or more requested roots are not current graph nodes".to_owned());
        }
        let graph_identity = self
            .canonical_json()
            .map(|json| sha256_hex(json.as_bytes()))
            .unwrap_or_else(|_| "unavailable".to_owned());
        let complete = !truncated && known_roots.len() == roots.len();
        SemanticImpact {
            schema_version: SEMANTIC_IMPACT_SCHEMA_VERSION.to_owned(),
            graph_identity,
            roots,
            nodes,
            edges,
            direct_dependents,
            test_identities,
            risk_flags: risk_flags.into_iter().collect(),
            complete,
            max_depth,
            max_nodes,
            limitations,
        }
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
        if function.is_test {
            let test_identity = crate::identity::test_declaration_id(namespace, &function.name);
            let test_fingerprint = identities
                .objects
                .iter()
                .find(|record| record.identity == test_identity)
                .map(|record| record.fingerprint.clone())
                .unwrap_or_default();
            let test_case_identity =
                crate::identity::test_case_id(namespace, &function.name, &test_fingerprint);
            edges.push(GraphEdge {
                from: function_identity.clone(),
                to: test_identity.clone(),
                kind: EdgeKind::ContainsTest,
            });
            edges.push(GraphEdge {
                from: test_identity,
                to: test_case_identity,
                kind: EdgeKind::ContainsTestCase,
            });
        }
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
                        crate::BodyOperationKind::Float { .. }
                        | crate::BodyOperationKind::FloatCompare { .. }
                        | crate::BodyOperationKind::FloatIntrinsic { .. } => {
                            Some(crate::obligations::body_obligation_id(
                                "float-finite",
                                &operation_identity,
                            ))
                        }
                        crate::BodyOperationKind::IntegerCompare { .. }
                        | crate::BodyOperationKind::FloatConstant { .. }
                        | crate::BodyOperationKind::BooleanOp { .. }
                        | crate::BodyOperationKind::BooleanCompare { .. }
                        | crate::BodyOperationKind::BooleanNot
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
                        | crate::BodyOperationKind::SequenceCopy {
                            evidence: BoundsEvidence::StaticExact,
                            ..
                        }
                        | crate::BodyOperationKind::SequenceConstruct { .. }
                        | crate::BodyOperationKind::SequenceLength { .. }
                        | crate::BodyOperationKind::BoundCheck { .. } => None,
                        crate::BodyOperationKind::SequenceReplace {
                            evidence:
                                BoundsEvidence::RuntimeChecked { .. } | BoundsEvidence::CheckedBound,
                            ..
                        } => Some(crate::obligations::body_obligation_id(
                            "sequence-replace-bounds",
                            &operation_identity,
                        )),
                        // Traversal-domain and checked-bound claims on span
                        // copies are never established by elaboration (two
                        // sequences share no single traversal domain; only
                        // projections discharge through checked indices);
                        // keep them attached to the unresolved requirement
                        // rather than silently discharging them.
                        crate::BodyOperationKind::SequenceCopy {
                            evidence:
                                BoundsEvidence::RuntimeChecked { .. }
                                | BoundsEvidence::TraversalDomain
                                | BoundsEvidence::CheckedBound,
                            ..
                        } => Some(crate::obligations::body_obligation_id(
                            "sequence-copy-bounds",
                            &operation_identity,
                        )),
                        crate::BodyOperationKind::VectorExtract {
                            evidence:
                                BoundsEvidence::RuntimeChecked { .. } | BoundsEvidence::CheckedBound,
                            ..
                        }
                        | crate::BodyOperationKind::VectorReplace {
                            evidence:
                                BoundsEvidence::RuntimeChecked { .. } | BoundsEvidence::CheckedBound,
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
                        crate::BodyOperationKind::ViewConstruct { .. }
                        | crate::BodyOperationKind::ViewNarrow { .. } => {
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
                        crate::BodyOperationKind::HostCall { .. } => {
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

    #[test]
    fn impact_neighborhood_is_bounded_and_exposes_test_cases() {
        let mut program = valid_program();
        let mut test = program.functions[0].clone();
        test.name = "transfer_test".to_owned();
        test.is_test = true;
        test.contracts.clear();
        test.effects.clear();
        test.capabilities.clear();
        test.assumptions.clear();
        test.evidence.clear();
        test.body = None;
        program.functions.push(test);
        let root = crate::function_id(&program.module, "transfer_test");
        let graph = program.semantic_graph().expect("graph");
        let impact = graph.impact_neighborhood(std::slice::from_ref(&root), 2, 8);
        assert_eq!(impact.schema_version, super::SEMANTIC_IMPACT_SCHEMA_VERSION);
        assert_eq!(impact.roots, vec![root]);
        assert!(!impact.test_identities.is_empty());
        assert!(impact
            .risk_flags
            .contains(&super::ImpactRisk::PublicContract));
        assert!(impact.nodes.len() <= 8);
        assert!(impact
            .limitations
            .iter()
            .any(|item| item.contains("cross-repository")));
    }

    #[test]
    fn requires_obligation_edges_resolve_to_generated_obligations() {
        // Every proof-graph obligation edge must name an obligation the
        // obligation pass actually generates for the same operation. The
        // executable body fixture below carries real operations (constant
        // and checked integer arithmetic), so a match arm that accidentally
        // widens (for example by splitting a shared `=> None` alternation)
        // shows up here as a dangling edge before it can mislead any
        // consumer of the graph.
        use std::collections::BTreeSet;
        let mut program = crate::body::tests::executable_program();
        // Total operations (comparison, selection, conversion) join the
        // fixture so the invariant also covers the shared discharged arm:
        // none of them may gain an obligation edge at all.
        let i32_ty = crate::BodyType::Integer(crate::IntegerType {
            bits: 32,
            signed: true,
        });
        let bool_ty = crate::BodyType::Bool;
        let u64_ty = crate::BodyType::Integer(crate::IntegerType {
            bits: 64,
            signed: false,
        });
        let total_op = |id: &str,
                        kind: crate::BodyOperationKind,
                        operands: Vec<String>,
                        ty: crate::BodyType| {
            crate::BodyOperation {
                id: id.to_owned(),
                kind,
                operands,
                results: vec![crate::BodyValue {
                    id: id.to_owned(),
                    ty,
                }],
                contracts: Vec::new(),
                assumptions: Vec::new(),
                machine_intent: None,
                lowering: None,
                portability: None,
            }
        };
        let body = program.functions[0].body.as_mut().expect("fixture body");
        let block = body
            .blocks
            .iter_mut()
            .find(|block| block.id == "entry")
            .expect("entry block");
        block.operations.push(total_op(
            "cmp",
            crate::BodyOperationKind::IntegerCompare {
                predicate: "eq".to_owned(),
                operand_type: crate::IntegerType {
                    bits: 32,
                    signed: true,
                },
            },
            vec!["a".to_owned(), "one".to_owned()],
            bool_ty.clone(),
        ));
        block.operations.push(total_op(
            "sel",
            crate::BodyOperationKind::Select {
                operand_type: Box::new(i32_ty.clone()),
            },
            vec!["cmp".to_owned(), "a".to_owned(), "one".to_owned()],
            i32_ty.clone(),
        ));
        block.operations.push(total_op(
            "conv",
            crate::BodyOperationKind::Convert {
                from: i32_ty.clone(),
                to: u64_ty.clone(),
            },
            vec!["sel".to_owned()],
            u64_ty.clone(),
        ));
        let graph = program.semantic_graph().expect("graph");
        let generated: BTreeSet<crate::SemanticId> = program
            .generate_obligations()
            .obligations
            .iter()
            .map(|obligation| obligation.identity.clone())
            .collect();
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.kind == EdgeKind::RequiresObligation),
            "fixture must carry at least one obligation edge"
        );
        for edge in graph
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::RequiresObligation)
        {
            assert!(
                generated.contains(&edge.to),
                "dangling obligation edge {} -> {}",
                edge.from,
                edge.to
            );
        }
    }
}
