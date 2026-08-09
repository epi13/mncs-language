use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::canonical::{canonical_evidence, sha256_hex};
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
    let identity = evidence_id(&program.module, &function.name, &canonical, occurrence);
    let subject = function_id(&program.module, &function.name);
    let property = contract_id(&program.module, &function.name, &claim.property);
    let assumptions = function
        .assumptions
        .iter()
        .map(|assumption| assumption_id(&program.module, assumption))
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
    use crate::{EvidenceFreshness, EvidenceState};

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
        after.functions[0].contracts[0].expression = "amount >= 0".to_owned();
        let report = saved.assess_against(&after).expect("assessment");
        assert_eq!(report[0].state, EvidenceState::Stale);
        assert!(!report[0].invalidated_by.is_empty());
    }
}
