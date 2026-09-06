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

/// Language-owned execution-requirement schema. Version 0.3 supersedes
/// 0.2 with issuer-authenticated issuance, attested conditional evidence,
/// and a requirement identity covering evidence, bounds, and session
/// context. Unsigned 0.2 envelopes no longer load.
pub const REQUIREMENT_SCHEMA: &str = "mncs.execution-requirement/0.3";
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
    /// Carried evidence by name. A bare value never authorizes: only an
    /// issuer-authenticated attestation envelope (see
    /// [`satisfied_by_attested_evidence`]) can satisfy outstanding
    /// conditional evidence. Values participate in the requirement
    /// identity so mutation is detectable.
    #[serde(default)]
    pub evidence: BTreeMap<String, JsonValue>,
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
    #[error("envelope carries no issuer binding: content integrity is not issuance")]
    MissingIssuer,
    #[error("issuer algorithm {0:?} is not ed25519")]
    UnknownIssuerAlgorithm(String),
    #[error("issuer {0:?} is not trusted by this verifier")]
    UntrustedIssuer(String),
    #[error("issuer signature is not 64-byte hex")]
    BadSignatureEncoding,
    #[error("trusted issuer {0:?} has an invalid public key")]
    InvalidIssuerKey(String),
    #[error("envelope signature does not verify against the trusted issuer key")]
    SignatureMismatch,
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
///   Atlas constrains one refuses (authorized placement cannot be shown);
///   a declared target Atlas never authorized refuses. `execution.target`
///   supplied only when the leg's target is bound to an authorized one.
/// * Conditional `missing` otherwise counts as supplied only from
///   issuer-authenticated carried evidence (see
///   [`satisfied_by_attested_evidence`]); bare caller-written values
///   never satisfy.
pub fn accept_leg(
    leg: &RequirementLeg,
    folds: &BTreeMap<String, Acceptance>,
    bound_artifact: &str,
    observed_artifact: &str,
    authorized_targets: &BTreeMap<String, Vec<String>>,
    evidence_terms: &EvidenceTerms,
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
        // Target authority for this need, order-independent. Multiple
        // authorized targets are placement options, not conflicting
        // authority: the leg must name one of them.
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
        let target_bound = match (authorized.is_empty(), leg.target.is_empty()) {
            (true, _) => true,
            (false, true) => false,
            (false, false) => authorized.contains(leg.target.as_str()),
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
                // `execution.target` is system-derived from the verified
                // target binding above, never caller-claimed; every other
                // item needs an issuer-authenticated attestation.
                let mut supplied: BTreeSet<&str> = BTreeSet::new();
                if constrained {
                    supplied.insert("execution.target");
                }
                let still_outstanding = satisfied_by_attested_evidence(
                    &fold.outstanding,
                    &leg.evidence,
                    &supplied,
                    evidence_terms,
                );
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
/// task, source, artifact, session context, bounds, the sorted carried
/// decision digests, and the legs (names, sorted needs, backends, targets,
/// evidence pairs).
///
/// Every caller-mutable field that can alter authority or execution
/// semantics participates: mutating evidence values, bounds, or the
/// session context changes the identity. Decision-enforced content arrives
/// via the decision digests; leg evidence arrives here.
///
/// Arity is the cross-language contract (mirrored by the Harness Python
/// projection argument-for-argument), so the parameter count is
/// intentional.
// Allow: arity is the pinned cross-language contract, not accretion.
#[allow(clippy::too_many_arguments)]
pub fn requirement_identity(
    task_id: &str,
    source_identity: &str,
    artifact_identity: &str,
    decision_digests: &[String],
    legs: &[RequirementLeg],
    bounds: &BTreeMap<String, JsonValue>,
    session_participant: &str,
    session_scope: &str,
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
                    "evidence": leg.evidence,
                    "name": leg.name,
                    "needs": needs,
                    "target": leg.target,
                })
            })
            .collect()
    };
    let body = serde_json::json!({
        "artifact_identity": artifact_identity,
        "bounds": bounds,
        "decision_digests": digests,
        "legs": legs,
        "schema_version": REQUIREMENT_SCHEMA,
        "session": {
            "participant": session_participant,
            "scope": session_scope,
        },
        "source_identity": source_identity,
        "task_id": task_id,
    });
    let canonical = serde_json::to_string(&body).expect("requirement body is serializable");
    sha256_hex(canonical.as_bytes())
}

/// Issuer-authenticity contract (epi13/mncs-atlas#31).
///
/// A content digest proves integrity, not issuance: anyone holding the
/// public canonicalization can recompute sha256 over forged JSON.
/// Authentic issuance binds the exact issued bytes to an issuer key with
/// Ed25519. This module owns the semantic contract only: envelope shape,
/// canonical signed bytes, trusted-key binding checks, and verification
/// outcomes. Cryptographic signature verification itself is a host
/// mechanism (mncs-cli, Harness verifiers); this crate stays free of
/// crypto implementations so the contract, not any one library, is what
/// independent implementations agree on.
///
/// Envelope shape (additive over `mncs.atlas-capability-decision/1`):
/// `issuer` carries `{key_id, algorithm}`, `issuer_signature` carries the
/// hex Ed25519 signature over [`issuance_signed_bytes`]. The same shape
/// authenticates evidence attestations ([`EVIDENCE_SCHEMA`]), which carry
/// no `decision_digest`.
pub const ISSUANCE_SIGNATURE_ALG: &str = "ed25519";
/// Authenticated evidence attestation envelope (see [`check_attestation`]).
pub const EVIDENCE_SCHEMA: &str = "mncs.evidence-attestation/1";

/// Issuer named by an issued envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerBinding {
    pub key_id: String,
    pub algorithm: String,
}

/// Issuance material extracted and binding-checked from an envelope,
/// ready for host-side cryptographic verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedIssuance {
    pub key_id: String,
    /// Exact bytes the issuer signed ([`issuance_signed_bytes`]).
    pub signed_bytes: Vec<u8>,
    /// Raw 64-byte Ed25519 signature.
    pub signature: Vec<u8>,
}

/// Terms under which carried evidence may satisfy outstanding conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceTerms {
    /// Session the outstanding evidence was issued for.
    pub participant: String,
    pub scope: String,
    /// Key ids the verifier trusts for evidence attestation.
    pub trusted_key_ids: BTreeSet<String>,
    /// Verifier clock, seconds since Unix epoch.
    pub now_secs: u64,
    /// Maximum attestation age in seconds; older never promotes.
    pub max_age_secs: u64,
}

/// Why carried evidence does or does not satisfy one outstanding item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceStanding {
    /// System-derived (e.g. target binding), not caller-claimed.
    SystemSupplied,
    /// Issuer-authenticated attestation covering name, session, scope,
    /// subject, and freshness.
    Attested,
    /// No entry, bare value, or attestation that fails a check.
    NotSupplied,
}

/// Canonical bytes the issuer signs: the envelope minus
/// `decision_digest` and `issuer_signature`. The issuer block stays
/// covered, binding the signing key to the payload.
pub fn issuance_signed_bytes(value: &JsonValue) -> Result<Vec<u8>, AuthorityError> {
    let object = value.as_object().ok_or(AuthorityError::NotAnObject)?;
    let mut body = object.clone();
    body.remove("decision_digest");
    body.remove("issuer_signature");
    let canonical =
        serde_json::to_string(&JsonValue::Object(body)).map_err(|_| AuthorityError::NotAnObject)?;
    Ok(canonical.into_bytes())
}

fn decode_hex_signature(text: &str) -> Result<Vec<u8>, AuthorityError> {
    if text.len() != 128 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AuthorityError::BadSignatureEncoding);
    }
    let nibble = |byte: u8| match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    };
    Ok(text
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| nibble(pair[0]) << 4 | nibble(pair[1]))
        .collect())
}

/// Check the issuance binding of an envelope without performing
/// cryptography: shape, algorithm, trusted key id, signature encoding.
/// Returns the material for host-side Ed25519 verification. Fails closed
/// on anything unsigned, mislabeled, untrusted, or malformed: a forged
/// envelope with a valid content digest still fails here for lack of
/// authentic issuance.
pub fn check_issuance_binding(
    envelope: &JsonValue,
    trusted_key_ids: &BTreeSet<String>,
) -> Result<ParsedIssuance, AuthorityError> {
    let object = envelope.as_object().ok_or(AuthorityError::NotAnObject)?;
    let issuer = object
        .get("issuer")
        .and_then(JsonValue::as_object)
        .ok_or(AuthorityError::MissingIssuer)?;
    let key_id = issuer
        .get("key_id")
        .and_then(JsonValue::as_str)
        .unwrap_or("");
    if key_id.is_empty() {
        return Err(AuthorityError::MissingIssuer);
    }
    let algorithm = issuer
        .get("algorithm")
        .and_then(JsonValue::as_str)
        .unwrap_or("");
    if algorithm != ISSUANCE_SIGNATURE_ALG {
        return Err(AuthorityError::UnknownIssuerAlgorithm(algorithm.to_owned()));
    }
    if !trusted_key_ids.contains(key_id) {
        return Err(AuthorityError::UntrustedIssuer(key_id.to_owned()));
    }
    let signature_text = object
        .get("issuer_signature")
        .and_then(JsonValue::as_str)
        .unwrap_or("");
    let signature = decode_hex_signature(signature_text)?;
    Ok(ParsedIssuance {
        key_id: key_id.to_owned(),
        signed_bytes: issuance_signed_bytes(envelope)?,
        signature,
    })
}

/// Standing of one carried evidence value for one outstanding item.
///
/// `system_supplied` covers names the verifier derives itself (never
/// caller claims). Otherwise the value must be an attestation envelope of
/// schema [`EVIDENCE_SCHEMA`] whose issuer binding checks out against the
/// trusted ids and whose name, session, scope, subject, and freshness all
/// match the terms. Anything else — absent entry, bare string, wrong
/// subject, stale or future-dated, untrusted issuer — does not supply.
pub fn evidence_standing(
    name: &str,
    value: Option<&JsonValue>,
    system_supplied: bool,
    subject: &str,
    terms: &EvidenceTerms,
) -> EvidenceStanding {
    use EvidenceStanding::{Attested, NotSupplied, SystemSupplied};
    if system_supplied {
        return SystemSupplied;
    }
    let attestation = match value {
        Some(JsonValue::Object(map)) => map,
        _ => return NotSupplied,
    };
    if attestation
        .get("schema_version")
        .and_then(JsonValue::as_str)
        != Some(EVIDENCE_SCHEMA)
    {
        return NotSupplied;
    }
    if attestation.get("name").and_then(JsonValue::as_str) != Some(name) {
        return NotSupplied;
    }
    if attestation.get("participant").and_then(JsonValue::as_str)
        != Some(terms.participant.as_str())
    {
        return NotSupplied;
    }
    if attestation.get("scope").and_then(JsonValue::as_str) != Some(terms.scope.as_str()) {
        return NotSupplied;
    }
    if let Some(bound) = attestation.get("subject").and_then(JsonValue::as_str) {
        if !bound.is_empty() && bound != subject {
            return NotSupplied;
        }
    }
    let issued_at = attestation.get("issued_at").and_then(JsonValue::as_u64);
    match issued_at {
        Some(when) if when <= terms.now_secs && terms.now_secs - when <= terms.max_age_secs => {}
        _ => return NotSupplied,
    }
    if check_issuance_binding(
        &JsonValue::Object(attestation.clone()),
        &terms.trusted_key_ids,
    )
    .is_err()
    {
        return NotSupplied;
    }
    Attested
}

/// Filter outstanding conditional evidence through carried values:
/// returns what remains unsupplied. System-supplied names bypass
/// attestation (the caller supplies them in `system_supplied` only when
/// the verifier derived them itself, e.g. a bound execution target).
pub fn satisfied_by_attested_evidence(
    outstanding: &[String],
    carried: &BTreeMap<String, JsonValue>,
    system_supplied: &BTreeSet<&str>,
    terms: &EvidenceTerms,
) -> Vec<String> {
    outstanding
        .iter()
        .filter(|item| {
            !matches!(
                evidence_standing(
                    item,
                    carried.get(item.as_str()),
                    system_supplied.contains(item.as_str()),
                    "",
                    terms,
                ),
                EvidenceStanding::SystemSupplied | EvidenceStanding::Attested
            )
        })
        .cloned()
        .collect()
}

/// Proof channel for the canonical confirm: what kind of observation the
/// actual target arrived on. Hosts map their own provenance classes onto
/// these; only operator channels can ever grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofClass {
    /// Fresh Fabric-inventory observation (worker-observed or
    /// operator-asserted class).
    Operator,
    /// Explicitly consumer-declared observation: never authority.
    ConsumerDeclared,
    /// Implementation predates observation provenance: consistent but
    /// unprovable, never granted.
    Legacy,
    /// No provenance information at all.
    Unknown,
}

/// Canonical execution confirmation (epi13/mncs-language#117).
///
/// Precondition: the leg already accepted GRANTED. Compares the observed
/// actual target against the authorized declared target on the proof
/// channel: a positive mismatch refuses even on legacy channels; a match
/// grants only on fresh operator proof; anything unproven stays UNKNOWN.
/// Hosts acquire actual target, proof class, and freshness; this function
/// decides.
pub fn confirm_execution(
    authorized_target: &str,
    actual_target: &str,
    proof: ProofClass,
    fresh: bool,
) -> Acceptance {
    let refused = |reason: String| Acceptance {
        verdict: AuthorityVerdict::Refused,
        leg: String::new(),
        reason,
        binding: String::new(),
        outstanding: Vec::new(),
    };
    let unknown = |reason: String| Acceptance {
        verdict: AuthorityVerdict::Unknown,
        leg: String::new(),
        reason,
        binding: String::new(),
        outstanding: Vec::new(),
    };
    if proof == ProofClass::Legacy {
        if !actual_target.is_empty()
            && !authorized_target.is_empty()
            && actual_target != authorized_target
        {
            return refused(format!(
                "target mismatch: authorized {authorized_target}, actual {actual_target}"
            ));
        }
        return unknown(format!(
            "actual target {} consistent with authorized {} but unproven: \
             implementation predates observation provenance",
            if actual_target.is_empty() {
                "unreported"
            } else {
                actual_target
            },
            if authorized_target.is_empty() {
                "unbound"
            } else {
                authorized_target
            },
        ));
    }
    if actual_target.is_empty() {
        return refused("no actual target evidence".to_owned());
    }
    if proof != ProofClass::Operator {
        return refused(format!("proof channel {proof:?} is not operator authority"));
    }
    if !fresh {
        return unknown(format!("stale target evidence for {actual_target}"));
    }
    if authorized_target.is_empty() {
        return unknown(format!(
            "target unbound end-to-end: actual {actual_target} has no declared target"
        ));
    }
    if actual_target != authorized_target {
        return refused(format!(
            "target mismatch: authorized {authorized_target}, actual {actual_target}"
        ));
    }
    Acceptance {
        verdict: AuthorityVerdict::Granted,
        leg: String::new(),
        reason: format!("actual target {actual_target} matches authorized {authorized_target}"),
        binding: String::new(),
        outstanding: Vec::new(),
    }
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
            evidence: BTreeMap::new(),
        };
        let accepted = accept_leg(
            &matching,
            &folds,
            "sha256:x",
            "",
            &targets,
            &evidence_terms(),
        );
        assert_eq!(accepted.verdict, AuthorityVerdict::Granted);
        let moved = RequirementLeg {
            target: "mncs:target:cpu-0.1".to_owned(),
            ..matching.clone()
        };
        assert_eq!(
            accept_leg(&moved, &folds, "sha256:x", "", &targets, &evidence_terms()).verdict,
            AuthorityVerdict::Refused
        );
        let unbound = RequirementLeg {
            target: String::new(),
            ..matching.clone()
        };
        assert_eq!(
            accept_leg(
                &unbound,
                &folds,
                "sha256:x",
                "",
                &targets,
                &evidence_terms()
            )
            .verdict,
            AuthorityVerdict::Refused
        );
    }

    #[test]
    fn multiple_authorized_targets_are_options() {
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
        let either = RequirementLeg {
            name: "gpu".to_owned(),
            needs: vec!["worker.dispatch".to_owned()],
            backend: "ptx".to_owned(),
            target: "mncs:target:cuda-0.1".to_owned(),
            evidence: BTreeMap::new(),
        };
        assert_eq!(
            accept_leg(&either, &folds, "sha256:x", "", &targets, &evidence_terms()).verdict,
            AuthorityVerdict::Granted
        );
        let other = RequirementLeg {
            target: "mncs:target:cpu-0.1".to_owned(),
            ..either.clone()
        };
        assert_eq!(
            accept_leg(&other, &folds, "sha256:x", "", &targets, &evidence_terms()).verdict,
            AuthorityVerdict::Granted
        );
        let outside = RequirementLeg {
            target: "mncs:target:tpu-0.1".to_owned(),
            ..either.clone()
        };
        assert_eq!(
            accept_leg(
                &outside,
                &folds,
                "sha256:x",
                "",
                &targets,
                &evidence_terms()
            )
            .verdict,
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
            evidence: BTreeMap::new(),
        }];
        let bounds: BTreeMap<String, JsonValue> = BTreeMap::new();
        let digests = vec!["d2".to_owned(), "d1".to_owned()];
        let first = requirement_identity("t", "s", "a", &digests, &legs, &bounds, "p", "q");
        let second = requirement_identity(
            "t",
            "s",
            "a",
            &["d1".to_owned(), "d2".to_owned()],
            &legs,
            &bounds,
            "p",
            "q",
        );
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
                &legs,
                &bounds,
                "e2e-agent",
                "repo(mncs-language)",
            ),
            "01af193b5208e4b69747f56c33d47087b027b2e502aebaf3a9e969c3c2a61ed6",
        );
    }

    #[test]
    fn requirement_identity_binds_evidence_bounds_session() {
        let legs = vec![RequirementLeg {
            name: "cpu".to_owned(),
            needs: vec!["a".to_owned()],
            backend: String::new(),
            target: String::new(),
            evidence: BTreeMap::new(),
        }];
        let bounds: BTreeMap<String, JsonValue> = BTreeMap::new();
        let base = requirement_identity("t", "s", "a", &[], &legs, &bounds, "p", "q");
        let mut keyed = BTreeMap::new();
        keyed.insert("x".to_owned(), JsonValue::String("y".to_owned()));
        let changed_evidence = vec![RequirementLeg {
            evidence: keyed,
            ..legs[0].clone()
        }];
        assert_ne!(
            base,
            requirement_identity("t", "s", "a", &[], &changed_evidence, &bounds, "p", "q")
        );
        let mut bounded = BTreeMap::new();
        bounded.insert("max_seconds".to_owned(), serde_json::json!(1));
        assert_ne!(
            base,
            requirement_identity("t", "s", "a", &[], &legs, &bounded, "p", "q")
        );
        assert_ne!(
            base,
            requirement_identity("t", "s", "a", &[], &legs, &bounds, "mallory", "q")
        );
        assert_ne!(
            base,
            requirement_identity("t", "s", "a", &[], &legs, &bounds, "p", "repo(other)")
        );
    }

    fn evidence_terms() -> EvidenceTerms {
        EvidenceTerms {
            participant: "e2e-agent".to_owned(),
            scope: "repo(mncs-language)".to_owned(),
            trusted_key_ids: BTreeSet::from(["test-key-1".to_owned()]),
            now_secs: 1_700_000_100,
            max_age_secs: 86_400,
        }
    }

    const TEST_ISSUER_KEY: &str = "test-key-1";
    const V1_SIGNED_BYTES: &str = "{\"authority\":\"fabric\",\"capability\":\"tests.execute\",\"conformant_path\":[],\"decision_by\":[\"fabric\"],\"decision_digest_alg\":\"sha256:canonical-json-v1\",\"evidence_required\":[],\"execution_target\":\"\",\"issuer\":{\"algorithm\":\"ed25519\",\"key_id\":\"test-key-1\"},\"missing\":[],\"reason\":\"test issuance\",\"schema_version\":\"mncs.atlas-capability-decision/1\",\"scope\":\"repo(mncs-language)\",\"session\":{\"participant\":\"e2e-agent\",\"scope\":\"repo(mncs-language)\"},\"status\":\"granted\",\"verdict\":\"PASS\"}";
    const V1_SIGNATURE_HEX: &str = "bc867e8760e1df075531868a84f7dc2ff7f78c76ed6fe49342cf367d7a316143144da4801c2f121849d70653225d99a6b08cb5168eb912bcfbac610e038f9707";
    const V1_DIGEST: &str = "ef52f5d2dc98b1a7c3d462ed4ffe44135c2c8328b35c567e75bf28f8257cab9a";

    fn signed_v1_envelope() -> JsonValue {
        let mut envelope =
            serde_json::from_str::<JsonValue>(V1_SIGNED_BYTES).expect("vector parses");
        let object = envelope.as_object_mut().expect("vector is an object");
        object.insert(
            "decision_digest".to_owned(),
            JsonValue::String(V1_DIGEST.to_owned()),
        );
        object.insert(
            "issuer_signature".to_owned(),
            JsonValue::String(V1_SIGNATURE_HEX.to_owned()),
        );
        envelope
    }

    #[test]
    fn issuance_signed_bytes_are_canonical() {
        let signed = issuance_signed_bytes(&signed_v1_envelope()).unwrap();
        assert_eq!(signed, V1_SIGNED_BYTES.as_bytes());
    }

    #[test]
    fn issuance_binding_parses_fixed_vector() {
        let parsed =
            check_issuance_binding(&signed_v1_envelope(), &evidence_terms().trusted_key_ids)
                .unwrap();
        assert_eq!(parsed.key_id, TEST_ISSUER_KEY);
        assert_eq!(parsed.signed_bytes, V1_SIGNED_BYTES.as_bytes());
        assert_eq!(parsed.signature.len(), 64);
        // Cryptographic validity of this exact vector is proven by the
        // mncs-cli host verifier test using the same bytes: the contract
        // here pins shape and binding, hosts prove the math.
    }

    #[test]
    fn issuance_binding_fails_closed() {
        let trusted = evidence_terms().trusted_key_ids;
        let mut unsigned: JsonValue = serde_json::from_str(V1_SIGNED_BYTES).unwrap();
        unsigned
            .as_object_mut()
            .expect("vector is an object")
            .remove("issuer");
        assert_eq!(
            check_issuance_binding(&unsigned, &trusted),
            Err(AuthorityError::MissingIssuer)
        );
        let unsigned_sig: JsonValue = serde_json::from_str(V1_SIGNED_BYTES).unwrap();
        assert_eq!(
            check_issuance_binding(&unsigned_sig, &trusted),
            Err(AuthorityError::BadSignatureEncoding)
        );
        let mut wrong_alg = signed_v1_envelope();
        wrong_alg["issuer"]["algorithm"] = JsonValue::String("hmac-sha256".to_owned());
        assert_eq!(
            check_issuance_binding(&wrong_alg, &trusted),
            Err(AuthorityError::UnknownIssuerAlgorithm(
                "hmac-sha256".to_owned()
            ))
        );
        assert_eq!(
            check_issuance_binding(&signed_v1_envelope(), &BTreeSet::from(["other".to_owned()])),
            Err(AuthorityError::UntrustedIssuer(TEST_ISSUER_KEY.to_owned()))
        );
        let mut bad_sig = signed_v1_envelope();
        bad_sig["issuer_signature"] = JsonValue::String("zz".to_owned());
        assert_eq!(
            check_issuance_binding(&bad_sig, &trusted),
            Err(AuthorityError::BadSignatureEncoding)
        );
    }

    const V2_ATTESTATION_JSON: &str = "{\"issued_at\":1700000000,\"issuer\":{\"algorithm\":\"ed25519\",\"key_id\":\"test-key-1\"},\"issuer_signature\":\"b79fddcb43b732be02206b366994895894a636ef8b4c5bccf00c7b75477b67228e2838b8a08468a9b567dd4ead05f31e53bed1ee445a245131da2336ce4f280b\",\"name\":\"lab.safety-cert\",\"participant\":\"e2e-agent\",\"schema_version\":\"mncs.evidence-attestation/1\",\"scope\":\"repo(mncs-language)\",\"source_subsystem\":\"lab-safety\",\"subject\":\"\"}";

    fn attested_value() -> JsonValue {
        serde_json::from_str(V2_ATTESTATION_JSON).expect("attestation vector parses")
    }

    #[test]
    fn attested_evidence_supplies_but_bare_values_do_not() {
        let terms = evidence_terms();
        assert_eq!(
            evidence_standing(
                "lab.safety-cert",
                Some(&attested_value()),
                false,
                "",
                &terms
            ),
            EvidenceStanding::Attested
        );
        assert_eq!(
            evidence_standing(
                "lab.safety-cert",
                Some(&JsonValue::String("trust me".to_owned())),
                false,
                "",
                &terms
            ),
            EvidenceStanding::NotSupplied
        );
        assert_eq!(
            evidence_standing("lab.safety-cert", None, false, "", &terms),
            EvidenceStanding::NotSupplied
        );
        assert_eq!(
            evidence_standing("lab.safety-cert", Some(&attested_value()), true, "", &terms),
            EvidenceStanding::SystemSupplied
        );
    }

    #[test]
    fn attestation_mismatch_never_supplies() {
        let terms = evidence_terms();
        assert_eq!(
            evidence_standing("other-evidence", Some(&attested_value()), false, "", &terms),
            EvidenceStanding::NotSupplied
        );
        let mut wrong_party = attested_value();
        wrong_party["participant"] = JsonValue::String("mallory".to_owned());
        assert_eq!(
            evidence_standing("lab.safety-cert", Some(&wrong_party), false, "", &terms),
            EvidenceStanding::NotSupplied
        );
        let stale = EvidenceTerms {
            now_secs: 1_800_000_000,
            ..terms.clone()
        };
        assert_eq!(
            evidence_standing(
                "lab.safety-cert",
                Some(&attested_value()),
                false,
                "",
                &stale
            ),
            EvidenceStanding::NotSupplied
        );
        let future = EvidenceTerms {
            now_secs: 1_600_000_000,
            ..terms.clone()
        };
        assert_eq!(
            evidence_standing(
                "lab.safety-cert",
                Some(&attested_value()),
                false,
                "",
                &future
            ),
            EvidenceStanding::NotSupplied
        );
        let mut untrusted = attested_value();
        untrusted["issuer"]["key_id"] = JsonValue::String("attacker-key".to_owned());
        assert_eq!(
            evidence_standing("lab.safety-cert", Some(&untrusted), false, "", &terms),
            EvidenceStanding::NotSupplied
        );
    }

    #[test]
    fn conditional_promotes_only_on_attested_evidence() {
        let mut conditional = decision("tests.execute", DecisionStatus::Conditional);
        conditional.missing = vec!["lab.safety-cert".to_owned()];
        let mut folds = BTreeMap::new();
        folds.insert(
            "tests.execute".to_owned(),
            fold_capability("tests.execute", &[conditional]),
        );
        let targets = BTreeMap::new();
        let bare = RequirementLeg {
            name: "cpu".to_owned(),
            needs: vec!["tests.execute".to_owned()],
            backend: String::new(),
            target: String::new(),
            evidence: BTreeMap::from([(
                "lab.safety-cert".to_owned(),
                JsonValue::String("trust me".to_owned()),
            )]),
        };
        assert_eq!(
            accept_leg(&bare, &folds, "sha256:x", "", &targets, &evidence_terms()).verdict,
            AuthorityVerdict::Unknown
        );
        let attested = RequirementLeg {
            evidence: BTreeMap::from([("lab.safety-cert".to_owned(), attested_value())]),
            ..bare.clone()
        };
        assert_eq!(
            accept_leg(
                &attested,
                &folds,
                "sha256:x",
                "",
                &targets,
                &evidence_terms()
            )
            .verdict,
            AuthorityVerdict::Granted
        );
    }

    #[test]
    fn canonical_confirm_matrix() {
        use AuthorityVerdict::{Granted, Refused, Unknown};
        use ProofClass::{ConsumerDeclared, Legacy, Operator, Unknown as NoProof};
        assert_eq!(
            confirm_execution("worker-01", "worker-01", Operator, true).verdict,
            Granted
        );
        assert_eq!(
            confirm_execution("worker-01", "worker-09", Operator, true).verdict,
            Refused
        );
        assert_eq!(
            confirm_execution("worker-01", "worker-01", Operator, false).verdict,
            Unknown
        );
        assert_eq!(
            confirm_execution("worker-01", "worker-01", ConsumerDeclared, true).verdict,
            Refused
        );
        assert_eq!(
            confirm_execution("worker-01", "worker-01", NoProof, true).verdict,
            Refused
        );
        assert_eq!(
            confirm_execution("worker-01", "", Operator, true).verdict,
            Refused
        );
        assert_eq!(
            confirm_execution("", "worker-01", Operator, true).verdict,
            Unknown
        );
        assert_eq!(
            confirm_execution("worker-01", "worker-01", Legacy, true).verdict,
            Unknown
        );
        assert_eq!(
            confirm_execution("worker-01", "worker-09", Legacy, true).verdict,
            Refused
        );
        assert_eq!(
            confirm_execution("worker-01", "", Legacy, true).verdict,
            Unknown
        );
    }
}
