use std::process::Command;

use serde_json::Value;

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn source() -> String {
    example("source/crypto-verify.mncs")
}

fn corpus() -> String {
    example("execution/crypto-verify-corpus.json")
}

fn reject_corpus() -> String {
    example("execution/crypto-verify-reject-corpus.json")
}

fn diagnostics(path: &str) -> Vec<String> {
    let output = binary()
        .args(["source-study", path])
        .output()
        .expect("run source-study");
    let study: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
    study["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|diag| diag["code"].as_str().map(str::to_owned))
        .collect()
}

fn run_with(args: &[String]) -> (bool, Value) {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(args)
        .output()
        .expect("run experiment");
    let value: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
    (output.status.success(), value)
}

fn crypto_args(capability: &str) -> Vec<String> {
    vec!["--grant-crypto".to_owned(), capability.to_owned()]
}

fn run_args(extra: &[String], corpus_path: &str) -> Vec<String> {
    let mut args = vec![
        "experiment".to_owned(),
        "run".to_owned(),
        source(),
        "--backend".to_owned(),
        "mncs-research-bytecode".to_owned(),
        "--corpus".to_owned(),
        corpus_path.to_owned(),
    ];
    args.extend(extra.iter().cloned());
    args
}

/// Unauthorized crypto shapes are rejected at elaboration: missing and
/// doubled declarations plus wrong-arity and non-view operands for both
/// intrinsics.
#[test]
fn crypto_authority_gaps_are_rejected() {
    let cases = [
        (
            "source/crypto-invalid-missing-decl.mncs",
            vec!["MNE241", "MNE244"],
        ),
        (
            "source/crypto-invalid-double.mncs",
            vec!["MNE242", "MNE245"],
        ),
        ("source/crypto-invalid-arity.mncs", vec!["MNP195", "MNP196"]),
        (
            "source/crypto-invalid-operand.mncs",
            vec!["MNE243", "MNE246"],
        ),
    ];
    for (fixture, expected_codes) in cases {
        let codes = diagnostics(&example(fixture));
        for expected_code in expected_codes {
            assert!(
                codes.contains(&expected_code.to_owned()),
                "{fixture}: expected {expected_code} in {codes:?}"
            );
        }
    }
}

/// Granted verify-only crypto (HARNESS-PRESSURE-006) matches the
/// independent-oracle vectors byte-exactly: the SHA-256 digests equal
/// the pinned 32 bytes, the genuine issuance verifies, the forged one
/// reports false (a verdict, never a failure), and effects are recorded
/// with kind/target/capability. Pure-data verification adds no
/// obligations, so the experiment reports PASS.
#[test]
fn granted_crypto_matches_independent_oracle() {
    let (success, result) = run_with(&run_args(&crypto_args("verifier"), &corpus()));
    assert!(success, "granted run exits success: {result}");
    assert_eq!(result["status"], "PASS", "{result}");
    for case in result["cases"].as_array().unwrap() {
        assert_eq!(case["status"], "returned", "{case}");
        assert_eq!(case["expectation_met"], true, "{case}");
        assert_eq!(case["effects_met"], true, "{case}");
    }
}

/// Malformed shapes fail closed with pinned expectations: a 31-byte key
/// and a 63-byte signature are InvalidRequest, never values, and the
/// run reports the refusals instead of passing.
#[test]
fn malformed_shapes_fail_closed() {
    let (success, result) = run_with(&run_args(&crypto_args("verifier"), &reject_corpus()));
    assert!(!success, "reject run must not succeed");
    for case in result["cases"].as_array().unwrap() {
        assert_eq!(case["status"], "invalid_request", "{case}");
        assert_eq!(case["status_met"], true, "{case}");
        assert_eq!(case["effects_met"], true, "{case}");
    }
}

/// Without a grant the calls fail closed: Unsupported, never values.
#[test]
fn crypto_without_grant_fails_closed() {
    let (success, result) = run_with(&run_args(&[], &corpus()));
    assert!(!success, "ungranted run must not succeed");
    for case in result["cases"].as_array().unwrap() {
        assert_eq!(case["status"], "unsupported", "{case}");
    }
}

/// A grant for the wrong capability does not satisfy the declared
/// authority: InvalidRequest, never values.
#[test]
fn wrong_capability_grant_is_rejected() {
    let (success, result) = run_with(&run_args(&crypto_args("other"), &corpus()));
    assert!(!success, "wrong-capability run must not succeed");
    for case in result["cases"].as_array().unwrap() {
        assert_eq!(case["status"], "invalid_request", "{case}");
    }
}

/// A malformed `--grant-crypto` value is refused before any execution.
#[test]
fn malformed_grant_crypto_is_refused() {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(run_args(&crypto_args("verifier=extra"), &corpus()))
        .output()
        .expect("run experiment");
    assert_eq!(output.status.code(), Some(2), "malformed grant exits 2");
}

/// The portable WASM backend cannot realize host calls: it refuses at
/// lowering and never manufactures PASS evidence.
#[test]
fn wasm_backend_refuses_host_calls() {
    let extra = crypto_args("verifier");
    let mut args = vec![
        "experiment".to_owned(),
        "run".to_owned(),
        source(),
        "--backend".to_owned(),
        "mncs-portable-wasm-mvp".to_owned(),
        "--corpus".to_owned(),
        corpus(),
    ];
    args.extend(extra);
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(args)
        .output()
        .expect("run wasm experiment");
    assert!(!output.status.success(), "wasm run must not succeed");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("host calls are unsupported"),
        "wasm refusal names host calls: {text:?}"
    );
}
