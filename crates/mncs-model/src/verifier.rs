use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::canonical::sha256_hex;
use crate::{
    AlignmentCapability, EvidenceFreshness, IntegerOperation, ObligationRecord, ObligationStatus,
    SemanticId, SemanticIdentities,
};

pub const VERIFIER_SCHEMA_VERSION: &str = "0.2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAuthorityClass {
    Verifier,
    DiagnosticObservation,
    ProposalResult,
    #[default]
    DevelopmentMeasurement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifierIndependence {
    Generator,
    Compiler,
    IndependentImplementation,
    IndependentEnvironment,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierIdentity {
    pub identity: SemanticId,
    pub name: String,
    pub version: String,
    pub independence: VerifierIndependence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifierMethod {
    IntegerRange,
    Alignment,
    CapabilityConsistency,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegerVerifierInput {
    pub operation: IntegerOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignmentVerifierInput {
    pub capability: AlignmentCapability,
    pub required_alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityVerifierInput {
    /// `Some(true)` and `Some(false)` represent established authorization
    /// facts; `None` preserves an unresolved obligation as UNKNOWN.
    pub authorized: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerifierInput {
    Integer(IntegerVerifierInput),
    Alignment(AlignmentVerifierInput),
    Capability(CapabilityVerifierInput),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierRequest {
    pub schema_version: String,
    pub obligation: ObligationRecord,
    pub subject: SemanticId,
    pub scope: String,
    pub input: VerifierInput,
    pub assumptions: Vec<SemanticId>,
    pub dependencies: Vec<SemanticId>,
    pub dependency_fingerprints: BTreeMap<SemanticId, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierResult {
    pub schema_version: String,
    pub obligation: SemanticId,
    pub subject: SemanticId,
    pub scope: String,
    pub status: ObligationStatus,
    pub verifier: VerifierIdentity,
    #[serde(default)]
    pub authority: EvidenceAuthorityClass,
    pub method: VerifierMethod,
    pub assumptions: Vec<SemanticId>,
    pub dependencies: Vec<SemanticId>,
    pub dependency_fingerprints: BTreeMap<SemanticId, String>,
    pub artifact: Option<String>,
    pub limitations: Vec<String>,
    pub freshness: EvidenceFreshness,
}

#[derive(Debug, Error)]
pub enum VerifierArtifactError {
    #[error("invalid verifier artifact JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("verifier artifact schema {actual:?} does not match request schema {expected:?}")]
    Schema { actual: String, expected: String },
    #[error("verifier artifact does not match the exact requested obligation or subject")]
    SubjectMismatch,
    #[error("verifier artifact scope does not match the requested scope")]
    ScopeMismatch,
    #[error("verifier artifact assumptions or dependencies do not match the request")]
    DependencyMismatch,
    #[error("verifier artifact has no verifier identity")]
    MissingVerifier,
}

pub trait MicroVerifier {
    fn identity(&self) -> &VerifierIdentity;
    fn verify(&self, request: &VerifierRequest) -> VerifierResult;
}

#[derive(Debug, Clone)]
pub struct DeterministicVerifier {
    identity: VerifierIdentity,
}

impl Default for DeterministicVerifier {
    fn default() -> Self {
        Self::new("mncs-deterministic", "0.2")
    }
}

impl DeterministicVerifier {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            identity: VerifierIdentity {
                identity: SemanticId(format!(
                    "mncs:0.2:verifier:{}",
                    sha256_hex(format!("{name}:{version}").as_bytes())
                )),
                name: name.to_owned(),
                version: version.to_owned(),
                independence: VerifierIndependence::IndependentImplementation,
            },
        }
    }

    fn identity_value(&self) -> VerifierIdentity {
        self.identity.clone()
    }
}

impl MicroVerifier for DeterministicVerifier {
    fn identity(&self) -> &VerifierIdentity {
        &self.identity
    }

    fn verify(&self, request: &VerifierRequest) -> VerifierResult {
        let verifier = self.identity_value();
        let (method, status, limitations) = if request.obligation.subject != request.subject {
            (
                VerifierMethod::IntegerRange,
                ObligationStatus::Fail,
                vec!["request subject does not match obligation subject".to_owned()],
            )
        } else {
            match &request.input {
                VerifierInput::Integer(input) => {
                    let evaluation = input.operation.evaluate();
                    let status = if evaluation.overflow {
                        ObligationStatus::Fail
                    } else {
                        ObligationStatus::Pass
                    };
                    (
                        VerifierMethod::IntegerRange,
                        status,
                        vec![
                            "bounded integer evaluation only; no general symbolic proof".to_owned()
                        ],
                    )
                }
                VerifierInput::Alignment(input) => {
                    let status = if input.capability.permits(
                        &request.subject,
                        &request.scope,
                        input.required_alignment,
                    ) {
                        ObligationStatus::Pass
                    } else {
                        ObligationStatus::Fail
                    };
                    (
                        VerifierMethod::Alignment,
                        status,
                        vec!["exact subject and scope alignment check".to_owned()],
                    )
                }
                VerifierInput::Capability(input) => (
                    VerifierMethod::CapabilityConsistency,
                    match input.authorized {
                        Some(true) => ObligationStatus::Pass,
                        Some(false) => ObligationStatus::Fail,
                        None => ObligationStatus::Unknown,
                    },
                    vec!["declared effect/capability relationship check".to_owned()],
                ),
            }
        };
        let artifact = Some(format!(
            "evidence://{}",
            sha256_hex(
                format!(
                    "{}:{}:{:?}",
                    request.obligation.identity, request.subject, status
                )
                .as_bytes()
            )
        ));
        VerifierResult {
            schema_version: VERIFIER_SCHEMA_VERSION.to_owned(),
            obligation: request.obligation.identity.clone(),
            subject: request.subject.clone(),
            scope: request.scope.clone(),
            status,
            verifier,
            authority: EvidenceAuthorityClass::Verifier,
            method,
            assumptions: request.assumptions.clone(),
            dependencies: request.dependencies.clone(),
            dependency_fingerprints: request.dependency_fingerprints.clone(),
            artifact,
            limitations,
            freshness: EvidenceFreshness::Current,
        }
    }
}

impl crate::Program {
    pub fn verifier_requests(&self) -> Vec<VerifierRequest> {
        let identities = self.semantic_identities();
        self.generate_obligations()
            .obligations
            .into_iter()
            .map(|obligation| {
                let dependency_fingerprints = obligation
                    .dependencies
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
                    .collect();
                let authorized = match obligation.status {
                    ObligationStatus::Pass => Some(true),
                    ObligationStatus::Fail => Some(false),
                    ObligationStatus::Unknown => None,
                };
                VerifierRequest {
                    schema_version: VERIFIER_SCHEMA_VERSION.to_owned(),
                    subject: obligation.subject.clone(),
                    scope: self.module.clone(),
                    input: VerifierInput::Capability(CapabilityVerifierInput { authorized }),
                    assumptions: obligation.assumptions.clone(),
                    dependencies: obligation.dependencies.clone(),
                    dependency_fingerprints,
                    obligation,
                }
            })
            .collect()
    }

    pub fn verify_obligations<V: MicroVerifier>(&self, verifier: &V) -> Vec<VerifierResult> {
        self.verifier_requests()
            .iter()
            .map(|request| verifier.verify(request))
            .collect()
    }
}

impl VerifierResult {
    pub fn validate_against(
        &self,
        request: &VerifierRequest,
        current: &SemanticIdentities,
    ) -> Result<Self, VerifierArtifactError> {
        if self.schema_version != request.schema_version {
            return Err(VerifierArtifactError::Schema {
                actual: self.schema_version.clone(),
                expected: request.schema_version.clone(),
            });
        }
        if self.obligation != request.obligation.identity || self.subject != request.subject {
            return Err(VerifierArtifactError::SubjectMismatch);
        }
        if self.scope != request.scope {
            return Err(VerifierArtifactError::ScopeMismatch);
        }
        if self.assumptions != request.assumptions
            || self.dependencies != request.dependencies
            || self.dependency_fingerprints != request.dependency_fingerprints
        {
            return Err(VerifierArtifactError::DependencyMismatch);
        }
        if self.verifier.identity.0.trim().is_empty() {
            return Err(VerifierArtifactError::MissingVerifier);
        }
        let mut refreshed = self.clone();
        refreshed.freshness = EvidenceFreshness::Unknown;
        Ok(refreshed.refresh(current))
    }

    pub fn import_json(
        input: &str,
        request: &VerifierRequest,
        current: &SemanticIdentities,
    ) -> Result<Self, VerifierArtifactError> {
        let artifact: Self = serde_json::from_str(input)?;
        artifact.validate_against(request, current)
    }

    pub fn refresh(&self, current: &SemanticIdentities) -> Self {
        let mut refreshed = self.clone();
        refreshed.freshness = if self.dependencies.iter().all(|dependency| {
            current.fingerprint(dependency).is_some_and(|fingerprint| {
                self.dependency_fingerprints
                    .get(dependency)
                    .map(String::as_str)
                    == Some(fingerprint)
            })
        }) {
            EvidenceFreshness::Current
        } else {
            EvidenceFreshness::Stale
        };
        refreshed
    }

    pub fn can_satisfy(&self, obligation: &ObligationRecord, subject: &SemanticId) -> bool {
        self.obligation == obligation.identity
            && self.subject == *subject
            && self.status == ObligationStatus::Pass
            && self.freshness == EvidenceFreshness::Current
            && self.authority == EvidenceAuthorityClass::Verifier
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::tests::valid_program;
    use crate::{ArithmeticIntent, IntegerType};

    fn integer_request() -> VerifierRequest {
        let operation = IntegerOperation {
            operator: "add".to_owned(),
            operand_type: IntegerType {
                bits: 8,
                signed: true,
            },
            left: 1,
            right: 2,
            intent: ArithmeticIntent::Checked,
        };
        let subject = operation.identity();
        let obligation = ObligationRecord {
            schema_version: "0.2".to_owned(),
            identity: SemanticId("obligation:range".to_owned()),
            subject: subject.clone(),
            requirement: SemanticId("requirement:no-overflow".to_owned()),
            status: ObligationStatus::Unknown,
            method: "request".to_owned(),
            assumptions: Vec::new(),
            dependencies: vec![subject.clone()],
            freshness: EvidenceFreshness::Unknown,
            fallback: Some("conservative".to_owned()),
        };
        let mut dependency_fingerprints = BTreeMap::new();
        dependency_fingerprints.insert(subject.clone(), "operation-fingerprint".to_owned());
        VerifierRequest {
            schema_version: "0.2".to_owned(),
            obligation,
            subject,
            scope: "expression".to_owned(),
            input: VerifierInput::Integer(IntegerVerifierInput { operation }),
            assumptions: Vec::new(),
            dependencies: vec![SemanticId("operation:dependency".to_owned())],
            dependency_fingerprints,
        }
    }

    #[test]
    fn deterministic_verifier_returns_identity_bound_pass_and_artifact() {
        let verifier = DeterministicVerifier::new("range", "1");
        let request = integer_request();
        let result = verifier.verify(&request);
        assert_eq!(result.status, ObligationStatus::Pass);
        assert_eq!(result.obligation, request.obligation.identity);
        assert!(result.artifact.is_some());
        assert_eq!(result.freshness, EvidenceFreshness::Current);
    }

    #[test]
    fn verifier_result_becomes_stale_when_dependency_fingerprint_changes() {
        let verifier = DeterministicVerifier::new("range", "1");
        let request = integer_request();
        let result = verifier.verify(&request);
        let identities = valid_program().semantic_identities();
        assert_eq!(
            result.refresh(&identities).freshness,
            EvidenceFreshness::Stale
        );
    }

    #[test]
    fn verifier_cannot_satisfy_wrong_subject_or_stale_obligation() {
        let verifier = DeterministicVerifier::new("range", "1");
        let request = integer_request();
        let result = verifier.verify(&request);
        assert!(!result.can_satisfy(&request.obligation, &SemanticId("other".to_owned())));
        let mut stale = result.clone();
        stale.freshness = EvidenceFreshness::Stale;
        assert!(!stale.can_satisfy(&request.obligation, &request.subject));
    }

    #[test]
    fn diagnostic_observation_cannot_satisfy_a_proof_obligation() {
        let verifier = DeterministicVerifier::default();
        let request = integer_request();
        let mut result = verifier.verify(&request);
        result.authority = EvidenceAuthorityClass::DiagnosticObservation;
        assert!(!result.can_satisfy(&request.obligation, &request.subject));
    }

    #[test]
    fn imported_verifier_artifacts_are_scope_and_dependency_bound() {
        let verifier = DeterministicVerifier::default();
        let request = integer_request();
        let result = verifier.verify(&request);
        let imported = result
            .validate_against(&request, &valid_program().semantic_identities())
            .expect("artifact validation");
        assert_eq!(imported.freshness, EvidenceFreshness::Stale);
        let mut wrong_scope = result;
        "different-scope".clone_into(&mut wrong_scope.scope);
        assert!(matches!(
            wrong_scope.validate_against(&request, &valid_program().semantic_identities()),
            Err(VerifierArtifactError::ScopeMismatch)
        ));
    }

    #[test]
    fn capability_verifier_preserves_unknown_without_authority_fact() {
        let verifier = DeterministicVerifier::default();
        let mut request = integer_request();
        request.input = VerifierInput::Capability(CapabilityVerifierInput { authorized: None });
        let result = verifier.verify(&request);
        assert_eq!(result.status, ObligationStatus::Unknown);
    }
}
