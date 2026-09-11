//! P2-006: a 64-record generation validates in-language through bounded
//! streaming windows (Profile 0.15 composition, no host-side count check).
//!
//! The 520-byte image (8-byte header + 64 8-byte records) validates as four
//! 128-byte windows through one shared `[byte; up_to 256]` validator: view
//! sub-slicing plus static widening compose, so the header COUNT
//! cross-check and every per-record verdict are MNCS-produced. The BE
//! decoders use checked arithmetic over bytes and carry no overflow debt
//! (P1-021 range discharge); only honest runtime-check obligations remain
//! (slice ranges, dynamic indices, iteration cost). Every corruption class
//! fails closed with a distinct code on every executable backend.

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

/// Windowed generation validation agrees on every backend, verdicts and
/// codes alike: no host length-gate parity is needed for the COUNT
/// cross-check or any corruption class.
#[test]
fn generation_scan_windows_agree_on_every_backend() {
    let source = example("source/pressure-generation-scan.mncs");
    let corpus = example("execution/pressure-generation-scan-corpus.json");
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
        assert_eq!(cases.len(), 7, "{backend}: case count");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
            assert_eq!(
                case["status"], "returned",
                "{backend} {id}: status {case:#}"
            );
            assert_eq!(
                case["expectation_met"], true,
                "{backend} {id}: verdict mismatch; returned={:#}",
                case["returned"]
            );
        }
    }
}

/// The scan's checked decoder arithmetic carries no overflow debt: the
/// only open obligations are the honest runtime checks (slice ranges,
/// dynamic indices, iteration cost), never `integer-overflow`.
#[test]
fn generation_scan_carries_no_overflow_debt() {
    let output = binary()
        .args([
            "source-study",
            &example("source/pressure-generation-scan.mncs"),
        ])
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run source-study");
    let result: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
    let unresolved: Vec<String> = result["unresolved_obligations"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();
    assert!(
        !unresolved.is_empty(),
        "window slices and dynamic indices keep their runtime checks"
    );
    assert!(
        unresolved
            .iter()
            .all(|identity| !identity.contains("integer-overflow")),
        "no overflow debt in the windowed scan: {unresolved:?}"
    );
}
