use std::process::Command;

use serde_json::Value;

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

fn diagnostics(path: &str) -> Vec<String> {
    source_study(path)["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|diag| diag["code"].as_str().map(str::to_owned))
        .collect()
}

fn run_backend(source: &str, backend: &str, corpus: &str) -> Value {
    let output = binary()
        .args(["experiment", "run", source, "--backend", backend, "--corpus", corpus])
        .output()
        .expect("run experiment");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("result JSON")
}

fn assert_all_met(result: &Value, backend: &str) {
    for case in result["cases"].as_array().unwrap() {
        assert_eq!(case["status"], "returned", "{backend}: {case}");
        assert_eq!(case["expectation_met"], true, "{backend}: {case}");
    }
}

/// Boolean `match` patterns (HARNESS-PRESSURE-013): `true`/`false` arms
/// execute identically on both executable backends.
#[test]
fn bool_match_executes_and_agrees_on_both_backends() {
    let source = example("source/profile06-bool-match.mncs");
    let corpus = example("execution/profile06-bool-match-corpus.json");
    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let result = run_backend(&source, backend, &corpus);
        assert_all_met(&result, backend);
    }
}

/// Boolean `match` arms agree across the layered reference executors.
#[test]
fn bool_match_agrees_across_layers() {
    let source = example("source/profile06-bool-match.mncs");
    let corpus = example("execution/profile06-bool-match-corpus.json");
    let output = binary()
        .args(["check-backend-execution", &source, &corpus])
        .output()
        .expect("run layered boolean-match check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("layered JSON");
    assert_eq!(result["status"], "consistent_over_corpus");
    assert_eq!(result["mismatching_cases"], 0);
}

/// `capability` as a term-level identifier (HARNESS-PRESSURE-014): the
/// eligibility-shaped fixture executes identically on both backends.
#[test]
fn capability_identifier_executes_on_both_backends() {
    let source = example("source/profile06-capability-identifier.mncs");
    let corpus = example("execution/profile06-capability-identifier-corpus.json");
    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let result = run_backend(&source, backend, &corpus);
        assert_all_met(&result, backend);
    }
}

/// Boolean-match negatives carry finite-type exhaustiveness diagnostics.
#[test]
fn bool_match_negatives_are_rejected_with_intended_codes() {
    let cases = [
        ("source/profile06-invalid-bool-match-missing.mncs", "MNE140"),
        ("source/profile06-invalid-bool-match-duplicate.mncs", "MNE139"),
        ("source/profile06-invalid-bool-match-unknown.mncs", "MNE138"),
    ];
    for (fixture, expected_code) in cases {
        let codes = diagnostics(&example(fixture));
        assert!(
            codes.contains(&expected_code.to_owned()),
            "{fixture}: expected {expected_code} in {codes:?}"
        );
    }
}

/// Diagnostic cascades stay suppressed (HARNESS-PRESSURE-012): one malformed
/// iteration header or match separator is exactly one diagnostic.
#[test]
fn malformed_iteration_and_match_headers_emit_single_diagnostics() {
    let cases = [
        (
            "source/profile06-invalid-iteration-bound.mncs",
            "MNP094",
        ),
        (
            "source/profile06-invalid-match-missing-comma.mncs",
            "MNP192",
        ),
    ];
    for (fixture, expected_code) in cases {
        let codes = diagnostics(&example(fixture));
        assert_eq!(
            codes,
            vec![expected_code.to_owned()],
            "{fixture}: expected exactly one {expected_code}"
        );
    }
}
