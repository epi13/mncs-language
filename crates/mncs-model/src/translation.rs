//! Translation-validation contracts.
//!
//! A validator may certify a claimed relation between exact artifacts. The
//! transformation generator is never the authority that records PASS.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::canonical::{canonical_json_value, sha256_hex};
use crate::compiler::{CompilerArtifactRef, RelationClaim};
use crate::{EvidenceFreshness, ExecutionValue, SemanticId, VerifierIdentity};

pub const TRANSLATION_VALIDATION_SCHEMA_VERSION: &str = "0.1";
pub const TRANSLATION_VALIDATION_CONTRACT_ID: &str =
    "mncs:language:translation-validation-result:0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TranslationJudgement {
    Pass,
    Fail,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationCounterexample {
    pub case_id: String,
    pub reason: String,
    pub arguments: Vec<ExecutionValue>,
    pub expected: Option<Vec<ExecutionValue>>,
    pub observed: Option<Vec<ExecutionValue>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationValidationResult {
    pub schema_version: String,
    pub contract_id: String,
    pub identity: SemanticId,
    pub validator: VerifierIdentity,
    pub input: CompilerArtifactRef,
    pub output: CompilerArtifactRef,
    pub claimed_relation: RelationClaim,
    pub transformation_kind: String,
    pub judgement: TranslationJudgement,
    pub assumptions: Vec<String>,
    pub evidence_dependencies: Vec<SemanticId>,
    pub dependency_fingerprints: BTreeMap<SemanticId, String>,
    pub counterexample: Option<TranslationCounterexample>,
    pub limitations: Vec<String>,
    pub residual_trust: String,
    pub freshness: EvidenceFreshness,
}

impl TranslationValidationResult {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        validator: VerifierIdentity,
        input: CompilerArtifactRef,
        output: CompilerArtifactRef,
        claimed_relation: RelationClaim,
        transformation_kind: impl Into<String>,
        judgement: TranslationJudgement,
        mut assumptions: Vec<String>,
        mut evidence_dependencies: Vec<SemanticId>,
        dependency_fingerprints: BTreeMap<SemanticId, String>,
        counterexample: Option<TranslationCounterexample>,
        mut limitations: Vec<String>,
        residual_trust: impl Into<String>,
        freshness: EvidenceFreshness,
    ) -> Self {
        assumptions.sort();
        assumptions.dedup();
        evidence_dependencies.sort();
        evidence_dependencies.dedup();
        limitations.sort();
        limitations.dedup();
        let mut result = Self {
            schema_version: TRANSLATION_VALIDATION_SCHEMA_VERSION.to_owned(),
            contract_id: TRANSLATION_VALIDATION_CONTRACT_ID.to_owned(),
            identity: SemanticId(String::new()),
            validator,
            input,
            output,
            claimed_relation,
            transformation_kind: transformation_kind.into(),
            judgement,
            assumptions,
            evidence_dependencies,
            dependency_fingerprints,
            counterexample,
            limitations,
            residual_trust: residual_trust.into(),
            freshness,
        };
        result.seal();
        result
    }

    pub fn seal(&mut self) {
        self.identity = SemanticId(String::new());
        let material =
            canonical_json_value(self).expect("translation validation result is canonicalizable");
        self.identity = SemanticId(format!(
            "mncs:compiler:translation-validation:{}",
            sha256_hex(material.as_bytes())
        ));
    }

    pub fn identity_is_valid(&self) -> bool {
        let mut material = self.clone();
        material.identity = SemanticId(String::new());
        let canonical = canonical_json_value(&material)
            .expect("translation validation result is canonicalizable");
        self.schema_version == TRANSLATION_VALIDATION_SCHEMA_VERSION
            && self.contract_id == TRANSLATION_VALIDATION_CONTRACT_ID
            && self.input.identity_is_valid()
            && self.output.identity_is_valid()
            && self.identity
                == SemanticId(format!(
                    "mncs:compiler:translation-validation:{}",
                    sha256_hex(canonical.as_bytes())
                ))
    }

    pub fn authorizes_promotion(&self) -> bool {
        self.judgement == TranslationJudgement::Pass
            && self.freshness == EvidenceFreshness::Current
            && self.identity_is_valid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{ArtifactRepresentation, ObservationModelRef};
    use crate::VerifierIndependence;

    fn relation() -> RelationClaim {
        RelationClaim {
            kind: "observational-equivalence".to_owned(),
            observation_model: ObservationModelRef::new(
                "mncs-public-behavior",
                "0.1",
                ["return_value".to_owned()].into_iter().collect(),
            ),
        }
    }

    fn artifact(kind: ArtifactRepresentation, fingerprint: &str) -> CompilerArtifactRef {
        CompilerArtifactRef::new(kind, "0.4", fingerprint)
    }

    fn validator() -> VerifierIdentity {
        VerifierIdentity {
            identity: SemanticId("mncs:validator:test:0.1".to_owned()),
            name: "test-validator".to_owned(),
            version: "0.1".to_owned(),
            independence: VerifierIndependence::IndependentImplementation,
        }
    }

    #[test]
    fn unknown_never_authorizes_promotion() {
        let result = TranslationValidationResult::new(
            validator(),
            artifact(ArtifactRepresentation::Ssa, &"a".repeat(64)),
            artifact(ArtifactRepresentation::Ssa, &"b".repeat(64)),
            relation(),
            "constant-folding",
            TranslationJudgement::Unknown,
            Vec::new(),
            Vec::new(),
            BTreeMap::new(),
            None,
            vec!["unsupported widening multiply".to_owned()],
            "shares mncs-model integer evaluation with the compiler",
            EvidenceFreshness::Current,
        );
        assert!(result.identity_is_valid());
        assert!(!result.authorizes_promotion());
    }

    #[test]
    fn fail_retains_a_counterexample() {
        let result = TranslationValidationResult::new(
            validator(),
            artifact(ArtifactRepresentation::Ssa, &"a".repeat(64)),
            artifact(ArtifactRepresentation::Ssa, &"b".repeat(64)),
            relation(),
            "checked-to-wrapping",
            TranslationJudgement::Fail,
            Vec::new(),
            Vec::new(),
            BTreeMap::new(),
            Some(TranslationCounterexample {
                case_id: "overflow".to_owned(),
                reason: "checked add traps; wrapping add returns a value".to_owned(),
                arguments: Vec::new(),
                expected: None,
                observed: None,
            }),
            Vec::new(),
            "shares mncs-model integer evaluation with the compiler",
            EvidenceFreshness::Current,
        );
        assert_eq!(result.judgement, TranslationJudgement::Fail);
        assert!(result.counterexample.is_some());
        assert!(!result.authorizes_promotion());
    }
}
