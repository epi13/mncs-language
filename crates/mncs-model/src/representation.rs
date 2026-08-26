//! Evidence-aware physical representation choices for one protected logical value.
//!
//! These records deliberately do not participate in logical type identity.
//! They are late-bound realization candidates whose eligibility is decided
//! from exact spatial facts and current capabilities.

use serde::{Deserialize, Serialize};

use crate::{AlignmentCapability, ObligationStatus, SemanticId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutFamily {
    ArrayOfStructures,
    StructureOfArrays,
    ArrayOfStructuresOfArrays { chunk_lanes: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepresentationObservation {
    LogicalValues,
    PhysicalFieldOrder,
    StableAddress,
    ExactSize,
    ExactAlignment,
    Abi,
    SerializedBytes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpatialFacts {
    pub subject: SemanticId,
    pub scope: String,
    pub contiguous: bool,
    pub stride_bytes: Option<u64>,
    pub established_alignment: Option<u64>,
    pub field_order: Vec<String>,
    pub packed: bool,
    pub stable_address: bool,
    pub address_space: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepresentationCandidate {
    pub identity: SemanticId,
    pub logical_type: SemanticId,
    pub family: LayoutFamily,
    pub applicable_targets: Vec<String>,
    pub requires_contiguous: bool,
    pub required_alignment: Option<u64>,
    pub preserves: Vec<RepresentationObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepresentationContext {
    pub logical_type: SemanticId,
    pub target: String,
    pub spatial: SpatialFacts,
    pub alignment: Option<AlignmentCapability>,
    pub protected_observations: Vec<RepresentationObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepresentationDecision {
    pub status: ObligationStatus,
    pub selected: SemanticId,
    pub fallback_used: bool,
    pub reasons: Vec<String>,
}

impl RepresentationCandidate {
    fn eligible(&self, context: &RepresentationContext) -> Result<(), String> {
        if self.logical_type != context.logical_type {
            return Err("candidate is bound to another logical type".to_owned());
        }
        if !self.applicable_targets.is_empty() && !self.applicable_targets.contains(&context.target)
        {
            return Err("candidate does not apply to the selected target".to_owned());
        }
        if matches!(
            self.family,
            LayoutFamily::ArrayOfStructuresOfArrays { chunk_lanes: 0 }
        ) {
            return Err("AoSoA chunk size must be nonzero".to_owned());
        }
        if self.requires_contiguous && !context.spatial.contiguous {
            return Err("contiguity is not established".to_owned());
        }
        if let Some(required) = self.required_alignment {
            let permitted = context.alignment.as_ref().is_some_and(|alignment| {
                alignment.permits(&context.spatial.subject, &context.spatial.scope, required)
            });
            if !permitted {
                return Err(format!("alignment<{required}> is not currently evidenced"));
            }
        }
        if let Some(observation) = context
            .protected_observations
            .iter()
            .find(|observation| !self.preserves.contains(observation))
        {
            return Err(format!("candidate does not preserve {observation:?}"));
        }
        Ok(())
    }
}

/// Select the first eligible preferred candidate, retaining an explicit
/// fallback and an UNKNOWN decision when required facts are unavailable.
pub fn select_representation(
    preferred: &[RepresentationCandidate],
    fallback: &RepresentationCandidate,
    context: &RepresentationContext,
) -> RepresentationDecision {
    let mut reasons = Vec::new();
    for candidate in preferred {
        match candidate.eligible(context) {
            Ok(()) => {
                return RepresentationDecision {
                    status: ObligationStatus::Pass,
                    selected: candidate.identity.clone(),
                    fallback_used: false,
                    reasons,
                };
            }
            Err(reason) => reasons.push(format!("{}: {reason}", candidate.identity.0)),
        }
    }
    match fallback.eligible(context) {
        Ok(()) => RepresentationDecision {
            status: ObligationStatus::Unknown,
            selected: fallback.identity.clone(),
            fallback_used: true,
            reasons,
        },
        Err(reason) => {
            reasons.push(format!("{}: {reason}", fallback.identity.0));
            RepresentationDecision {
                status: ObligationStatus::Fail,
                selected: fallback.identity.clone(),
                fallback_used: true,
                reasons,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{EvidenceFreshness, Obligation, Requirement};

    use super::*;

    fn candidate(
        identity: &str,
        family: LayoutFamily,
        alignment: Option<u64>,
    ) -> RepresentationCandidate {
        RepresentationCandidate {
            identity: SemanticId(identity.to_owned()),
            logical_type: SemanticId("logical:particle-window".to_owned()),
            family,
            applicable_targets: vec!["host".to_owned()],
            requires_contiguous: true,
            required_alignment: alignment,
            preserves: vec![RepresentationObservation::LogicalValues],
        }
    }

    fn context(aligned: bool) -> RepresentationContext {
        let subject = SemanticId("region:particles".to_owned());
        let scope = "epoch:7:bytes:0..256".to_owned();
        let alignment = aligned.then(|| AlignmentCapability {
            identity: SemanticId("alignment:particles:64".to_owned()),
            subject: subject.clone(),
            scope: scope.clone(),
            alignment: 64,
            evidence: Obligation {
                identity: SemanticId("obligation:alignment".to_owned()),
                subject: subject.clone(),
                requirement: SemanticId("requirement:alignment:64".to_owned()),
                status: ObligationStatus::Pass,
                freshness: EvidenceFreshness::Current,
                method: "exact-region-alignment-test".to_owned(),
            },
        });
        let _requirement = Requirement {
            identity: SemanticId("requirement:alignment:64".to_owned()),
            subject: subject.clone(),
            statement: "aligned to at least 64 bytes".to_owned(),
        };
        RepresentationContext {
            logical_type: SemanticId("logical:particle-window".to_owned()),
            target: "host".to_owned(),
            spatial: SpatialFacts {
                subject,
                scope,
                contiguous: true,
                stride_bytes: Some(16),
                established_alignment: aligned.then_some(64),
                field_order: vec!["x".to_owned(), "y".to_owned()],
                packed: false,
                stable_address: false,
                address_space: "host".to_owned(),
            },
            alignment,
            protected_observations: vec![RepresentationObservation::LogicalValues],
        }
    }

    #[test]
    fn aligned_aosoa_is_selected_without_changing_logical_identity() {
        let fallback = candidate("rep:aos", LayoutFamily::ArrayOfStructures, None);
        let preferred = candidate(
            "rep:aosoa:8",
            LayoutFamily::ArrayOfStructuresOfArrays { chunk_lanes: 8 },
            Some(64),
        );
        let decision = select_representation(&[preferred], &fallback, &context(true));
        assert_eq!(decision.status, ObligationStatus::Pass);
        assert_eq!(decision.selected, SemanticId("rep:aosoa:8".to_owned()));
        assert!(!decision.fallback_used);
    }

    #[test]
    fn unknown_alignment_withholds_aosoa_and_keeps_aos_fallback() {
        let fallback = candidate("rep:aos", LayoutFamily::ArrayOfStructures, None);
        let preferred = candidate("rep:soa", LayoutFamily::StructureOfArrays, Some(64));
        let decision = select_representation(&[preferred], &fallback, &context(false));
        assert_eq!(decision.status, ObligationStatus::Unknown);
        assert_eq!(decision.selected, SemanticId("rep:aos".to_owned()));
        assert!(decision.fallback_used);
    }

    #[test]
    fn physical_observation_blocks_an_incompatible_layout_change() {
        let fallback = candidate("rep:aos", LayoutFamily::ArrayOfStructures, None);
        let preferred = candidate("rep:soa", LayoutFamily::StructureOfArrays, None);
        let mut constrained = context(true);
        constrained
            .protected_observations
            .push(RepresentationObservation::PhysicalFieldOrder);
        let decision = select_representation(&[preferred], &fallback, &constrained);
        assert_eq!(decision.status, ObligationStatus::Fail);
    }
}
