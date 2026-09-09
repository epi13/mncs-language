//! Raised-ceiling pins for two engine pressures.
//!
//! ENG-PRESSURE-0004 (`iterate i up_to 64` in a single loop) and
//! ENG-PRESSURE-0018 (flat sequences past 64 lanes; here a 128-lane row
//! spelled with the ENG-0020 repeat form) hold on current main. This test
//! pins both on every executable backend so the engine entries can be
//! retested and their chaining/nesting workarounds removed.

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
fn raised_ceilings_hold_per_backend() {
    let source = example("source/pressure-ceilings.mncs");
    let corpus = example("execution/pressure-ceilings-corpus.json");
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
        assert_eq!(cases.len(), 2, "{backend}: case count");
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
