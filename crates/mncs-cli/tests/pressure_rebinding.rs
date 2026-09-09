//! ENG-PRESSURE-0021: same-scope rebinding is shadowing.
//!
//! `let x = 1; let x = x + 1;` used to fail (`MNE110`); each initializer
//! elaborates against the previous binding and the fresh SSA value keeps
//! every use dominated by its own definition, so rebinding is shadowing,
//! not mutation. The corpus pins multi-step chains, parameter shadowing,
//! type-changing shadowing, and branch-local shadowing on every executable
//! backend, plus the retained `MNE110` for rebinding a traversal-index
//! name in the loop-body scope and the fail-closed discharge when a nested
//! scope shadows an index name.

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

#[test]
fn rebinding_is_shadowing_per_backend() {
    let source = example("source/pressure-rebind.mncs");
    let corpus = example("execution/pressure-rebind-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_eq!(
            code,
            Some(0),
            "{backend}: unexpected exit; stderr={stderr}; result={result:#}"
        );
        let cases = result["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
        assert_eq!(cases.len(), 5, "{backend}: case count");
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
    }
}

/// Rebinding a traversal-index name in the loop-body scope stays `MNE110`:
/// index names are reserved so traversal-discharge reasoning cannot silently
/// change meaning.
#[test]
fn rebind_index_name_stays_reserved() {
    let path = example("source/invalid-rebind-index.mncs");
    let output = binary()
        .args(["validate", &path])
        .output()
        .expect("validate");
    let diagnostics: Value = serde_json::from_slice(&output.stdout).expect("diagnostics JSON");
    let codes: Vec<&str> = diagnostics
        .as_array()
        .expect("diagnostics list")
        .iter()
        .filter_map(|diag| diag["code"].as_str())
        .collect();
    assert!(codes.contains(&"MNE110"), "expected MNE110, got {codes:?}");
}
