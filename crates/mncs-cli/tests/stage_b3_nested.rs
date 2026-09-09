//! Stage B3 nested iteration (Profile 0.11): two-level bounded nests with
//! bound counted indices, plus the negative fixtures that pin the limits.
//! Value agreement per backend; overall PASS/UNKNOWN is accepted.

use std::process::Command;

use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn library(name: &str) -> String {
    format!("{}/../../library/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn run_experiment(source: &str, backend: &str, corpus: &str) -> (Option<i32>, Value, String) {
    let output = binary()
        .args([
            "experiment",
            "run",
            source,
            "--backend",
            backend,
            "--corpus",
            corpus,
        ])
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run experiment");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let value: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("experiment JSON ({stderr}): {error}"));
    (output.status.code(), value, stderr)
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

fn assert_value_agreement(
    backend: &str,
    code: Option<i32>,
    result: &Value,
    stderr: &str,
    expected_len: usize,
) {
    assert_eq!(
        code,
        Some(0),
        "{backend}: unexpected exit; stderr={stderr}; result={result:#}"
    );
    let cases = result["cases"]
        .as_array()
        .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
    assert_eq!(
        cases.len(),
        expected_len,
        "{backend}: case count {} != {expected_len}",
        cases.len()
    );
    for case in cases {
        let id = case["case_id"].as_str().unwrap_or("?");
        assert_eq!(
            case["status"], "returned",
            "{backend} {id}: status {:#}",
            case["status"]
        );
        if !case["status_met"].is_null() {
            assert_eq!(
                case["status_met"], true,
                "{backend} {id}: status_met is not true: {case:#}"
            );
        }
        assert_eq!(
            case["expectation_met"], true,
            "{backend} {id}: logical value mismatch; returned={:#}",
            case["returned"]
        );
    }
    let overall = result["status"].as_str().unwrap_or("");
    assert!(
        overall == "PASS" || overall == "UNKNOWN",
        "{backend}: overall status {overall} is not PASS or UNKNOWN"
    );
}

#[test]
fn nested_iteration_agrees_per_backend() {
    let source = example("source/stage-b3-nested-iteration.mncs");
    let corpus = example("execution/stage-b3-nested-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_value_agreement(backend, code, &result, &stderr, 14);
    }
}

#[test]
fn nested_iteration_limits_are_rejected_with_coded_diagnostics() {
    struct Case {
        fixture: &'static str,
        code: &'static str,
    }
    let cases = [
        Case {
            fixture: "nested-depth-three.mncs",
            code: "MNE147",
        },
        Case {
            fixture: "nested-on-010.mncs",
            code: "MNE147",
        },
        Case {
            fixture: "counted-index-on-010.mncs",
            code: "MNE102",
        },
    ];
    for case in &cases {
        let path = example(&format!("source/profile11/{}", case.fixture));
        let output = binary()
            .args(["validate", &path])
            .output()
            .expect("validate");
        let diagnostics: Value = serde_json::from_slice(&output.stdout).expect("diagnostics JSON");
        let items = diagnostics
            .as_array()
            .map(|list| list.iter().collect::<Vec<_>>())
            .unwrap_or_else(|| {
                diagnostics
                    .get("diagnostics")
                    .and_then(Value::as_array)
                    .map(|list| list.iter().collect::<Vec<_>>())
                    .expect("diagnostics list")
            });
        let codes: Vec<&str> = items
            .iter()
            .filter_map(|diag| diag["code"].as_str())
            .collect();
        assert!(
            !codes.is_empty(),
            "{}: expected rejection, got none",
            case.fixture
        );
        assert!(
            codes.contains(&case.code),
            "{}: expected {}, got {codes:?}",
            case.fixture,
            case.code
        );
    }
}
