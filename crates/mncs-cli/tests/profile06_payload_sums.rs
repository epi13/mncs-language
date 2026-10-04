use std::process::Command;

use serde_json::Value;

/// Stage F: the `mncs-stdlib` checkout backing these tests: explicit
/// `MNCS_STDLIB_ROOT` wins, else the `mncs-stdlib` sibling checkout.
/// Fails closed with a clear message when absent.
fn stdlib_checkout_dir() -> String {
    // Test inputs need a real checkout: an explicitly empty variable
    // (the CLI's hermetic spelling) falls through to the sibling here.
    let explicit = std::env::var("MNCS_STDLIB_ROOT")
        .ok()
        .filter(|root| !root.trim().is_empty());
    let checkout =
        explicit.unwrap_or_else(|| format!("{}/../../../mncs-stdlib", env!("CARGO_MANIFEST_DIR")));
    assert!(
        std::path::Path::new(&checkout).is_dir(),
        "mncs-stdlib checkout missing at {checkout}; set MNCS_STDLIB_ROOT"
    );
    checkout
}

/// Stage F: stdlib-owned examples (fixtures/corpora) read across repos.
fn stdlib_example(name: &str) -> String {
    format!("{}/examples/{name}", stdlib_checkout_dir())
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn source_study(path: &str) -> Value {
    let output = binary()
        .args(["source-study", path])
        .output()
        .expect("run source-study");
    serde_json::from_slice(&output.stdout).expect("source-study JSON")
}

/// Payload-bearing sums (Profile 0.6): construction with canonical payload
/// fields, exhaustive binding patterns, and function-boundary transport must
/// execute identically in both reference executors, and the bounded bytecode
/// realization must meet every corpus expectation.
#[test]
fn profile06_payload_sums_execute_and_agree_on_bytecode() {
    let source = stdlib_example("source/profile06-payload-sums.mncs");
    let corpus = stdlib_example("execution/profile06-payload-sums-corpus.json");
    let output = binary()
        .args([
            "experiment",
            "run",
            &source,
            "--backend",
            "mncs-research-bytecode",
            "--corpus",
            &corpus,
        ])
        .output()
        .expect("run payload sums experiment");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
    // Checked arithmetic inside the fixture stays honestly unresolved; every
    // behavioral case must still be met.
    assert_eq!(result["status"], "UNKNOWN");
    for case_ in result["cases"].as_array().unwrap() {
        assert_eq!(case_["status"], "returned", "{case_}");
        assert_eq!(case_["expectation_met"], true, "{case_}");
    }
    for validation in result["translation_validations"].as_array().unwrap() {
        assert_eq!(validation["judgement"], "PASS");
    }
}

/// The WASM realization now materializes payload sums through linear memory;
/// the whole payload corpus must execute with every expectation met.
#[test]
fn portable_wasm_executes_payload_sums() {
    let source = stdlib_example("source/profile06-payload-sums.mncs");
    let corpus = stdlib_example("execution/profile06-payload-sums-corpus.json");
    let output = binary()
        .args([
            "experiment",
            "run",
            &source,
            "--backend",
            "mncs-portable-wasm-mvp",
            "--corpus",
            &corpus,
        ])
        .output()
        .expect("run payload sums on wasm");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
    // Overall UNKNOWN comes only from honest checked-arithmetic obligations.
    assert_eq!(result["status"], "UNKNOWN");
    for case_ in result["cases"].as_array().unwrap() {
        assert_eq!(case_["status"], "returned", "{case_}");
        assert_eq!(case_["expectation_met"], true, "{case_}");
    }
}

fn expect_rejected(path: &str, expected_code: &str) {
    let result = source_study(path);
    let codes: Vec<&str> = result["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|diag| diag["code"].as_str())
        .collect();
    assert!(
        codes.contains(&expected_code),
        "{path}: expected {expected_code} in {codes:?}"
    );
}

#[test]
fn profile06_negative_fixtures_are_rejected_with_intended_codes() {
    expect_rejected(
        &example("source/profile06-invalid-payload-match-missing.mncs"),
        "MNE140",
    );
    expect_rejected(
        &example("source/profile06-invalid-payload-construction-omits-field.mncs"),
        "MNE174",
    );
    expect_rejected(
        &example("source/profile06-invalid-payload-pattern-unknown-field.mncs"),
        "MNE177",
    );
    expect_rejected(
        &example("source/profile06-invalid-payload-type-mismatch.mncs"),
        "MNE175",
    );
}

/// Strict boolean operators (Profile 0.6): `a && b || c` guards and chained
/// relational guards must agree across body, SSA, and the portable WASM
/// realization (9/9 layered cases).
#[test]
fn profile06_boolean_operators_agree_across_layers() {
    let source = stdlib_example("source/profile06-boolean-operators.mncs");
    let corpus = stdlib_example("execution/profile06-boolean-operators-corpus.json");
    let output = binary()
        .args(["check-backend-execution", &source, &corpus])
        .output()
        .expect("run layered boolean check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("layered JSON");
    assert_eq!(result["status"], "consistent_over_corpus");
    assert_eq!(result["matching_cases"], 9);
    assert_eq!(result["mismatching_cases"], 0);
}
