//! Canonical cross-subsystem execution-authority contract.
//!
//! Atlas decides admission and capability status; Harness enforces carried
//! authority; Fabric places within it. This module owns the portable,
//! machine-native part of that chain: the decision/requirement
//! representation, the content digests that make tampering detectable, the
//! deterministic verdict fold over duplicate decisions, and the leg
//! acceptance rules (target and artifact binding included).
//!
//! Host orchestration (tool-to-capability mapping, Fabric placement checks,
//! session management) stays in the hosts. The pure semantics here are the
//! reference those hosts project; golden vectors below pin the exact bytes
//! so independent implementations (Rust, Python) agree.
//!
//! Verdict lattice: GRANTED > UNKNOWN > REFUSED. UNKNOWN is never promoted.
//! Conflicts fail closed and order-independently: any denial refuses,
//! otherwise any unsatisfied condition holds the leg at UNKNOWN.
//!
//! Digest algorithm ``sha256:canonical-json-v1``: UTF-8 JSON, object keys
//! sorted by code point, compact separators, computed over the mapping
//! *without* the ``decision_digest`` field. Byte-identical between
//! ``serde_json`` and Python ``json.dumps(sort_keys=True,
//! separators=(",", ":"), ensure_ascii=False)`` for null/boolean/integer/
//! string/array/object values. Floating-point values are outside the
//! contract and must not appear in digested envelopes.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;

use crate::canonical::sha256_hex;

/// Language-owned execution-requirement schema. Version 0.2 supersedes the
/// Harness-local 0.1 draft with digest verification, session-context
/// binding, order-independent duplicate folding, and declared-target
/// binding.
pub const REQUIREMENT_SCHEMA: &str = "mncs.execution-requirement/0.2";
/// Atlas capability-decision envelope this contract binds.
pub const DECISION_SCHEMA: &str = "mncs.atlas-capability-decision/1";
/// Content-digest algorithm binding a decision to its issued bytes.
pub const DECISION_DIGEST_ALG: &str = "sha256:canonical-json-v1";

/// Machine verdict for one capability or leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthorityVerdict {
    Granted,
    Unknown,
    Refused,
}

impl AuthorityVerdict {
    /// Conservative lattice fold: the worse verdict wins, so folding can
    /// never promote UNKNOWN to GRANTED or mask REFUSED.
    pub fn fold(self, other: AuthorityVerdict) -> AuthorityVerdict {
        use AuthorityVerdict::{Granted, Refused, Unknown};
        match (self, other) {
            (Refused, _) | (_, Refused) => Refused,
            (Unknown, _) | (_, Unknown) => Unknown,
            (Granted, Granted) => Granted,
        }
    }
}

/// Atlas-issued per-capability status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Granted,
    Conditional,
    Denied,
}

/// Admitted context a decision was issued for. Opaque strings: Atlas owns
/// the participant/scope namespaces; this contract only checks equality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBinding {
    pub participant: String,
    pub scope: String,
}

/// One Atlas capability decision as consumed for enforcement. Unknown
/// envelope fields are ignored on parse but still covered by the digest,
/// which is always verified against the raw envelope first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityDecision {
    pub capability: String,
    #[serde(default)]
    pub status: Option<DecisionStatus>,
    #[serde(default)]
    pub missing: Vec<String>,
    #[serde(default)]
    pub session: Option<SessionBinding>,
    #[serde(default)]
    pub execution_target: String,
    #[serde(default)]
    pub authority: String,
    #[serde(default)]
    pub decided_by: Vec<String>,
}

/// One execution leg: what it needs and where it will run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementLeg {
    pub name: String,
    #[serde(default)]
    pub needs: Vec<String>,
    #[serde(default)]
    pub backend: String,
    /// Declared execution target, checked against Atlas-authorized targets.
    #[serde(default)]
    pub target: String,
    /// Names of conditional evidence the leg already carries.
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// Machine-readable acceptance of one leg.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acceptance {
    pub verdict: AuthorityVerdict,
    pub leg: String,
    pub reason: String,
    #[serde(default)]
    pub binding: String,
    /// Outstanding conditional evidence union (capability folds only).
    #[serde(default)]
    pub outstanding: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthorityError {
    #[error("decision envelope is not a JSON object")]
    NotAnObject,
    #[error("decision digest algorithm {0:?} is not sha256:canonical-json-v1")]
    UnknownAlgorithm(String),
    #[error("decision carries no digest")]
    MissingDigest,
    #[error("decision digest mismatch: envelope was modified after issuance")]
    DigestMismatch,
    #[error("no atlas decision carried for capability {0:?}")]
    NoDecision(String),
    #[error("session context mismatch: {0}")]
    SessionMismatch(String),
    #[error("requirement carries no atlas decisions")]
    EmptyDecisions,
}

/// Canonical bytes of a JSON object value excluding ``decision_digest``.
pub fn canonical_envelope_bytes(value: &JsonValue) -> Result<String, AuthorityError> {
    let object = value.as_object().ok_or(AuthorityError::NotAnObject)?;
    let mut body = object.clone();
    body.remove("decision_digest");
    serde_json::to_string(&JsonValue::Object(body)).map_err(|_| AuthorityError::NotAnObject)
}

/// Recompute and check a decision digest against its envelope.
pub fn verify_decision_digest(envelope: &JsonValue) -> Result<(), AuthorityError> {
    let object = envelope.as_object().ok_or(AuthorityError::NotAnObject)?;
    let alg = object
        .get("decision_digest_alg")
        .and_then(JsonValue::as_str)
        .unwrap_or("");
    if alg != DECISION_DIGEST_ALG {
        return Err(AuthorityError::UnknownAlgorithm(alg.to_owned()));
    }
    let digest = object
        .get("decision_digest")
        .and_then(JsonValue::as_str)
        .ok_or(AuthorityError::MissingDigest)?;
    let canonical = canonical_envelope_bytes(envelope)?;
    if sha256_hex(canonical.as_bytes()) != digest {
        return Err(AuthorityError::DigestMismatch);
    }
    Ok(())
}

/// Fold all decisions for one capability into a single verdict.
/// Order-independent by construction: denial anywhere refuses, otherwise
/// any unsatisfied condition holds at unknown with the sorted union of
/// outstanding evidence, otherwise granted.
pub fn fold_capability(capability: &str, decisions: &[CapabilityDecision]) -> Acceptance {
    if decisions.is_empty() {
        return Acceptance {
            verdict: AuthorityVerdict::Refused,
            leg: capability.to_owned(),
            reason: format!("{capability}: no atlas decision carried (bypass?)"),
            binding: String::new(),
            outstanding: Vec::new(),
        };
    }
    let mut verdict = AuthorityVerdict::Granted;
    let mut outstanding = BTreeSet::new();
    let mut reasons = Vec::new();
    let mut binding = String::new();
    for decision in decisions {
        let session = decision
            .session
            .as_ref()
            .map(|session| {
                format!(
                    " participant={} scope={}",
                    session.participant, session.scope
                )
            })
            .unwrap_or_default();
        if binding.is_empty() {
            binding = format!("{}={:?}{session}", decision.capability, decision.status);
        }
        match decision.status {
            None | Some(DecisionStatus::Denied) => {
                verdict = verdict.fold(AuthorityVerdict::Refused);
                reasons.push(format!("{}: atlas denied", decision.capability));
            }
            Some(DecisionStatus::Conditional) => {
                let missing: Vec<String> = {
                    let mut items = decision.missing.clone();
                    items.sort();
                    items
                };
                if missing.is_empty() {
                    reasons.push(format!(
                        "{}: conditional with no outstanding evidence",
                        decision.capability
                    ));
                } else {
                    verdict = verdict.fold(AuthorityVerdict::Unknown);
                    outstanding.extend(missing.clone());
                    reasons.push(format!(
                        "{}: conditional, outstanding {missing:?}",
                        decision.capability
                    ));
                }
            }
            Some(DecisionStatus::Granted) => {
                reasons.push(format!("{}: atlas granted", decision.capability));
            }
        }
    }
    let outstanding: Vec<String> = outstanding.into_iter().collect();
    if verdict == AuthorityVerdict::Unknown {
        reasons.push(format!("outstanding union: {outstanding:?}"));
    }
    Acceptance {
        verdict,
        leg: capability.to_owned(),
        reason: reasons.join("; "),
        binding,
        outstanding,
    }
}

/// Require every carried decision to belong to one admitted context.
/// A decision without a session binding, or bound to another
/// participant/scope, cannot authorize this requirement: replay
/// out-of-context fails closed here, before any leg is accepted.
pub fn verify_session_context(
    decisions: &[CapabilityDecision],
    participant: &str,
    scope: &str,
) -> Result<(), AuthorityError> {
    if decisions.is_empty() {
        return Err(AuthorityError::EmptyDecisions);
    }
    for decision in decisions {
        match &decision.session {
            None => {
                return Err(AuthorityError::SessionMismatch(format!(
                    "{}: decision carries no session binding",
                    decision.capability
                )));
            }
            Some(session) if session.participant != participant || session.scope != scope => {
                return Err(AuthorityError::SessionMismatch(format!(
                    "{}: bound to participant={} scope={}",
                    decision.capability, session.participant, session.scope
                )));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Accept one leg against folded per-capability verdicts.
///
/// * `observed_artifact` differing from `bound_artifact` refuses (tamper).
/// * A needed capability with no fold refuses (bypass attempt).
/// * Target binding: the distinct non-empty Atlas-authorized targets for a
///   need must agree with each other (conflicting authority refuses) and
///   with the leg's declared target. A leg that declares no target while
///   Atlas constrains one stays UNKNOWN; a declared target Atlas never
///   authorized refuses. `execution.target` conditional evidence counts as
///   supplied only when the leg's target is bound to an authorized one.
/// * Conditional `missing` otherwise counts as supplied only from carried
///   leg evidence names.
pub fn accept_leg(
    leg: &RequirementLeg,
    folds: &BTreeMap<String, Acceptance>,
    bound_artifact: &str,
    observed_artifact: &str,
    authorized_targets: &BTreeMap<String, Vec<String>>,
) -> Acceptance {
    if !observed_artifact.is_empty() && observed_artifact != bound_artifact {
        return Acceptance {
            verdict: AuthorityVerdict::Refused,
            leg: leg.name.clone(),
            reason: "artifact digest mismatch: observed differs from bound".to_owned(),
            binding: String::new(),
            outstanding: Vec::new(),
        };
    }
    let mut verdict = AuthorityVerdict::Granted;
    let mut reasons = Vec::new();
    let mut binding = String::new();
    for need in &leg.needs {
        let fold = match folds.get(need) {
            None => {
                verdict = verdict.fold(AuthorityVerdict::Refused);
                reasons.push(format!("{need}: no atlas decision carried (bypass?)"));
                continue;
            }
            Some(fold) => fold,
        };
        if binding.is_empty() {
            binding = fold.binding.clone();
        }
        // Target authority for this need, order-independent.
        let authorized: BTreeSet<&str> = authorized_targets
            .get(need)
            .map(|targets| {
                targets
                    .iter()
                    .map(String::as_str)
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let target_bound = match (authorized.len(), leg.target.is_empty()) {
            (0, _) => true,
            (1, false) if authorized.contains(leg.target.as_str()) => true,
            (1, _) => false,
            _ => false,
        };
        if !target_bound {
            verdict = verdict.fold(AuthorityVerdict::Refused);
            let authorized_list: Vec<&str> = authorized.into_iter().collect();
            reasons.push(format!(
                "{need}: target mismatch: leg declares {:?}, atlas authorizes {authorized_list:?}",
                leg.target
            ));
            continue;
        }
        let constrained = !leg.target.is_empty()
            && authorized_targets
                .get(need)
                .map(|targets| targets.iter().any(|t| !t.is_empty()))
                .unwrap_or(false);
        match fold.verdict {
            AuthorityVerdict::Refused => {
                verdict = verdict.fold(AuthorityVerdict::Refused);
                reasons.push(format!("{need}: {}", fold.reason));
            }
            AuthorityVerdict::Unknown => {
                // Re-evaluate outstanding evidence against this leg.
                let mut supplied: BTreeSet<&str> =
                    leg.evidence.iter().map(String::as_str).collect();
                if constrained {
                    supplied.insert("execution.target");
                }
                let still_outstanding = fold
                    .outstanding
                    .iter()
                    .filter(|item| !supplied.contains(item.as_str()))
                    .cloned()
                    .collect::<Vec<_>>();
                if still_outstanding.is_empty() {
                    reasons.push(format!("{need}: conditional satisfied by carried evidence"));
                } else {
                    verdict = verdict.fold(AuthorityVerdict::Unknown);
                    reasons.push(format!(
                        "{need}: conditional, outstanding {still_outstanding:?}"
                    ));
                }
            }
            AuthorityVerdict::Granted => {
                reasons.push(format!("{need}: {}", fold.reason));
            }
        }
    }
    Acceptance {
        verdict,
        leg: leg.name.clone(),
        reason: reasons.join("; "),
        binding,
        outstanding: Vec::new(),
    }
}

/// Canonical requirement identity: sha256 over the canonical body binding
/// task, source, artifact, the sorted carried decision digests, and the
/// legs (names, sorted needs, backends, targets).
pub fn requirement_identity(
    task_id: &str,
    source_identity: &str,
    artifact_identity: &str,
    decision_digests: &[String],
    legs: &[RequirementLeg],
) -> String {
    let mut digests = decision_digests.to_vec();
    digests.sort();
    let legs: Vec<JsonValue> = {
        let mut legs: Vec<&RequirementLeg> = legs.iter().collect();
        legs.sort_by(|a, b| a.name.cmp(&b.name));
        legs.into_iter()
            .map(|leg| {
                let mut needs = leg.needs.clone();
                needs.sort();
                serde_json::json!({
                    "backend": leg.backend,
                    "name": leg.name,
                    "needs": needs,
                    "target": leg.target,
                })
            })
            .collect()
    };
    let body = serde_json::json!({
        "artifact_identity": artifact_identity,
        "decision_digests": digests,
        "legs": legs,
        "schema_version": REQUIREMENT_SCHEMA,
        "source_identity": source_identity,
        "task_id": task_id,
    });
    let canonical = serde_json::to_string(&body).expect("requirement body is serializable");
    sha256_hex(canonical.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> SessionBinding {
        SessionBinding {
            participant: "e2e-agent".to_owned(),
            scope: "repo(mncs-language)".to_owned(),
        }
    }

    fn decision(capability: &str, status: DecisionStatus) -> CapabilityDecision {
        CapabilityDecision {
            capability: capability.to_owned(),
            status: Some(status),
            missing: Vec::new(),
            session: Some(session()),
            execution_target: String::new(),
            authority: "atlas".to_owned(),
            decided_by: vec!["atlas".to_owned()],
        }
    }

    #[test]
    fn verdict_fold_is_conservative() {
        use AuthorityVerdict::{Granted, Refused, Unknown};
        assert_eq!(Granted.fold(Granted), Granted);
        assert_eq!(Granted.fold(Unknown), Unknown);
        assert_eq!(Unknown.fold(Granted), Unknown);
        assert_eq!(Granted.fold(Refused), Refused);
        assert_eq!(Unknown.fold(Refused), Refused);
    }

    #[test]
    fn duplicate_fold_is_order_independent() {
        let granted = decision("tests.execute", DecisionStatus::Granted);
        let mut denied = decision("tests.execute", DecisionStatus::Denied);
        denied.missing = vec!["x".to_owned()];
        let ab = fold_capability("tests.execute", &[granted.clone(), denied.clone()]);
        let ba = fold_capability("tests.execute", &[denied, granted]);
        assert_eq!(ab.verdict, AuthorityVerdict::Refused);
        assert_eq!(ba.verdict, AuthorityVerdict::Refused);
        assert_eq!(ab.verdict, ba.verdict);
    }

    #[test]
    fn conditional_union_is_order_independent() {
        let mut first = decision("cap", DecisionStatus::Conditional);
        first.missing = vec!["b".to_owned(), "a".to_owned()];
        let mut second = decision("cap", DecisionStatus::Conditional);
        second.missing = vec!["c".to_owned()];
        let ab = fold_capability("cap", &[first.clone(), second.clone()]);
        let ba = fold_capability("cap", &[second, first]);
        assert_eq!(ab.verdict, AuthorityVerdict::Unknown);
        assert_eq!(
            ab.outstanding,
            vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]
        );
        assert_eq!(ab.outstanding, ba.outstanding);
    }

    #[test]
    fn empty_fold_refuses() {
        let fold = fold_capability("cap", &[]);
        assert_eq!(fold.verdict, AuthorityVerdict::Refused);
    }

    #[test]
    fn session_context_rejects_replay() {
        let decisions = vec![decision("cap", DecisionStatus::Granted)];
        assert!(verify_session_context(&decisions, "e2e-agent", "repo(mncs-language)").is_ok());
        assert!(verify_session_context(&decisions, "intruder", "repo(mncs-language)").is_err());
        assert!(verify_session_context(&decisions, "e2e-agent", "repo(other)").is_err());
        assert!(verify_session_context(&[], "e2e-agent", "repo(mncs-language)").is_err());
    }

    #[test]
    fn digest_verify_detects_tamper() {
        let envelope = serde_json::json!({
            "capability": "tests.execute",
            "decision_digest_alg": DECISION_DIGEST_ALG,
            "execution_target": "",
            "missing": [],
            "schema_version": DECISION_SCHEMA,
            "scope": "repo(mncs-language)",
            "session": {"participant": "e2e-agent", "scope": "repo(mncs-language)"},
            "status": "granted",
        });
        let mut issued = envelope.as_object().unwrap().clone();
        let canonical = canonical_envelope_bytes(&JsonValue::Object(issued.clone())).unwrap();
        issued.insert(
            "decision_digest".to_owned(),
            JsonValue::String(sha256_hex(canonical.as_bytes())),
        );
        assert!(verify_decision_digest(&JsonValue::Object(issued.clone())).is_ok());
        let mut forged = issued.clone();
        forged.insert("status".to_owned(), JsonValue::String("denied".to_owned()));
        // Digest still the issued one, body changed: mismatch.
        assert_eq!(
            verify_decision_digest(&JsonValue::Object(forged)),
            Err(AuthorityError::DigestMismatch)
        );
        let mut undigested = issued.clone();
        undigested.remove("decision_digest");
        assert_eq!(
            verify_decision_digest(&JsonValue::Object(undigested)),
            Err(AuthorityError::MissingDigest)
        );
    }

    #[test]
    fn target_binding_refuses_mismatch() {
        let granted = decision("worker.dispatch", DecisionStatus::Granted);
        let mut folds = BTreeMap::new();
        folds.insert(
            "worker.dispatch".to_owned(),
            fold_capability("worker.dispatch", &[granted]),
        );
        let mut targets = BTreeMap::new();
        targets.insert(
            "worker.dispatch".to_owned(),
            vec!["mncs:target:cuda-0.1".to_owned()],
        );
        let matching = RequirementLeg {
            name: "gpu".to_owned(),
            needs: vec!["worker.dispatch".to_owned()],
            backend: "ptx".to_owned(),
            target: "mncs:target:cuda-0.1".to_owned(),
            evidence: Vec::new(),
        };
        let accepted = accept_leg(&matching, &folds, "sha256:x", "", &targets);
        assert_eq!(accepted.verdict, AuthorityVerdict::Granted);
        let moved = RequirementLeg {
            target: "mncs:target:cpu-0.1".to_owned(),
            ..matching.clone()
        };
        assert_eq!(
            accept_leg(&moved, &folds, "sha256:x", "", &targets).verdict,
            AuthorityVerdict::Refused
        );
        let unbound = RequirementLeg {
            target: String::new(),
            ..matching.clone()
        };
        assert_eq!(
            accept_leg(&unbound, &folds, "sha256:x", "", &targets).verdict,
            AuthorityVerdict::Refused
        );
    }

    #[test]
    fn conflicting_authorized_targets_refuse() {
        let granted = decision("worker.dispatch", DecisionStatus::Granted);
        let mut folds = BTreeMap::new();
        folds.insert(
            "worker.dispatch".to_owned(),
            fold_capability("worker.dispatch", &[granted]),
        );
        let mut targets = BTreeMap::new();
        targets.insert(
            "worker.dispatch".to_owned(),
            vec![
                "mncs:target:cuda-0.1".to_owned(),
                "mncs:target:cpu-0.1".to_owned(),
            ],
        );
        let leg = RequirementLeg {
            name: "gpu".to_owned(),
            needs: vec!["worker.dispatch".to_owned()],
            backend: "ptx".to_owned(),
            target: "mncs:target:cuda-0.1".to_owned(),
            evidence: Vec::new(),
        };
        assert_eq!(
            accept_leg(&leg, &folds, "sha256:x", "", &targets).verdict,
            AuthorityVerdict::Refused
        );
    }

    #[test]
    fn requirement_identity_is_stable_and_order_free() {
        let legs = vec![RequirementLeg {
            name: "cpu".to_owned(),
            needs: vec!["b".to_owned(), "a".to_owned()],
            backend: String::new(),
            target: String::new(),
            evidence: Vec::new(),
        }];
        let digests = vec!["d2".to_owned(), "d1".to_owned()];
        let first = requirement_identity("t", "s", "a", &digests, &legs);
        let second =
            requirement_identity("t", "s", "a", &["d1".to_owned(), "d2".to_owned()], &legs);
        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
        // Golden vector: any independent implementation (mncs-harness
        // Python, Atlas) must reproduce this exact identity for the same
        // inputs. Breaks if the canonical form drifts.
        assert_eq!(
            requirement_identity(
                "e2e-001",
                "sha256:aa",
                "sha256:bb",
                &["dd".to_owned()],
                &legs
            ),
            "2b56d74b42c0ed29cec46f392330e9e732524980fa39ef1a76fa0612a3ab54ae",
        );
    }
}
