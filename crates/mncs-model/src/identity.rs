use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

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
    Function,
    Contract,
    Assumption,
    Effect,
    Capability,
    Evidence,
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

pub(crate) fn function_id(module: &str, function: &str) -> SemanticId {
    make_id(IdentityKind::Function, &[module, function])
}

pub(crate) fn contract_id(module: &str, function: &str, contract: &str) -> SemanticId {
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
        IdentityKind::Function => "function",
        IdentityKind::Contract => "contract",
        IdentityKind::Assumption => "assumption",
        IdentityKind::Effect => "effect",
        IdentityKind::Capability => "capability",
        IdentityKind::Evidence => "evidence",
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
    use crate::validation::tests::valid_program;

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
        after.functions[0].contracts[0].expression = "amount >= 0".to_owned();
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
