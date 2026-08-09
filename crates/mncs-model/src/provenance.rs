use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::canonical::sha256_hex;
use crate::{
    EvidenceFreshness, LoweringEnvelope, ObligationRecord, ObligationStatus, PortabilityEnvelope,
    PortabilityTarget, RealizationClass, SemanticId, SemanticIdentities, VerifierResult,
};

pub const PROVENANCE_SCHEMA_VERSION: &str = "0.2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetIdentity {
    pub identity: SemanticId,
    pub family: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub integer_widths: Vec<u16>,
    pub abi: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Realization {
    pub schema_version: String,
    pub identity: SemanticId,
    pub operation: SemanticId,
    pub class: RealizationClass,
    pub target: TargetIdentity,
    pub obligations: Vec<SemanticId>,
    pub evidence: Vec<SemanticId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealizationSelection {
    pub schema_version: String,
    pub operation: SemanticId,
    pub selected: Option<Realization>,
    pub fallback: Option<RealizationClass>,
    pub permitted: bool,
    pub reason: String,
}

#[derive(Debug, Error)]
pub enum RealizationError {
    #[error("realization class is outside the lowering envelope")]
    ClassNotPermitted,
    #[error("target is outside the portability envelope")]
    TargetNotPermitted,
    #[error("required obligation is absent, stale, or not current PASS")]
    ObligationNotSatisfied,
}

impl Realization {
    pub fn new(
        operation: SemanticId,
        class: RealizationClass,
        target: TargetIdentity,
        obligations: Vec<SemanticId>,
        evidence: Vec<SemanticId>,
    ) -> Self {
        let material =
            serde_json::to_string(&(&operation, &class, &target, &obligations, &evidence))
                .expect("realization is serializable");
        Self {
            schema_version: PROVENANCE_SCHEMA_VERSION.to_owned(),
            identity: SemanticId(format!(
                "mncs:0.2:realization:{}",
                sha256_hex(material.as_bytes())
            )),
            operation,
            class,
            target,
            obligations,
            evidence,
        }
    }

    pub fn select(
        operation: SemanticId,
        class: RealizationClass,
        target: TargetIdentity,
        lowering: &LoweringEnvelope,
        portability: &PortabilityEnvelope,
        obligations: &[ObligationRecord],
        verifier_results: &[VerifierResult],
    ) -> RealizationSelection {
        let fallback = lowering
            .fallback
            .clone()
            .or_else(|| portability.fallback.clone());
        let attempt = || -> Result<Realization, RealizationError> {
            if !lowering.permitted_realizations.contains(&class) {
                return Err(RealizationError::ClassNotPermitted);
            }
            if !target_matches(&target, &portability.target)
                || !portability
                    .required_features
                    .iter()
                    .all(|feature| target.features.contains(feature))
                || !portability
                    .integer_widths
                    .iter()
                    .all(|width| target.integer_widths.contains(width))
                || portability.abi.as_ref() != target.abi.as_ref()
            {
                return Err(RealizationError::TargetNotPermitted);
            }
            let mut evidence = Vec::new();
            for required in &lowering.required_obligations {
                let Some(obligation) = obligations
                    .iter()
                    .find(|obligation| obligation.identity == *required)
                else {
                    return Err(RealizationError::ObligationNotSatisfied);
                };
                if obligation.status != ObligationStatus::Pass
                    || obligation.freshness != EvidenceFreshness::Current
                {
                    return Err(RealizationError::ObligationNotSatisfied);
                }
                let Some(result) = verifier_results
                    .iter()
                    .find(|result| result.obligation == obligation.identity)
                else {
                    return Err(RealizationError::ObligationNotSatisfied);
                };
                if result.status != ObligationStatus::Pass
                    || result.freshness != EvidenceFreshness::Current
                {
                    return Err(RealizationError::ObligationNotSatisfied);
                }
                evidence.push(result.verifier.identity.clone());
            }
            Ok(Realization::new(
                operation.clone(),
                class,
                target,
                lowering.required_obligations.clone(),
                evidence,
            ))
        };
        match attempt() {
            Ok(selected) => RealizationSelection {
                schema_version: PROVENANCE_SCHEMA_VERSION.to_owned(),
                operation: selected.operation.clone(),
                selected: Some(selected),
                fallback,
                permitted: true,
                reason: "realization is inside both envelopes with current PASS evidence"
                    .to_owned(),
            },
            Err(error) => RealizationSelection {
                schema_version: PROVENANCE_SCHEMA_VERSION.to_owned(),
                operation,
                selected: None,
                fallback,
                permitted: false,
                reason: error.to_string(),
            },
        }
    }
}

fn target_matches(target: &TargetIdentity, portability: &PortabilityTarget) -> bool {
    match portability {
        PortabilityTarget::Portable => target.family == "portable",
        PortabilityTarget::ArchitectureFamily(family) => target.family == *family,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformationRecord {
    pub schema_version: String,
    pub identity: SemanticId,
    pub input_identity: SemanticId,
    pub output_identity: SemanticId,
    pub rule: String,
    pub operation: SemanticId,
    pub realization: Option<SemanticId>,
    pub obligations_required: Vec<SemanticId>,
    pub evidence_consumed: Vec<SemanticId>,
    pub runtime_checks_inserted: Vec<SemanticId>,
    pub facts_established: Vec<SemanticId>,
    pub backend_promises_permitted: Vec<String>,
    pub backend_promises_withheld: Vec<String>,
    pub invalidated_evidence: Vec<SemanticId>,
    pub target: TargetIdentity,
    pub implementation: SemanticId,
}

impl TransformationRecord {
    pub fn new(
        input_identity: SemanticId,
        output_identity: SemanticId,
        rule: String,
        operation: SemanticId,
        target: TargetIdentity,
        implementation: SemanticId,
    ) -> Self {
        let material = serde_json::to_string(&(
            &input_identity,
            &output_identity,
            &rule,
            &operation,
            &target,
            &implementation,
        ))
        .expect("transformation is serializable");
        Self {
            schema_version: PROVENANCE_SCHEMA_VERSION.to_owned(),
            identity: SemanticId(format!(
                "mncs:0.2:transformation:{}",
                sha256_hex(material.as_bytes())
            )),
            input_identity,
            output_identity,
            rule,
            operation,
            realization: None,
            obligations_required: Vec::new(),
            evidence_consumed: Vec::new(),
            runtime_checks_inserted: Vec::new(),
            facts_established: Vec::new(),
            backend_promises_permitted: Vec::new(),
            backend_promises_withheld: Vec::new(),
            invalidated_evidence: Vec::new(),
            target,
            implementation,
        }
    }
}

pub fn evidence_is_current(
    evidence: &SemanticId,
    identities: &SemanticIdentities,
    records: &[crate::EvidenceRecord],
) -> bool {
    records.iter().any(|record| {
        &record.identity == evidence
            && record.freshness == EvidenceFreshness::Current
            && identities.fingerprint(&record.subject).is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeterministicVerifier;

    fn target() -> TargetIdentity {
        TargetIdentity {
            identity: SemanticId("target:portable".to_owned()),
            family: "portable".to_owned(),
            features: Vec::new(),
            integer_widths: vec![32],
            abi: None,
        }
    }

    #[test]
    fn realization_requires_envelopes_and_current_obligations() {
        let program = crate::validation::tests::valid_program();
        let obligations = program.generate_obligations().obligations;
        let results = program.verify_obligations(&DeterministicVerifier::default());
        let envelope = LoweringEnvelope {
            schema_version: PROVENANCE_SCHEMA_VERSION.to_owned(),
            permitted_realizations: vec![RealizationClass::PortableReference],
            prohibited_transformations: Vec::new(),
            required_obligations: Vec::new(),
            fallback: Some(RealizationClass::RuntimeChecked),
        };
        let portability = PortabilityEnvelope {
            schema_version: PROVENANCE_SCHEMA_VERSION.to_owned(),
            target: PortabilityTarget::Portable,
            required_features: Vec::new(),
            integer_widths: vec![32],
            abi: None,
            fallback: Some(RealizationClass::RuntimeChecked),
        };
        let selected = Realization::select(
            SemanticId("operation:add".to_owned()),
            RealizationClass::PortableReference,
            target(),
            &envelope,
            &portability,
            &obligations,
            &results,
        );
        assert!(selected.permitted);
        assert!(selected.selected.is_some());
        let rejected = Realization::select(
            SemanticId("operation:add".to_owned()),
            RealizationClass::Specialized("avx".to_owned()),
            target(),
            &envelope,
            &portability,
            &obligations,
            &results,
        );
        assert!(!rejected.permitted);
        assert_eq!(rejected.fallback, Some(RealizationClass::RuntimeChecked));
    }

    #[test]
    fn transformation_identity_is_deterministic_and_separate() {
        let first = TransformationRecord::new(
            SemanticId("input".to_owned()),
            SemanticId("output".to_owned()),
            "body-to-hir".to_owned(),
            SemanticId("operation:add".to_owned()),
            target(),
            SemanticId("implementation:lowerer".to_owned()),
        );
        let second = TransformationRecord::new(
            first.input_identity.clone(),
            first.output_identity.clone(),
            first.rule.clone(),
            first.operation.clone(),
            first.target.clone(),
            first.implementation.clone(),
        );
        assert_eq!(first.identity, second.identity);
        assert_ne!(first.identity, first.operation);
    }
}
