//! Structured capability-gap artifacts (`docs/capability-gap-artifacts.md`).
//!
//! When understood MNCS semantics fall outside the active profile, a backend
//! cannot realize valid semantics, or a required library/runtime operation is
//! absent, callers can record the limitation as inspectable evidence instead
//! of prose. The artifact is content-addressed: its identity covers every
//! field that changes semantics, scope, or reproduction inputs, while human
//! wording, line numbers, and local paths are redacted out of the identity.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::canonical::sha256_hex;
use crate::identity::SemanticId;

/// Artifact kind marker; the only kind this module issues.
pub const CAPABILITY_GAP_ARTIFACT_KIND: &str = "mncs.capability_gap";
/// Contract revision of the artifact shape defined here.
pub const CAPABILITY_GAP_CONTRACT_REVISION: &str = "0.1";

const MAX_ID_LENGTH: usize = 256;
const MAX_TEXT_LENGTH: usize = 2048;
const MAX_LIST_ENTRIES: usize = 64;
const MAX_LIST_ENTRY_LENGTH: usize = 512;

/// Where the requested semantics got stuck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapObstruction {
    Parser,
    Typing,
    SemanticValidation,
    Lowering,
    Runtime,
    Backend,
    Library,
    Verification,
}

/// Honest tri-state outcome for the requested semantics in scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GapStatus {
    Pass,
    Fail,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityGap {
    pub artifact_kind: String,
    pub contract_revision: String,
    pub gap_id: String,
    pub producer: String,
    pub compiler_identity: Option<String>,
    pub source_revision: Option<String>,
    pub source_artifact: String,
    pub source_location: String,
    pub requested_semantics: String,
    pub obstruction: GapObstruction,
    pub active_profile: String,
    pub backends: Vec<String>,
    pub closest_supported_semantics: Option<String>,
    pub reproducer: String,
    pub protected_properties: Vec<String>,
    pub approximation_prohibited: bool,
    pub evidence_requirements: Vec<String>,
    pub status: GapStatus,
    pub unresolved_fields: Vec<String>,
    pub identity: SemanticId,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CapabilityGapError {
    #[error("gap field {0} is missing or out of bounds")]
    Bounds(String),
    #[error("gap identity does not match its content")]
    IdentityMismatch,
}

/// Replace any local directory components with a stable redaction marker.
///
/// Only the final path segment survives, so two checkouts of the same
/// repository produce the same bytes and no hostname or home directory
/// leaks into shared evidence.
pub fn redact_local_path(value: &str) -> String {
    let tail = value.rsplit(['/', '\\']).next().unwrap_or(value);
    if tail == value {
        return value.to_owned();
    }
    format!("<local-path>/{tail}")
}

fn bounded(value: &str, field: &str, maximum: usize) -> Result<String, CapabilityGapError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > maximum {
        return Err(CapabilityGapError::Bounds(field.to_owned()));
    }
    Ok(trimmed.to_owned())
}

fn bounded_list(values: &[String], field: &str) -> Result<Vec<String>, CapabilityGapError> {
    if values.len() > MAX_LIST_ENTRIES {
        return Err(CapabilityGapError::Bounds(field.to_owned()));
    }
    let mut sorted: Vec<String> = values.to_vec();
    for value in &sorted {
        if value.len() > MAX_LIST_ENTRY_LENGTH {
            return Err(CapabilityGapError::Bounds(field.to_owned()));
        }
    }
    sorted.sort();
    sorted.dedup();
    Ok(sorted)
}

fn identity_for(projection: &serde_json::Value) -> SemanticId {
    let bytes = serde_json::to_vec(projection).expect("gap projection is JSON");
    SemanticId(format!("mncs:0.2:capability-gap:{}", sha256_hex(&bytes)))
}

impl CapabilityGap {
    /// Build a validated, identity-bound artifact.
    ///
    /// Identity covers semantics, scope, and reproduction inputs only:
    /// free-text rationale and redacted locations are excluded so wording
    /// edits and checkout moves never rename the gap.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        gap_id: impl Into<String>,
        producer: impl Into<String>,
        source_artifact: impl Into<String>,
        source_location: impl Into<String>,
        requested_semantics: impl Into<String>,
        obstruction: GapObstruction,
        active_profile: impl Into<String>,
        backends: Vec<String>,
        reproducer: impl Into<String>,
        status: GapStatus,
    ) -> Result<Self, CapabilityGapError> {
        let gap = Self {
            artifact_kind: CAPABILITY_GAP_ARTIFACT_KIND.to_owned(),
            contract_revision: CAPABILITY_GAP_CONTRACT_REVISION.to_owned(),
            gap_id: bounded(&gap_id.into(), "gap_id", MAX_ID_LENGTH)?,
            producer: bounded(&producer.into(), "producer", MAX_ID_LENGTH)?,
            compiler_identity: None,
            source_revision: None,
            source_artifact: bounded(&source_artifact.into(), "source_artifact", MAX_TEXT_LENGTH)?,
            source_location: redact_local_path(&source_location.into()),
            requested_semantics: bounded(
                &requested_semantics.into(),
                "requested_semantics",
                MAX_TEXT_LENGTH,
            )?,
            obstruction,
            active_profile: bounded(&active_profile.into(), "active_profile", MAX_ID_LENGTH)?,
            backends: bounded_list(&backends, "backends")?,
            closest_supported_semantics: None,
            reproducer: bounded(&reproducer.into(), "reproducer", MAX_TEXT_LENGTH)?,
            protected_properties: Vec::new(),
            approximation_prohibited: false,
            evidence_requirements: Vec::new(),
            status,
            unresolved_fields: Vec::new(),
            identity: SemanticId(String::new()),
        };
        let identity = identity_for(&gap.identity_projection());
        Ok(Self { identity, ..gap })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_optionals(
        mut self,
        compiler_identity: Option<String>,
        source_revision: Option<String>,
        closest_supported_semantics: Option<String>,
        protected_properties: Vec<String>,
        approximation_prohibited: bool,
        evidence_requirements: Vec<String>,
        unresolved_fields: Vec<String>,
    ) -> Result<Self, CapabilityGapError> {
        self.compiler_identity = compiler_identity;
        self.source_revision = source_revision;
        self.closest_supported_semantics = closest_supported_semantics;
        self.protected_properties = bounded_list(&protected_properties, "protected_properties")?;
        self.approximation_prohibited = approximation_prohibited;
        self.evidence_requirements = bounded_list(&evidence_requirements, "evidence_requirements")?;
        self.unresolved_fields = bounded_list(&unresolved_fields, "unresolved_fields")?;
        // Optional human-facing fields never enter the identity projection,
        // so this reassignment is a no-op by construction; recompute anyway
        // so a future projection change fails loudly here instead of in data.
        self.identity = identity_for(&self.identity_projection());
        Ok(self)
    }

    fn identity_projection(&self) -> serde_json::Value {
        serde_json::json!({
            "artifact_kind": self.artifact_kind,
            "contract_revision": self.contract_revision,
            "gap_id": self.gap_id,
            "source_artifact": self.source_artifact,
            "requested_semantics": self.requested_semantics,
            "obstruction": self.obstruction,
            "active_profile": self.active_profile,
            "backends": self.backends,
            "reproducer": self.reproducer,
            "status": self.status,
        })
    }

    /// Recompute the identity and check it against the stored one.
    pub fn verify_identity(&self) -> Result<(), CapabilityGapError> {
        if self.artifact_kind != CAPABILITY_GAP_ARTIFACT_KIND {
            return Err(CapabilityGapError::Bounds("artifact_kind".to_owned()));
        }
        if self.contract_revision != CAPABILITY_GAP_CONTRACT_REVISION {
            return Err(CapabilityGapError::Bounds("contract_revision".to_owned()));
        }
        bounded(&self.gap_id, "gap_id", MAX_ID_LENGTH)?;
        bounded(&self.producer, "producer", MAX_ID_LENGTH)?;
        bounded(&self.source_artifact, "source_artifact", MAX_TEXT_LENGTH)?;
        bounded(
            &self.requested_semantics,
            "requested_semantics",
            MAX_TEXT_LENGTH,
        )?;
        bounded(&self.active_profile, "active_profile", MAX_ID_LENGTH)?;
        bounded(&self.reproducer, "reproducer", MAX_TEXT_LENGTH)?;
        bounded_list(&self.backends, "backends")?;
        bounded_list(&self.protected_properties, "protected_properties")?;
        bounded_list(&self.evidence_requirements, "evidence_requirements")?;
        bounded_list(&self.unresolved_fields, "unresolved_fields")?;
        let recomputed = identity_for(&self.identity_projection());
        if recomputed != self.identity {
            return Err(CapabilityGapError::IdentityMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh_gap() -> CapabilityGap {
        CapabilityGap::new(
            "commons.mesh/qualified-record-construction",
            "commons-mesh-pressure",
            "commons.mesh.lattice_check",
            "/home/epi13/Documents/Projects/MNCS-Commons/src/mncs_commons/mesh/mncs/commons/mesh/lattice_check.mncs",
            "construct an imported record value through a Profile 0.9 alias route",
            GapObstruction::Typing,
            "0.10",
            vec!["mncs-research-bytecode".to_owned(), "portable-wasm".to_owned()],
            "status.StatusPair { left: left, right: right } via `use ... as status`",
            GapStatus::Fail,
        )
        .expect("fixture gap")
    }

    #[test]
    fn redaction_removes_local_directories() {
        assert_eq!(
            redact_local_path("lattice_check.mncs"),
            "lattice_check.mncs"
        );
        assert_eq!(
            redact_local_path("/home/user/work/tree/lattice_check.mncs"),
            "<local-path>/lattice_check.mncs"
        );
        assert_eq!(
            redact_local_path("C:\\work\\tree\\lattice_check.mncs"),
            "<local-path>/lattice_check.mncs"
        );
    }

    #[test]
    fn identity_is_stable_and_verifies() {
        let gap = mesh_gap();
        assert!(gap
            .identity
            .as_str()
            .starts_with("mncs:0.2:capability-gap:"));
        assert_eq!(gap.identity.as_str(), mesh_gap().identity.as_str());
        assert!(gap.verify_identity().is_ok());
        // Checkout moves never rename the gap.
        let relocated = CapabilityGap::new(
            "commons.mesh/qualified-record-construction",
            "commons-mesh-pressure",
            "commons.mesh.lattice_check",
            "/tmp/other-checkout/lattice_check.mncs",
            "construct an imported record value through a Profile 0.9 alias route",
            GapObstruction::Typing,
            "0.10",
            vec![
                "portable-wasm".to_owned(),
                "mncs-research-bytecode".to_owned(),
            ],
            "status.StatusPair { left: left, right: right } via `use ... as status`",
            GapStatus::Fail,
        )
        .expect("relocated gap");
        assert_eq!(gap.identity, relocated.identity);
    }

    #[test]
    fn tampering_breaks_verification() {
        let mut gap = mesh_gap();
        gap.requested_semantics = "something else entirely".to_owned();
        assert_eq!(
            gap.verify_identity(),
            Err(CapabilityGapError::IdentityMismatch)
        );
    }

    #[test]
    fn checked_in_fixture_round_trips() {
        let raw =
            include_str!("../../../examples/capability-gaps/qualified-record-construction.json");
        let gap: CapabilityGap = serde_json::from_str(raw).expect("fixture parses");
        assert!(gap.verify_identity().is_ok());
        assert_eq!(gap.gap_id, "commons.mesh/qualified-record-construction");
        assert_eq!(gap.obstruction, GapObstruction::Typing);
        assert_eq!(gap.status, GapStatus::Fail);
        assert_eq!(
            gap.identity.as_str(),
            "mncs:0.2:capability-gap:e2ac5125d915242e2c1ebddfd4b4a2bfe5a0f3fa03ff2d1d42938ffcf386dc7e"
        );
        // Local paths never survive redaction.
        assert!(!gap.source_location.contains("/home/"));
    }

    #[test]
    fn embeddable_execution_fixture_round_trips() {
        let raw =
            include_str!("../../../examples/capability-gaps/embeddable-execution.json");
        let gap: CapabilityGap = serde_json::from_str(raw).expect("fixture parses");
        assert!(gap.verify_identity().is_ok());
        assert_eq!(gap.gap_id, "commons.mesh/embeddable-execution");
        assert_eq!(gap.obstruction, GapObstruction::Runtime);
        assert_eq!(gap.status, GapStatus::Fail);
        assert_eq!(
            gap.identity.as_str(),
            "mncs:0.2:capability-gap:6e9b55bd409080f455fbc59f3e76c6fd020b41df118cbf1eba6288bcf37a7a01"
        );
    }

    #[test]
    fn bounds_fail_closed() {
        assert!(mesh_gap().verify_identity().is_ok());
        assert_eq!(
            CapabilityGap::new(
                "",
                "producer",
                "artifact",
                "loc",
                "semantics",
                GapObstruction::Parser,
                "0.10",
                vec![],
                "reproducer",
                GapStatus::Unknown,
            ),
            Err(CapabilityGapError::Bounds("gap_id".to_owned()))
        );
        assert_eq!(
            CapabilityGap::new(
                "id",
                "producer",
                "artifact",
                "loc",
                "semantics",
                GapObstruction::Parser,
                "0.10",
                vec!["b".to_owned(); 65],
                "reproducer",
                GapStatus::Unknown,
            ),
            Err(CapabilityGapError::Bounds("backends".to_owned()))
        );
    }
}
