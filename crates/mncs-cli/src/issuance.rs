//! Host-side Ed25519 issuance verification (epi13/mncs-atlas#31).
//!
//! The semantic contract lives in `mncs_model::authority`
//! (`check_issuance_binding`, canonical signed bytes, verification
//! outcomes). This module performs the cryptographic step hosts own:
//! verifying a parsed issuance against a trusted issuer public key.
//! Nothing here decides policy; it answers only whether the recognized
//! authority issued this exact object.

use std::collections::{BTreeMap, BTreeSet};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use mncs_model::{check_issuance_binding, verify_decision_digest, AuthorityError};

/// Verify an issued envelope (Atlas decision or evidence attestation)
/// against trusted issuer public keys (`key_id` -> raw 32-byte key).
/// Returns the issuing key id on success; fails closed otherwise.
pub fn verify_envelope_issuance(
    envelope: &serde_json::Value,
    trusted_keys: &BTreeMap<String, [u8; 32]>,
) -> Result<String, AuthorityError> {
    // Content integrity first: a carried decision_digest that does not
    // match fails before any issuer reasoning. Attestations carry no
    // digest and skip this step.
    if envelope.get("decision_digest").is_some() {
        verify_decision_digest(envelope)?;
    }
    let ids: BTreeSet<String> = trusted_keys.keys().cloned().collect();
    let parsed = check_issuance_binding(envelope, &ids)?;
    let raw = trusted_keys
        .get(&parsed.key_id)
        .ok_or_else(|| AuthorityError::UntrustedIssuer(parsed.key_id.clone()))?;
    let verifying = VerifyingKey::from_bytes(raw)
        .map_err(|_| AuthorityError::InvalidIssuerKey(parsed.key_id.clone()))?;
    let signature = Signature::from_slice(&parsed.signature)
        .map_err(|_| AuthorityError::BadSignatureEncoding)?;
    verifying
        .verify(&parsed.signed_bytes, &signature)
        .map_err(|_| AuthorityError::SignatureMismatch)?;
    Ok(parsed.key_id)
}

/// Decode a 64-character hex public key from trust configuration.
pub fn pubkey_from_hex(text: &str) -> Result<[u8; 32], AuthorityError> {
    if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AuthorityError::InvalidIssuerKey(text.to_owned()));
    }
    let nibble = |byte: u8| match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    };
    let bytes = text
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| nibble(pair[0]) << 4 | nibble(pair[1]))
        .collect::<Vec<_>>();
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY_ID: &str = "test-key-1";
    const TEST_PUBKEY_HEX: &str =
        "28d5aacb5f1d4041c42ef016bac24f803d8f607dd569010b94d7a09a4289522f";
    const V1_DIGEST: &str = "ef52f5d2dc98b1a7c3d462ed4ffe44135c2c8328b35c567e75bf28f8257cab9a";
    const V1_SIGNATURE_HEX: &str = "bc867e8760e1df075531868a84f7dc2ff7f78c76ed6fe49342cf367d7a316143144da4801c2f121849d70653225d99a6b08cb5168eb912bcfbac610e038f9707";
    const V2_ATTESTATION_JSON: &str = "{\"issued_at\":1700000000,\"issuer\":{\"algorithm\":\"ed25519\",\"key_id\":\"test-key-1\"},\"issuer_signature\":\"b79fddcb43b732be02206b366994895894a636ef8b4c5bccf00c7b75477b67228e2838b8a08468a9b567dd4ead05f31e53bed1ee445a245131da2336ce4f280b\",\"name\":\"lab.safety-cert\",\"participant\":\"e2e-agent\",\"schema_version\":\"mncs.evidence-attestation/1\",\"scope\":\"repo(mncs-language)\",\"source_subsystem\":\"lab-safety\",\"subject\":\"\"}";

    fn trusted() -> BTreeMap<String, [u8; 32]> {
        BTreeMap::from([(
            TEST_KEY_ID.to_owned(),
            pubkey_from_hex(TEST_PUBKEY_HEX).expect("vector key decodes"),
        )])
    }

    fn signed_v1() -> serde_json::Value {
        serde_json::json!({
            "authority": "fabric",
            "capability": "tests.execute",
            "decision_by": ["fabric"],
            "decision_digest": V1_DIGEST,
            "decision_digest_alg": "sha256:canonical-json-v1",
            "evidence_required": [],
            "conformant_path": [],
            "execution_target": "",
            "issuer": {"algorithm": "ed25519", "key_id": TEST_KEY_ID},
            "issuer_signature": V1_SIGNATURE_HEX,
            "missing": [],
            "reason": "test issuance",
            "schema_version": "mncs.atlas-capability-decision/1",
            "scope": "repo(mncs-language)",
            "session": {"participant": "e2e-agent", "scope": "repo(mncs-language)"},
            "status": "granted",
            "verdict": "PASS",
        })
    }

    #[test]
    fn fixed_vector_verifies() {
        // Cryptographic proof for the contract-level fixed vector: the
        // same bytes mncs-model pins for shape/binding verify here.
        assert_eq!(
            verify_envelope_issuance(&signed_v1(), &trusted()).unwrap(),
            TEST_KEY_ID
        );
        let attestation: serde_json::Value =
            serde_json::from_str(V2_ATTESTATION_JSON).expect("vector parses");
        assert_eq!(
            verify_envelope_issuance(&attestation, &trusted()).unwrap(),
            TEST_KEY_ID
        );
    }

    #[test]
    fn full_attacker_effort_still_fails() {
        // Attacker flips content AND recomputes the content digest, but
        // cannot reproduce the issuer signature over the new bytes.
        let mut forged = signed_v1();
        forged["capability"] = serde_json::Value::String("worker.dispatch".to_owned());
        forged["execution_target"] = serde_json::Value::String("worker-evil".to_owned());
        let mut undigested = forged.as_object().unwrap().clone();
        undigested.remove("decision_digest");
        let canonical = serde_json::to_string(&serde_json::Value::Object(undigested)).unwrap();
        forged["decision_digest"] =
            serde_json::Value::String(mncs_model::sha256_hex(canonical.as_bytes()));
        assert_eq!(
            verify_envelope_issuance(&forged, &trusted()),
            Err(AuthorityError::SignatureMismatch)
        );
    }

    #[test]
    fn stale_digest_after_mutation_fails_first() {
        let mut edited = signed_v1();
        edited["reason"] = serde_json::Value::String("edited after issuance".to_owned());
        assert_eq!(
            verify_envelope_issuance(&edited, &trusted()),
            Err(AuthorityError::DigestMismatch)
        );
    }

    #[test]
    fn wrong_key_or_unknown_issuer_fails() {
        assert_eq!(
            verify_envelope_issuance(&signed_v1(), &BTreeMap::new()),
            Err(AuthorityError::UntrustedIssuer(TEST_KEY_ID.to_owned()))
        );
        let mut other_key = trusted();
        // Valid curve point, wrong key: signature must not verify.
        other_key.insert(
            TEST_KEY_ID.to_owned(),
            pubkey_from_hex("85b94adcac51039733762d07e077187e1643e0192e4079f76aa945377c661743")
                .expect("vector key decodes"),
        );
        assert_eq!(
            verify_envelope_issuance(&signed_v1(), &other_key),
            Err(AuthorityError::SignatureMismatch)
        );
    }
}
