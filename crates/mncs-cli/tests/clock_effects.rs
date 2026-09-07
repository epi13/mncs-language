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
    example("source/clock-scan.mncs")
}

fn corpus() -> String {
    example("execution/clock-scan-corpus.json")
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

fn time_args(capability: &str) -> Vec<String> {
    vec!["--grant-time".to_owned(), capability.to_owned()]
}

fn is_pure(case: &Value) -> bool {
    // Observations omit `expected_effects` when no effects are expected.
    case.get("expected_effects")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
}

fn run_args(extra: &[String]) -> Vec<String> {
    let mut args = vec![
        "experiment".to_owned(),
        "run".to_owned(),
        source(),
        "--backend".to_owned(),
        "mncs-research-bytecode".to_owned(),
        "--corpus".to_owned(),
        corpus(),
    ];
    args.extend(extra.iter().cloned());
    args
}

/// Unauthorized `clock_read()` shapes are rejected at elaboration: no
/// declaration, doubled declarations, and call-site arguments.
#[test]
fn clock_read_authority_gaps_are_rejected() {
    let cases = [
        ("source/clock-invalid-missing-decl.mncs", "MNE238"),
        ("source/clock-invalid-double.mncs", "MNE239"),
        ("source/clock-invalid-arity.mncs", "MNP194"),
    ];
    for (fixture, expected_code) in cases {
        let codes = diagnostics(&example(fixture));
        assert!(
            codes.contains(&expected_code.to_owned()),
            "{fixture}: expected {expected_code} in {codes:?}"
        );
    }
}

/// A real MNCS program observes wall-clock instants through the
/// capability boundary (HARNESS-PRESSURE-005): granted instants flow
/// into relational comparisons, and the realized effect is recorded
/// with kind/target/capability while unexpected effects are prohibited.
#[test]
fn granted_instants_flow_and_effects_are_recorded() {
    let (success, result) = run_with(&run_args(&time_args("ticker")));
    assert!(success, "granted run exits success: {result}");
    for case in result["cases"].as_array().unwrap() {
        assert_eq!(case["status"], "returned", "{case}");
        assert_eq!(case["expectation_met"], true, "{case}");
        assert_eq!(case["effects_met"], true, "{case}");
    }
}

/// Without a grant the call fails closed: Unsupported, never a value,
/// and no effect is recorded. The corpus declares the granted
/// expectation, so it reports unmet (false): nothing was produced.
#[test]
fn clock_read_without_grant_fails_closed() {
    let (success, result) = run_with(&run_args(&[]));
    assert!(!success, "ungranted run must not succeed");
    for case in result["cases"].as_array().unwrap() {
        if is_pure(case) {
            // Pure stdlib relations need no authority and keep passing.
            assert_eq!(case["status"], "returned", "{case}");
            assert_eq!(case["expectation_met"], true, "{case}");
            continue;
        }
        assert_eq!(case["status"], "unsupported", "{case}");
        assert_eq!(case["expectation_met"], false, "{case}");
    }
}

/// A grant for the wrong capability does not satisfy the declared
/// authority: InvalidRequest, never a value.
#[test]
fn wrong_capability_grant_is_rejected() {
    let (success, result) = run_with(&run_args(&time_args("other_reader")));
    assert!(!success, "wrong-capability run must not succeed");
    for case in result["cases"].as_array().unwrap() {
        if is_pure(case) {
            // Pure stdlib relations need no authority and keep passing.
            assert_eq!(case["status"], "returned", "{case}");
            assert_eq!(case["expectation_met"], true, "{case}");
            continue;
        }
        assert_eq!(case["status"], "invalid_request", "{case}");
    }
}

/// A malformed `--grant-time` value is refused before any execution.
#[test]
fn malformed_grant_time_is_refused() {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(run_args(&time_args("ticker=extra")))
        .output()
        .expect("run experiment");
    assert_eq!(output.status.code(), Some(2), "malformed grant exits 2");
}

/// Relational verdicts are stable across executions even though the
/// observed instants differ: corpora pin relations, never absolutes.
#[test]
fn relational_verdicts_are_stable_across_runs() {
    let (first_ok, first) = run_with(&run_args(&time_args("ticker")));
    let (second_ok, second) = run_with(&run_args(&time_args("ticker")));
    assert!(first_ok && second_ok, "both runs succeed");
    let verdicts = |result: &Value| {
        result["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|case| {
                (
                    case["case_id"].as_str().unwrap().to_owned(),
                    case["returned"].clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(verdicts(&first), verdicts(&second));
}

/// The portable WASM backend cannot realize host calls: it refuses at
/// lowering and never manufactures PASS evidence.
#[test]
fn wasm_backend_refuses_host_calls() {
    let extra = time_args("ticker");
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
