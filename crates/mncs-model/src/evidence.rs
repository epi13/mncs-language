use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::canonical::{canonical_evidence, canonical_json_value, sha256_hex};
use crate::graph::GraphError;
use crate::identity::{assumption_id, contract_id, evidence_id, function_id, SemanticId};
use crate::{EvidenceClaim, EvidenceStatus, Function, Program, SemanticIdentities};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFreshness {
    Current,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Current,
    Stale,
    Unknown,
}

/// An immutable, identity-bound answer to one validation question.
///
/// A receipt is reusable only when its exact subject, validator, scope, and
/// dependency fingerprints still match. It records bounded evidence; it does
/// not turn an empirical observation into a proof or promote UNKNOWN.
pub const EVIDENCE_RECEIPT_SCHEMA_VERSION: &str = "0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceReceiptOutcome {
    Pass,
    Fail,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceReceipt {
    pub schema_version: String,
    pub identity: SemanticId,
    pub fact: String,
    pub subject: SemanticId,
    pub validator: SemanticId,
    pub scope: BTreeMap<String, String>,
    pub dependencies: BTreeMap<SemanticId, String>,
    pub assumptions: Vec<SemanticId>,
    pub outcome: EvidenceReceiptOutcome,
    pub invalidation_triggers: Vec<String>,
}

impl EvidenceReceipt {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        fact: impl Into<String>,
        subject: SemanticId,
        validator: SemanticId,
        mut scope: BTreeMap<String, String>,
        dependencies: BTreeMap<SemanticId, String>,
        mut assumptions: Vec<SemanticId>,
        outcome: EvidenceReceiptOutcome,
        mut invalidation_triggers: Vec<String>,
    ) -> Self {
        scope.retain(|key, value| !key.trim().is_empty() && !value.trim().is_empty());
        assumptions.sort();
        assumptions.dedup();
        invalidation_triggers.sort();
        invalidation_triggers.dedup();
        let mut receipt = Self {
            schema_version: EVIDENCE_RECEIPT_SCHEMA_VERSION.to_owned(),
            identity: SemanticId(String::new()),
            fact: fact.into(),
            subject,
            validator,
            scope,
            dependencies,
            assumptions,
            outcome,
            invalidation_triggers,
        };
        receipt.seal();
        receipt
    }

    pub fn seal(&mut self) {
        self.identity = SemanticId(String::new());
        let canonical = canonical_json_value(self).expect("evidence receipt is serializable");
        self.identity = SemanticId(format!(
            "mncs:evidence:receipt:{}",
            sha256_hex(canonical.as_bytes())
        ));
    }

    pub fn identity_is_valid(&self) -> bool {
        let mut material = self.clone();
        material.identity = SemanticId(String::new());
        self.schema_version == EVIDENCE_RECEIPT_SCHEMA_VERSION
            && !self.fact.trim().is_empty()
            && !self.subject.0.trim().is_empty()
            && !self.validator.0.trim().is_empty()
            && self.identity == {
                let canonical =
                    canonical_json_value(&material).expect("evidence receipt is serializable");
                SemanticId(format!(
                    "mncs:evidence:receipt:{}",
                    sha256_hex(canonical.as_bytes())
                ))
            }
    }

    pub fn reusable_if(
        &self,
        subject: &SemanticId,
        validator: &SemanticId,
        scope: &BTreeMap<String, String>,
        dependencies: &BTreeMap<SemanticId, String>,
    ) -> bool {
        self.identity_is_valid()
            && &self.subject == subject
            && &self.validator == validator
            && &self.scope == scope
            && &self.dependencies == dependencies
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub identity: SemanticId,
    pub subject: SemanticId,
    pub property: SemanticId,
    pub verifier: String,
    pub status: EvidenceStatus,
    pub artifact: Option<String>,
    pub assumptions: Vec<SemanticId>,
    pub dependencies: Vec<SemanticId>,
    pub dependency_fingerprints: BTreeMap<SemanticId, String>,
    pub semantic_fingerprint: String,
    pub freshness: EvidenceFreshness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceManifest {
    pub schema_version: String,
    pub evidence: Vec<EvidenceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceStatusReport {
    pub evidence: SemanticId,
    pub state: EvidenceState,
    pub invalidated_by: Vec<SemanticId>,
}

impl Program {
    pub fn evidence_manifest(&self) -> Result<EvidenceManifest, GraphError> {
        if !self.validate().valid {
            return Err(GraphError::InvalidProgram(self.validate()));
        }
        Ok(manifest_for_program(self, EvidenceFreshness::Current))
    }
}

impl EvidenceManifest {
    pub fn assess_against(
        &self,
        program: &Program,
    ) -> Result<Vec<EvidenceStatusReport>, GraphError> {
        let current = program.evidence_manifest()?;
        let identities = program.semantic_identities();
        Ok(self
            .evidence
            .iter()
            .map(|record| {
                let mut invalidated_by = Vec::new();
                for dependency in &record.dependencies {
                    let stored = record.dependency_fingerprints.get(dependency);
                    let current_fingerprint = identities.fingerprint(dependency);
                    if current_fingerprint.is_none()
                        || stored.map(String::as_str) != current_fingerprint
                    {
                        invalidated_by.push(dependency.clone());
                    }
                }
                let current_record = current
                    .evidence
                    .iter()
                    .find(|candidate| candidate.identity == record.identity);
                if current_record.map(|candidate| candidate.semantic_fingerprint.as_str())
                    != Some(record.semantic_fingerprint.as_str())
                {
                    invalidated_by.push(record.subject.clone());
                }
                invalidated_by.sort();
                invalidated_by.dedup();
                EvidenceStatusReport {
                    evidence: record.identity.clone(),
                    state: if invalidated_by.is_empty() {
                        EvidenceState::Current
                    } else {
                        EvidenceState::Stale
                    },
                    invalidated_by,
                }
            })
            .collect())
    }
}

fn manifest_for_program(program: &Program, freshness: EvidenceFreshness) -> EvidenceManifest {
    let identities = program.semantic_identities();
    let mut evidence = Vec::new();
    for function in &program.functions {
        for claim in &function.evidence {
            let occurrence = occurrence_for_claim(function, claim);
            evidence.push(record_for_claim(
                program,
                &identities,
                function,
                claim,
                occurrence,
                freshness,
            ));
        }
    }
    evidence.sort_by(|left, right| left.identity.cmp(&right.identity));
    EvidenceManifest {
        schema_version: "0.2".to_owned(),
        evidence,
    }
}

fn occurrence_for_claim(function: &Function, claim: &EvidenceClaim) -> usize {
    let target = serde_json::to_string(&canonical_evidence(claim)).expect("canonical evidence");
    function
        .evidence
        .iter()
        .take_while(|candidate| !std::ptr::eq(*candidate, claim))
        .filter(|candidate| {
            serde_json::to_string(&canonical_evidence(candidate)).expect("canonical evidence")
                == target
        })
        .count()
}

fn record_for_claim(
    program: &Program,
    identities: &SemanticIdentities,
    function: &Function,
    claim: &EvidenceClaim,
    occurrence: usize,
    freshness: EvidenceFreshness,
) -> EvidenceRecord {
    let canonical = serde_json::to_string(&canonical_evidence(claim)).expect("canonical evidence");
    let namespace = function.identity_namespace(&program.module);
    let identity = evidence_id(namespace, &function.name, &canonical, occurrence);
    let subject = function_id(namespace, &function.name);
    let property = contract_id(namespace, &function.name, &claim.property);
    let assumptions = function
        .assumptions
        .iter()
        .map(|assumption| assumption_id(namespace, assumption))
        .collect::<Vec<_>>();
    let mut dependencies = vec![subject.clone(), property.clone()];
    dependencies.extend(assumptions.iter().cloned());
    dependencies.sort();
    dependencies.dedup();
    let dependency_fingerprints = dependencies
        .iter()
        .map(|dependency| {
            (
                dependency.clone(),
                identities
                    .fingerprint(dependency)
                    .unwrap_or("missing")
                    .to_owned(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let dependency_material = dependencies
        .iter()
        .map(|dependency| {
            format!(
                "{}={}",
                dependency,
                identities.fingerprint(dependency).unwrap_or("missing")
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    EvidenceRecord {
        identity,
        subject,
        property,
        verifier: claim.verifier.clone(),
        status: claim.status.clone(),
        artifact: claim.artifact.clone(),
        assumptions,
        dependencies,
        dependency_fingerprints,
        semantic_fingerprint: sha256_hex(dependency_material.as_bytes()),
        freshness,
    }
}

#[cfg(test)]
mod tests {
    use crate::validation::tests::valid_program;
    use crate::{
        EvidenceFreshness, EvidenceReceipt, EvidenceReceiptOutcome, EvidenceState, SemanticId,
    };
    use std::collections::BTreeMap;

    #[test]
    fn manifest_binds_subject_property_and_assumption_dependencies() {
        let manifest = valid_program().evidence_manifest().expect("manifest");
        let record = &manifest.evidence[0];
        assert_eq!(record.freshness, EvidenceFreshness::Current);
        assert!(record.subject.0.contains(":function:"));
        assert!(record.property.0.contains(":contract:"));
        assert_eq!(record.assumptions.len(), 1);
    }

    #[test]
    fn contract_change_makes_saved_evidence_stale() {
        let before = valid_program();
        let saved = before.evidence_manifest().expect("manifest");
        let mut after = before;
        "amount >= 0".clone_into(&mut after.functions[0].contracts[0].expression);
        let report = saved.assess_against(&after).expect("assessment");
        assert_eq!(report[0].state, EvidenceState::Stale);
        assert!(!report[0].invalidated_by.is_empty());
    }

    #[test]
    fn receipt_reuse_requires_exact_subject_validator_scope_and_dependencies() {
        let subject = SemanticId("mncs:test:subject".to_owned());
        let validator = SemanticId("mncs:test:validator:0.1".to_owned());
        let scope = BTreeMap::from([(String::from("contract"), String::from("0.1"))]);
        let dependencies = BTreeMap::from([(
            SemanticId("mncs:test:dependency".to_owned()),
            String::from("abc"),
        )]);
        let receipt = EvidenceReceipt::new(
            "bounded test fact",
            subject.clone(),
            validator.clone(),
            scope.clone(),
            dependencies.clone(),
            Vec::new(),
            EvidenceReceiptOutcome::Pass,
            vec!["subject changes".to_owned()],
        );
        assert!(receipt.identity_is_valid());
        assert!(receipt.reusable_if(&subject, &validator, &scope, &dependencies));

        let changed_dependencies = BTreeMap::from([(
            SemanticId("mncs:test:dependency".to_owned()),
            String::from("changed"),
        )]);
        assert!(!receipt.reusable_if(&subject, &validator, &scope, &changed_dependencies));
    }

    #[test]
    fn receipt_identity_tampering_fails_closed() {
        let mut receipt = EvidenceReceipt::new(
            "bounded test fact",
            SemanticId("mncs:test:subject".to_owned()),
            SemanticId("mncs:test:validator:0.1".to_owned()),
            BTreeMap::new(),
            BTreeMap::new(),
            Vec::new(),
            EvidenceReceiptOutcome::Unknown,
            Vec::new(),
        );
        receipt.fact = "different fact".to_owned();
        assert!(!receipt.identity_is_valid());
    }
}
