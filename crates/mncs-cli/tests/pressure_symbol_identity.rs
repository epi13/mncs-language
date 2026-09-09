//! ENG-PRESSURE-0017: same-named functions in distinct modules must lower
//! and execute as semantically distinct natives.
//!
//! Three legal modules each define a local `helper` (leaf A, leaf B, and a
//! middle module that additionally links leaf B transitively). The root
//! calls them module-qualified. Before qualified native symbols the three
//! native backends refused the whole program (`unsupported`: C redefinition
//! of `mncs_helper`); reference and WASM executed it. Every case pins an
//! independent hand-computed value, and the `entry-*` cases additionally
//! prove that execution requests address each colliding function by its own
//! `(module, function)` identity on every executable backend.

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

/// Every case must return with the expected logical value: status agreement
/// alone would hide a mislinked callee (the historical failure refused
/// loudly, but a silent mix-up of the three `helper` bodies is the deeper
/// hazard this pins against).
fn assert_distinct_callees(backend: &str, code: Option<i32>, result: &Value, stderr: &str) {
    assert_eq!(
        code,
        Some(0),
        "{backend}: unexpected exit; stderr={stderr}; result={result:#}"
    );
    let cases = result["cases"]
        .as_array()
        .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
    assert_eq!(cases.len(), 7, "{backend}: case count");
    for case in cases {
        let id = case["case_id"].as_str().unwrap_or("?");
        assert_eq!(
            case["status"], "returned",
            "{backend} {id}: status {case:#}"
        );
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
fn same_named_functions_stay_distinct_per_backend() {
    let source = example("source/pressure-collision.mncs");
    let corpus = example("execution/pressure-symbol-collision-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_distinct_callees(backend, code, &result, &stderr);
    }
}
