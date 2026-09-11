//! Tranche-0.2 proof transport: the compiler representation of an admitted proof.
//!
//! A [`ProofRelationship`] is pure DATA: the MNCS-issued binding (decoded
//! from genuine MNCS execution by `mncs-compiler` admission), the
//! independent checker's corroboration report, and the dependency
//! fingerprints the admission was sealed under. It carries NO authority of
//! its own: every consumer must additionally hold a fresh MNCS
//! `binding_reusable` PASS for the exact current fingerprints, and this
//! module's own gates only ever downgrade (withhold), never upgrade.
//!
//! Dependency slot convention (positional; MNCS compares bytes exactly):
//! - slot 0: obligation fingerprint (sha256 of the obligation identity),
//! - slot 1: operation fingerprint (the SSA operation the use authorizes),
//! - slot 2: HIR content fingerprint (proof-free: snapshotted before
//!   relationships attach, so attachment itself cannot perturb it),
//! - slot 3: reserved, always zero.
//!
//! The relationship survives HIR -> SSA -> lowering evidence as validated
//! metadata: attachment re-validates the obligation and the slot
//! fingerprints at every stage, and any drift withholds the proof-bearing
//! evidence record instead of failing loudly elsewhere.

use serde::{Deserialize, Serialize};

use crate::{DepAssumptionSet, DepCorroboration, DepVerdict, ExecutionValue, TransformationRecord};

pub const PROOF_RELATIONSHIP_SCHEMA_VERSION: &str = "0.1";
/// Dependency slots bound into one admission (slot 3 is reserved zero).
pub const PROOF_DEPENDENCY_SLOTS: usize = 4;
/// Rule name recorded when a proof relationship enters lowering evidence.
pub const PROOF_EVIDENCE_RULE: &str = "tranche-0.2-proof-bearing-evidence";

/// Evidence behind one admission: which MNCS program issued the binding and
/// how many steps the seal execution took. Audit trail only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofAdmissionEvidence {
    pub program_fingerprint: String,
    pub seal_steps: u64,
}

/// The compiler representation of one admitted tranche-0.2 proof: the exact
/// identities MNCS sealed, the canonical assumption set MNCS observed, the
/// MNCS-issued verdict, the sealed binding record itself (opaque to every
/// consumer except MNCS re-validation), and the corroboration report.
///
/// Construction is restricted to the admission path (`mncs-compiler`
/// executes the MNCS kernel and the independent checker); every other
/// module only validates and transports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofRelationship {
    pub schema_version: String,
    pub proof_identity: String,
    pub obligation: String,
    pub kernel: String,
    pub assumptions: DepAssumptionSet,
    pub mncs_verdict: DepVerdict,
    pub sealed_binding: ExecutionValue,
    pub admission: ProofAdmissionEvidence,
    pub corroboration: DepCorroboration,
    /// Hex fingerprints in slot order (see the slot convention above).
    pub dependencies: Vec<String>,
}

impl ProofRelationship {
    pub fn consumable(&self) -> bool {
        self.schema_version == PROOF_RELATIONSHIP_SCHEMA_VERSION
            && self.corroboration.consumable()
            && self.mncs_verdict == DepVerdict::Pass
            && self.assumptions.valid
    }
}

/// How validation refused a relationship at a compiler stage. Refusal is
/// always fail-closed: the relationship is dropped or the evidence is
/// withheld, never repaired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportMismatch {
    NotConsumable,
    ObligationMismatch,
    KernelMismatch,
    DependencyMismatch,
    ObligationAbsent,
}

/// Versioned proof reference carried into backend artifacts (tranche-0.3
/// proof transport). This is DATA, not authority: it names the exact
/// admitted proof, the obligation and kernel it was sealed under, the
/// slot-ordered dependency fingerprints, and the SSA fingerprint the
/// backend lowered, so any consumer can re-fetch the full relationship
/// and proof-bearing evidence record from the artifact's SSA input and
/// re-validate them without trusting the backend's word. A backend that
/// cannot preserve these references must refuse proof-bearing SSA
/// instead of emitting a proof-silent artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofBindingRef {
    pub schema_version: String,
    pub proof_identity: String,
    pub obligation: String,
    pub kernel: String,
    /// Hex fingerprints in slot order (see the slot convention above).
    pub dependencies: Vec<String>,
    /// Full SSA fingerprint the backend lowered when sealing this ref.
    pub ssa_fingerprint: String,
}

impl ProofRelationship {
    /// Project this relationship to its artifact-carried reference form,
    /// bound to the exact SSA fingerprint the backend lowered.
    pub fn binding_ref(&self, ssa_fingerprint: &str) -> ProofBindingRef {
        ProofBindingRef {
            schema_version: self.schema_version.clone(),
            proof_identity: self.proof_identity.clone(),
            obligation: self.obligation.clone(),
            kernel: self.kernel.clone(),
            dependencies: self.dependencies.clone(),
            ssa_fingerprint: ssa_fingerprint.to_owned(),
        }
    }
}

impl crate::SsaModule {
    /// Deterministic (sorted, deduplicated) proof references for every
    /// relationship attached to this SSA, bound to this SSA's fingerprint.
    /// Empty for proof-free SSA, so proof-free artifacts are untouched.
    pub fn proof_binding_refs(&self) -> Vec<ProofBindingRef> {
        let fingerprint = self.fingerprint().unwrap_or_default();
        let mut refs: Vec<ProofBindingRef> = self
            .proof_relationships
            .iter()
            .map(|relationship| relationship.binding_ref(&fingerprint))
            .collect();
        refs.sort_by(|left, right| {
            (
                &left.proof_identity,
                &left.obligation,
                &left.kernel,
                &left.dependencies,
                &left.ssa_fingerprint,
            )
                .cmp(&(
                    &right.proof_identity,
                    &right.obligation,
                    &right.kernel,
                    &right.dependencies,
                    &right.ssa_fingerprint,
                ))
        });
        refs.dedup();
        refs
    }
}

impl std::fmt::Display for TransportMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConsumable => write!(f, "relationship is not consumable"),
            Self::ObligationMismatch => write!(f, "obligation identity drifted"),
            Self::KernelMismatch => write!(f, "kernel identity drifted"),
            Self::DependencyMismatch => write!(f, "dependency fingerprints drifted"),
            Self::ObligationAbsent => write!(f, "obligation is absent from this stage"),
        }
    }
}

/// Data-level validation before (and independent of) the MNCS reuse check:
/// the relationship must be consumable on its face and must name exactly
/// the obligation, kernel, and dependency fingerprints presented now.
/// Passes nothing by itself; the MNCS `binding_reusable` execution in the
/// compiler layer has the final word.
pub fn validate_relationship_for_use(
    relationship: &ProofRelationship,
    obligation: &str,
    kernel: &str,
    dependencies: &[String],
) -> Result<(), TransportMismatch> {
    if !relationship.consumable() {
        return Err(TransportMismatch::NotConsumable);
    }
    if relationship.obligation != obligation {
        return Err(TransportMismatch::ObligationMismatch);
    }
    if relationship.kernel != kernel {
        return Err(TransportMismatch::KernelMismatch);
    }
    if relationship.dependencies != dependencies {
        return Err(TransportMismatch::DependencyMismatch);
    }
    Ok(())
}

fn obligation_present(obligations: &[crate::ObligationRecord], obligation: &str) -> bool {
    obligations
        .iter()
        .any(|record| record.identity.0 == obligation)
}

impl crate::HighLevelIr {
    /// Attach an admitted relationship to HIR after validating it against
    /// the current HIR state: the obligation must be a live HIR obligation
    /// and the sealed slot fingerprints must equal the presented ones. Any
    /// drift refuses attachment (fail-closed); attachment never repairs.
    /// The content fingerprint is recomputed over the attached state so the
    /// HIR integrity invariant (`integrity_is_valid`) keeps holding.
    pub fn attach_proof_relationship(
        &mut self,
        relationship: ProofRelationship,
        dependencies: &[String],
    ) -> Result<(), TransportMismatch> {
        if !obligation_present(&self.obligations, &relationship.obligation) {
            return Err(TransportMismatch::ObligationAbsent);
        }
        validate_relationship_for_use(
            &relationship,
            &relationship.obligation.clone(),
            &relationship.kernel.clone(),
            dependencies,
        )?;
        if let Some(slot) = self
            .proof_relationships
            .iter_mut()
            .find(|current| current.proof_identity == relationship.proof_identity)
        {
            *slot = relationship;
        } else {
            self.proof_relationships.push(relationship);
        }
        self.content_fingerprint = self
            .content_binding()
            .expect("attached HIR is canonicalizable");
        Ok(())
    }
}

/// Outcome of recording proof-bearing lowering evidence. `recorded` is true
/// only when the relationship, the MNCS reuse verdict, and the current
/// fingerprints all agree; otherwise the record documents the withholding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofEvidenceOutcome {
    pub recorded: bool,
    pub record: TransformationRecord,
    pub reason: String,
}

fn proof_evidence_target() -> crate::TargetIdentity {
    crate::TargetIdentity {
        identity: crate::SemanticId("mncs:0.4:target:portable".to_owned()),
        family: "portable".to_owned(),
        features: Vec::new(),
        integer_widths: vec![8, 16, 32, 64, 128],
        abi: None,
    }
}

impl crate::SsaModule {
    /// Attach an admitted relationship to SSA after validating it against
    /// the current SSA state (same fail-closed discipline as HIR).
    pub fn attach_proof_relationship(
        &mut self,
        relationship: ProofRelationship,
        dependencies: &[String],
    ) -> Result<(), TransportMismatch> {
        if !obligation_present(&self.obligations, &relationship.obligation) {
            return Err(TransportMismatch::ObligationAbsent);
        }
        validate_relationship_for_use(
            &relationship,
            &relationship.obligation.clone(),
            &relationship.kernel.clone(),
            dependencies,
        )?;
        if let Some(slot) = self
            .proof_relationships
            .iter_mut()
            .find(|current| current.proof_identity == relationship.proof_identity)
        {
            *slot = relationship;
        } else {
            self.proof_relationships.push(relationship);
        }
        Ok(())
    }

    /// Record proof-bearing lowering evidence for one attached relationship.
    ///
    /// `permitted_by_mncs` is the transported MNCS `binding_reusable` verdict
    /// for the exact current fingerprints — never a Rust decision. This
    /// function can only downgrade it: the record is written only when the
    /// attached relationship is consumable on its face, names the same
    /// obligation and fingerprints, AND the MNCS verdict permits. Any
    /// mismatch withholds the evidence and records the refusal instead.
    /// No backend promise is permitted or withheld here: the record carries
    /// the proof identity as consumed formal evidence, nothing more.
    pub fn record_proof_evidence(
        &mut self,
        proof_identity: &str,
        operation: &crate::SemanticId,
        obligation: &str,
        dependencies: &[String],
        permitted_by_mncs: bool,
    ) -> ProofEvidenceOutcome {
        let input_identity = self.identity.clone();
        let mut record = TransformationRecord::new(
            input_identity,
            self.identity.clone(),
            PROOF_EVIDENCE_RULE.to_owned(),
            operation.clone(),
            proof_evidence_target(),
            crate::SemanticId("mncs:0.4:transformation:proof-bearing-evidence".to_owned()),
        );
        record
            .obligations_required
            .push(crate::SemanticId(obligation.to_owned()));
        let attached = self
            .proof_relationships
            .iter()
            .find(|relationship| relationship.proof_identity == proof_identity);
        let Some(relationship) = attached else {
            record
                .invalidated_evidence
                .push(crate::SemanticId(proof_identity.to_owned()));
            let outcome = ProofEvidenceOutcome {
                recorded: false,
                record: record.clone(),
                reason: "no such proof relationship is attached to this SSA".to_owned(),
            };
            self.transformations.push(record);
            return outcome;
        };
        if validate_relationship_for_use(
            relationship,
            obligation,
            &relationship.kernel.clone(),
            dependencies,
        )
        .is_err()
        {
            record
                .invalidated_evidence
                .push(crate::SemanticId(proof_identity.to_owned()));
            let outcome = ProofEvidenceOutcome {
                recorded: false,
                record: record.clone(),
                reason: "proof relationship drifted from the current use; evidence withheld"
                    .to_owned(),
            };
            self.transformations.push(record);
            return outcome;
        }
        if !permitted_by_mncs {
            record
                .invalidated_evidence
                .push(crate::SemanticId(proof_identity.to_owned()));
            let outcome = ProofEvidenceOutcome {
                recorded: false,
                record: record.clone(),
                reason: "MNCS reuse check refused; evidence withheld".to_owned(),
            };
            self.transformations.push(record);
            return outcome;
        }
        record
            .evidence_consumed
            .push(crate::SemanticId(proof_identity.to_owned()));
        let outcome = ProofEvidenceOutcome {
            recorded: true,
            record: record.clone(),
            reason: format!(
                "MNCS-admitted proof {proof_identity} corroborated and current; evidence recorded"
            ),
        };
        self.transformations.push(record);
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_relationship() -> ProofRelationship {
        ProofRelationship {
            schema_version: PROOF_RELATIONSHIP_SCHEMA_VERSION.to_owned(),
            proof_identity: "mncs:proof-dep:test".to_owned(),
            obligation: "mncs:obligation:test".to_owned(),
            kernel: "mncs:proof-kernel:0.2".to_owned(),
            assumptions: DepAssumptionSet {
                uses: Vec::new(),
                valid: true,
            },
            mncs_verdict: DepVerdict::Pass,
            sealed_binding: ExecutionValue::Sequence {
                values: Vec::new().into(),
            },
            admission: ProofAdmissionEvidence {
                program_fingerprint: "test".to_owned(),
                seal_steps: 1,
            },
            corroboration: DepCorroboration::AgreePass,
            dependencies: vec!["aa".to_owned(), "bb".to_owned()],
        }
    }

    #[test]
    fn consumable_requires_every_leg() {
        let base = synthetic_relationship();
        assert!(base.consumable());
        let mut stale_schema = base.clone();
        stale_schema.schema_version = "0.0".to_owned();
        assert!(!stale_schema.consumable());
        let mut disputed = base.clone();
        disputed.corroboration = DepCorroboration::Disagree {
            mncs: DepVerdict::Pass,
            rust: DepVerdict::Fail,
        };
        assert!(!disputed.consumable());
        let mut unavailable = base.clone();
        unavailable.corroboration = DepCorroboration::CheckerUnavailable;
        assert!(!unavailable.consumable());
        let mut non_pass = base.clone();
        non_pass.mncs_verdict = DepVerdict::Unknown;
        non_pass.corroboration = DepCorroboration::AgreeNonPass {
            verdict: DepVerdict::Unknown,
        };
        assert!(!non_pass.consumable());
        let mut invalid_set = base.clone();
        invalid_set.assumptions.valid = false;
        assert!(!invalid_set.consumable());
    }

    fn ssa_with_relationships(relationships: Vec<ProofRelationship>) -> crate::SsaModule {
        crate::SsaModule {
            schema_version: crate::SSA_SCHEMA_VERSION.to_owned(),
            identity: crate::SemanticId("mncs:test:ssa".to_owned()),
            semantic_identity: crate::SemanticId("mncs:test:semantic".to_owned()),
            hir_fingerprint: "00".repeat(32),
            binding_table: None,
            record_types: Vec::new(),
            functions: Vec::new(),
            obligations: Vec::new(),
            trace: crate::SsaTraceMap {
                entries: Vec::new(),
            },
            transformations: Vec::new(),
            generic_specializations: Vec::new(),
            proof_relationships: relationships,
        }
    }

    #[test]
    fn binding_refs_are_sorted_deduped_and_ssa_bound() {
        let mut second = synthetic_relationship();
        second.proof_identity = "mncs:proof-dep:b".to_owned();
        let mut first = synthetic_relationship();
        first.proof_identity = "mncs:proof-dep:a".to_owned();
        // Out of order with a duplicate: the refs must canonicalize.
        let ssa = ssa_with_relationships(vec![second.clone(), first.clone(), second.clone()]);
        let refs = ssa.proof_binding_refs();
        assert_eq!(refs.len(), 2, "duplicate refs collapse");
        assert_eq!(refs[0].proof_identity, "mncs:proof-dep:a");
        assert_eq!(refs[1].proof_identity, "mncs:proof-dep:b");
        let fingerprint = ssa.fingerprint().expect("SSA is serializable");
        for reference in &refs {
            assert_eq!(reference.schema_version, PROOF_RELATIONSHIP_SCHEMA_VERSION);
            assert_eq!(reference.ssa_fingerprint, fingerprint);
            assert_eq!(reference.obligation, "mncs:obligation:test");
            assert_eq!(reference.kernel, "mncs:proof-kernel:0.2");
        }
        // Proof-free SSA yields no refs, so proof-free artifacts are untouched.
        assert!(ssa_with_relationships(Vec::new())
            .proof_binding_refs()
            .is_empty());
    }

    #[test]
    fn validation_rejects_each_drift_leg() {
        let base = synthetic_relationship();
        let deps = vec!["aa".to_owned(), "bb".to_owned()];
        assert!(validate_relationship_for_use(
            &base,
            "mncs:obligation:test",
            "mncs:proof-kernel:0.2",
            &deps
        )
        .is_ok());
        assert_eq!(
            validate_relationship_for_use(
                &base,
                "mncs:obligation:other",
                "mncs:proof-kernel:0.2",
                &deps
            ),
            Err(TransportMismatch::ObligationMismatch)
        );
        assert_eq!(
            validate_relationship_for_use(
                &base,
                "mncs:obligation:test",
                "mncs:proof-kernel:9.9",
                &deps
            ),
            Err(TransportMismatch::KernelMismatch)
        );
        assert_eq!(
            validate_relationship_for_use(
                &base,
                "mncs:obligation:test",
                "mncs:proof-kernel:0.2",
                &["aa".to_owned()]
            ),
            Err(TransportMismatch::DependencyMismatch)
        );
        let mut disputed = base.clone();
        disputed.corroboration = DepCorroboration::CheckerUnavailable;
        assert_eq!(
            validate_relationship_for_use(
                &disputed,
                "mncs:obligation:test",
                "mncs:proof-kernel:0.2",
                &deps
            ),
            Err(TransportMismatch::NotConsumable)
        );
    }
}
