use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::body::BoundsEvidence;
use crate::canonical::{
    canonical_assumption, canonical_contract, canonical_effect, canonical_evidence,
    canonical_function, canonical_json_value, sha256_hex,
};
use crate::Program;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SemanticId(pub String);

impl SemanticId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SemanticId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityKind {
    Program,
    Module,
    FiniteType,
    FiniteVariant,
    RecordType,
    Function,
    Contract,
    Assumption,
    Effect,
    Capability,
    Evidence,
    Body,
    Iteration,
    Block,
    Operation,
    Value,
    RuntimeCheck,
    MachineIntent,
    Fact,
    Requirement,
    Obligation,
    Realization,
    Transformation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityRecord {
    pub identity: SemanticId,
    pub kind: IdentityKind,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticIdentities {
    pub schema_version: String,
    pub objects: Vec<IdentityRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityChange {
    pub identity: SemanticId,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticDiff {
    pub added: Vec<IdentityRecord>,
    pub removed: Vec<IdentityRecord>,
    pub changed: Vec<IdentityChange>,
}

impl Program {
    pub fn semantic_identities(&self) -> SemanticIdentities {
        let mut objects = Vec::new();
        let program_id = program_id(&self.module);
        objects.push(IdentityRecord {
            identity: program_id,
            kind: IdentityKind::Program,
            fingerprint: fingerprint_json(&self.canonical_form().expect("canonical form").json),
        });

        for finite_type in &self.finite_types {
            objects.push(IdentityRecord {
                identity: finite_type.identity.clone(),
                kind: IdentityKind::FiniteType,
                fingerprint: fingerprint_json(
                    &serde_json::to_string(finite_type).expect("finite type"),
                ),
            });
            for variant in &finite_type.variants {
                objects.push(IdentityRecord {
                    identity: variant.identity.clone(),
                    kind: IdentityKind::FiniteVariant,
                    fingerprint: fingerprint_json(
                        &serde_json::to_string(variant).expect("finite variant"),
                    ),
                });
            }
        }

        for assumption in &self.assumptions {
            objects.push(IdentityRecord {
                identity: assumption_id(&self.module, &assumption.id),
                kind: IdentityKind::Assumption,
                fingerprint: fingerprint_json(&canonical_assumption(assumption).to_string()),
            });
        }
        for function in &self.functions {
            let function_identity = function_id(&self.module, &function.name);
            objects.push(IdentityRecord {
                identity: function_identity.clone(),
                kind: IdentityKind::Function,
                fingerprint: fingerprint_json(&canonical_function(function).to_string()),
            });
            if let Some(body) = &function.body {
                let body_identity = body_id(&self.module, &function.name);
                objects.push(IdentityRecord {
                    identity: body_identity,
                    kind: IdentityKind::Body,
                    fingerprint: fingerprint_json(
                        &body.canonical_json().expect("canonical executable body"),
                    ),
                });
                for iteration in &body.bounded_iterations {
                    let iteration_identity =
                        iteration_id(&self.module, &function.name, &iteration.id);
                    objects.push(IdentityRecord {
                        identity: iteration_identity.clone(),
                        kind: IdentityKind::Iteration,
                        fingerprint: fingerprint_json(
                            &serde_json::to_string(iteration).expect("bounded iteration"),
                        ),
                    });
                    for obligation_kind in [
                        "iteration-bound-valid",
                        "iteration-resource-ceiling",
                        "iteration-exact-resource-cost",
                        "iteration-authority-closure",
                        "iteration-state-preservation",
                        "iteration-completion-modes",
                    ] {
                        objects.push(IdentityRecord {
                            identity: crate::obligations::body_obligation_id(
                                obligation_kind,
                                &iteration_identity,
                            ),
                            kind: IdentityKind::Obligation,
                            fingerprint: fingerprint_json(&format!(
                                "{obligation_kind}:{}",
                                iteration_identity.0
                            )),
                        });
                    }
                }
                for parameter in &body.parameters {
                    objects.push(IdentityRecord {
                        identity: parameter_id(&self.module, &function.name, &parameter.id),
                        kind: IdentityKind::Value,
                        fingerprint: fingerprint_json(
                            &serde_json::to_string(parameter).expect("body parameter"),
                        ),
                    });
                }
                for block in &body.blocks {
                    let block_identity = block_id(&self.module, &function.name, &block.id);
                    objects.push(IdentityRecord {
                        identity: block_identity,
                        kind: IdentityKind::Block,
                        fingerprint: fingerprint_json(
                            &serde_json::to_string(block).expect("body block"),
                        ),
                    });
                    for parameter in &block.parameters {
                        objects.push(IdentityRecord {
                            identity: value_id(
                                &self.module,
                                &function.name,
                                &block.id,
                                "parameter",
                                &parameter.id,
                            ),
                            kind: IdentityKind::Value,
                            fingerprint: fingerprint_json(
                                &serde_json::to_string(parameter).expect("block parameter"),
                            ),
                        });
                    }
                    for operation in &block.operations {
                        let operation_identity =
                            operation_id(&self.module, &function.name, &block.id, &operation.id);
                        objects.push(IdentityRecord {
                            identity: operation_identity.clone(),
                            kind: IdentityKind::Operation,
                            fingerprint: fingerprint_json(
                                &serde_json::to_string(operation).expect("body operation"),
                            ),
                        });
                        for result in &operation.results {
                            objects.push(IdentityRecord {
                                identity: value_id(
                                    &self.module,
                                    &function.name,
                                    &block.id,
                                    &operation.id,
                                    &result.id,
                                ),
                                kind: IdentityKind::Value,
                                fingerprint: fingerprint_json(
                                    &serde_json::to_string(result).expect("body value"),
                                ),
                            });
                        }
                        let generated_obligation = match &operation.kind {
                            crate::BodyOperationKind::Integer { .. } => {
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
                        if let Some(identity) = generated_obligation {
                            objects.push(IdentityRecord {
                                identity,
                                kind: IdentityKind::Obligation,
                                fingerprint: fingerprint_json(
                                    &serde_json::to_string(operation)
                                        .expect("generated obligation"),
                                ),
                            });
                        }
                        if let crate::BodyOperationKind::RuntimeCheck { obligation, .. } =
                            &operation.kind
                        {
                            objects.push(IdentityRecord {
                                identity: obligation.clone(),
                                kind: IdentityKind::Obligation,
                                fingerprint: fingerprint_json(obligation.as_str()),
                            });
                        }
                        if operation.machine_intent.is_some() {
                            objects.push(IdentityRecord {
                                identity: machine_intent_id(
                                    &self.module,
                                    &function.name,
                                    &block.id,
                                    &operation.id,
                                ),
                                kind: IdentityKind::MachineIntent,
                                fingerprint: fingerprint_json(
                                    &serde_json::to_string(&operation.machine_intent)
                                        .expect("machine intent"),
                                ),
                            });
                            if let Some(machine_intent) = &operation.machine_intent {
                                for fact in &machine_intent.facts {
                                    objects.push(IdentityRecord {
                                        identity: fact.identity.clone(),
                                        kind: IdentityKind::Fact,
                                        fingerprint: fingerprint_json(
                                            &serde_json::to_string(fact).expect("machine fact"),
                                        ),
                                    });
                                }
                                for requirement in &machine_intent.requirements {
                                    objects.push(IdentityRecord {
                                        identity: requirement.identity.clone(),
                                        kind: IdentityKind::Requirement,
                                        fingerprint: fingerprint_json(
                                            &serde_json::to_string(requirement)
                                                .expect("machine requirement"),
                                        ),
                                    });
                                }
                                for obligation in &machine_intent.obligations {
                                    objects.push(IdentityRecord {
                                        identity: obligation.identity.clone(),
                                        kind: IdentityKind::Obligation,
                                        fingerprint: fingerprint_json(
                                            &serde_json::to_string(obligation)
                                                .expect("machine obligation"),
                                        ),
                                    });
                                }
                            }
                        }
                        if matches!(
                            operation.kind,
                            crate::BodyOperationKind::RuntimeCheck { .. }
                        ) {
                            objects.push(IdentityRecord {
                                identity: runtime_check_id(
                                    &self.module,
                                    &function.name,
                                    &block.id,
                                    &operation.id,
                                ),
                                kind: IdentityKind::RuntimeCheck,
                                fingerprint: fingerprint_json(
                                    &serde_json::to_string(operation).expect("runtime check"),
                                ),
                            });
                        }
                    }
                }
            }
            for contract in &function.contracts {
                objects.push(IdentityRecord {
                    identity: contract_id(&self.module, &function.name, &contract.id),
                    kind: IdentityKind::Contract,
                    fingerprint: fingerprint_json(&canonical_contract(contract).to_string()),
                });
            }
            for capability in &function.capabilities {
                objects.push(IdentityRecord {
                    identity: capability_id(&self.module, &function.name, capability),
                    kind: IdentityKind::Capability,
                    fingerprint: fingerprint_json(capability),
                });
            }
            let mut effect_occurrences = BTreeMap::<String, usize>::new();
            for effect in &function.effects {
                let key =
                    canonical_json_value(&canonical_effect(effect)).expect("canonical effect");
                let occurrence = effect_occurrences.entry(key.clone()).or_default();
                objects.push(IdentityRecord {
                    identity: effect_id(&self.module, &function.name, &key, *occurrence),
                    kind: IdentityKind::Effect,
                    fingerprint: fingerprint_json(&key),
                });
                *occurrence += 1;
            }
            let mut evidence_occurrences = BTreeMap::<String, usize>::new();
            for evidence in &function.evidence {
                let key = canonical_json_value(&canonical_evidence(evidence))
                    .expect("canonical evidence");
                let occurrence = evidence_occurrences.entry(key.clone()).or_default();
                objects.push(IdentityRecord {
                    identity: evidence_id(&self.module, &function.name, &key, *occurrence),
                    kind: IdentityKind::Evidence,
                    fingerprint: fingerprint_json(&key),
                });
                *occurrence += 1;
            }
        }
        objects.sort_by(|left, right| left.identity.cmp(&right.identity));
        SemanticIdentities {
            schema_version: "0.2".to_owned(),
            objects,
        }
    }

    pub fn semantic_diff(&self, after: &Program) -> SemanticDiff {
        diff_identities(&self.semantic_identities(), &after.semantic_identities())
    }
}

impl SemanticIdentities {
    pub fn record(&self, identity: &SemanticId) -> Option<&IdentityRecord> {
        self.objects
            .iter()
            .find(|record| &record.identity == identity)
    }

    pub fn fingerprint(&self, identity: &SemanticId) -> Option<&str> {
        self.record(identity)
            .map(|record| record.fingerprint.as_str())
    }
}

pub(crate) fn program_id(module: &str) -> SemanticId {
    make_id(IdentityKind::Program, &[module])
}

/// A module namespace identity. Modules are semantic namespaces, not
/// compatibility evidence; this identity names the namespace itself so
/// programs can declare which modules they bind against.
pub fn module_id(name: &str) -> SemanticId {
    make_id(IdentityKind::Module, &[name])
}

pub fn finite_type_id(module: &str, name: &str) -> SemanticId {
    make_id(IdentityKind::FiniteType, &[module, name])
}

/// Logical record-type identity.  Field names and types participate in
/// canonical (sorted) order so declaration order cannot change the logical
/// type; no physical layout information participates.
pub fn record_type_id(module: &str, name: &str, fields: &[(&str, &str)]) -> SemanticId {
    let mut canonical: Vec<(&str, &str)> = fields.to_vec();
    canonical.sort();
    make_id(
        IdentityKind::RecordType,
        &[module, name, &{
            let mut joined = String::new();
            for (field_name, field_type) in &canonical {
                joined.push_str(field_name);
                joined.push(':');
                joined.push_str(field_type);
                joined.push(';');
            }
            joined
        }],
    )
}

pub fn finite_variant_id(module: &str, type_name: &str, variant: &str) -> SemanticId {
    make_id(IdentityKind::FiniteVariant, &[module, type_name, variant])
}

pub fn function_id(module: &str, function: &str) -> SemanticId {
    make_id(IdentityKind::Function, &[module, function])
}

/// Public contract identity: `mncs:0.2:contract:<module>::<function>::<id>`.
/// Generally useful for tools that must reference contract clauses without
/// re-deriving identity rules.
pub fn contract_id(module: &str, function: &str, contract: &str) -> SemanticId {
    make_id(IdentityKind::Contract, &[module, function, contract])
}

pub(crate) fn assumption_id(module: &str, assumption: &str) -> SemanticId {
    make_id(IdentityKind::Assumption, &[module, assumption])
}

pub(crate) fn capability_id(module: &str, function: &str, capability: &str) -> SemanticId {
    make_id(IdentityKind::Capability, &[module, function, capability])
}

pub(crate) fn effect_id(
    module: &str,
    function: &str,
    canonical: &str,
    occurrence: usize,
) -> SemanticId {
    make_id(
        IdentityKind::Effect,
        &[
            module,
            function,
            &format!("{}:{occurrence}", sha256_hex(canonical.as_bytes())),
        ],
    )
}

pub(crate) fn evidence_id(
    module: &str,
    function: &str,
    canonical: &str,
    occurrence: usize,
) -> SemanticId {
    make_id(
        IdentityKind::Evidence,
        &[
            module,
            function,
            &format!("{}:{occurrence}", sha256_hex(canonical.as_bytes())),
        ],
    )
}

pub(crate) fn diff_identities(
    before: &SemanticIdentities,
    after: &SemanticIdentities,
) -> SemanticDiff {
    let before_map: BTreeMap<_, _> = before
        .objects
        .iter()
        .map(|record| (&record.identity, record))
        .collect();
    let after_map: BTreeMap<_, _> = after
        .objects
        .iter()
        .map(|record| (&record.identity, record))
        .collect();
    let all_ids: BTreeSet<_> = before_map.keys().chain(after_map.keys()).collect();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    for identity in all_ids {
        match (before_map.get(identity), after_map.get(identity)) {
            (None, Some(record)) => added.push((*record).clone()),
            (Some(record), None) => removed.push((*record).clone()),
            (Some(left), Some(right)) if left.fingerprint != right.fingerprint => {
                changed.push(IdentityChange {
                    identity: (*identity).clone(),
                    before: left.fingerprint.clone(),
                    after: right.fingerprint.clone(),
                });
            }
            _ => {}
        }
    }
    SemanticDiff {
        added,
        removed,
        changed,
    }
}

fn make_id(kind: IdentityKind, components: &[&str]) -> SemanticId {
    let prefix = match kind {
        IdentityKind::Program => "program",
        IdentityKind::Module => "module",
        IdentityKind::FiniteType => "finite-type",
        IdentityKind::FiniteVariant => "finite-variant",
        IdentityKind::RecordType => "record-type",
        IdentityKind::Function => "function",
        IdentityKind::Contract => "contract",
        IdentityKind::Assumption => "assumption",
        IdentityKind::Effect => "effect",
        IdentityKind::Capability => "capability",
        IdentityKind::Evidence => "evidence",
        IdentityKind::Body => "body",
        IdentityKind::Iteration => "iteration",
        IdentityKind::Block => "block",
        IdentityKind::Operation => "operation",
        IdentityKind::Value => "value",
        IdentityKind::RuntimeCheck => "runtime-check",
        IdentityKind::MachineIntent => "machine-intent",
        IdentityKind::Fact => "fact",
        IdentityKind::Requirement => "requirement",
        IdentityKind::Obligation => "obligation",
        IdentityKind::Realization => "realization",
        IdentityKind::Transformation => "transformation",
    };
    SemanticId(format!(
        "mncs:0.2:{prefix}:{}",
        components
            .iter()
            .map(|value| encode_component(value))
            .collect::<Vec<_>>()
            .join("::")
    ))
}

pub(crate) fn body_id(module: &str, function: &str) -> SemanticId {
    make_id(IdentityKind::Body, &[module, function])
}

pub(crate) fn iteration_id(module: &str, function: &str, iteration: &str) -> SemanticId {
    make_id(IdentityKind::Iteration, &[module, function, iteration])
}

pub(crate) fn block_id(module: &str, function: &str, block: &str) -> SemanticId {
    make_id(IdentityKind::Block, &[module, function, block])
}

pub(crate) fn operation_id(
    module: &str,
    function: &str,
    block: &str,
    operation: &str,
) -> SemanticId {
    make_id(
        IdentityKind::Operation,
        &[module, function, block, operation],
    )
}

pub(crate) fn parameter_id(module: &str, function: &str, value: &str) -> SemanticId {
    make_id(IdentityKind::Value, &[module, function, "parameter", value])
}

pub(crate) fn value_id(
    module: &str,
    function: &str,
    block: &str,
    operation: &str,
    value: &str,
) -> SemanticId {
    make_id(
        IdentityKind::Value,
        &[module, function, block, operation, value],
    )
}

pub(crate) fn runtime_check_id(
    module: &str,
    function: &str,
    block: &str,
    operation: &str,
) -> SemanticId {
    make_id(
        IdentityKind::RuntimeCheck,
        &[module, function, block, operation],
    )
}

pub(crate) fn machine_intent_id(
    module: &str,
    function: &str,
    block: &str,
    operation: &str,
) -> SemanticId {
    make_id(
        IdentityKind::MachineIntent,
        &[module, function, block, operation],
    )
}

/// Inverse of [`encode_component`]. Returns `None` when the text contains a
/// malformed escape.
pub fn decode_component(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = value.get(index + 1..index + 3)?;
            let byte = u8::from_str_radix(hex, 16).ok()?;
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

impl SemanticId {
    /// The module namespace named by a module identity, or `None` for any
    /// other identity kind.
    pub fn module_name(&self) -> Option<String> {
        let prefix = format!("{MODULE_ID_PREFIX}:");
        let rest = self.0.strip_prefix(&prefix)?;
        decode_component(rest)
    }
}

const MODULE_ID_PREFIX: &str = "mncs:0.2:module";

fn encode_component(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' | b'.' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn fingerprint_json(value: &str) -> String {
    sha256_hex(value.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::{record_type_id, IdentityKind};
    use crate::validation::tests::valid_program;

    #[test]
    fn record_type_identity_is_canonical_in_field_order() {
        let sorted = record_type_id("m", "R", &[("a", "i32"), ("b", "i32")]);
        let permuted = record_type_id("m", "R", &[("b", "i32"), ("a", "i32")]);
        assert_eq!(sorted, permuted);
        let different_field_type = record_type_id("m", "R", &[("a", "i64"), ("b", "i32")]);
        assert_ne!(sorted, different_field_type);
        let different_name = record_type_id("m", "S", &[("a", "i32"), ("b", "i32")]);
        assert_ne!(sorted, different_name);
    }

    #[test]
    fn record_type_ids_use_the_record_kind_namespace() {
        let identity = record_type_id("m", "R", &[("a", "i32")]);
        assert!(identity.0.contains(":record-type:"));
        assert!(!identity.0.contains(":finite-type:"));
        let _ = IdentityKind::Program;
    }

    #[test]
    fn identity_is_deterministic_and_namespaced() {
        let program = valid_program();
        let left = program.semantic_identities();
        let right = program.semantic_identities();
        assert_eq!(left, right);
        assert!(left
            .objects
            .iter()
            .any(|record| record.identity.0.starts_with("mncs:0.2:function:")));
    }

    #[test]
    fn local_contract_change_changes_contract_and_function_fingerprints() {
        let before = valid_program();
        let mut after = before.clone();
        "amount >= 0".clone_into(&mut after.functions[0].contracts[0].expression);
        let diff = before.semantic_diff(&after);
        assert!(diff
            .changed
            .iter()
            .any(|change| change.identity.0.contains(":contract:")));
        assert!(diff
            .changed
            .iter()
            .any(|change| change.identity.0.contains(":function:")));
    }
}
