//! Bounded JSON recognition agreement: the scanner, stream envelope, and
//! raw projection probes execute identically on every backend.
//!
//! These three corpora were previously generated and committed but never
//! wired to a runner, so the modules they cover (`mncs.std.json.v1`,
//! `mncs.std.json_stream.v1`, `mncs.std.json_projection.v1`) executed only
//! through downstream consumers. Each test runs its probe fixture against
//! its corpus on every executable backend.

use std::process::Command;

use serde_json::Value;

fn workspace(name: &str) -> String {
    format!("{}/../../{name}", env!("CARGO_MANIFEST_DIR"))
}

fn library_dir() -> String {
    format!("{}/../../library/", env!("CARGO_MANIFEST_DIR"))
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

fn run_probe_corpus(source: &str, corpus: &str, backend: &str, expected_cases: usize) {
    let output = Command::new(env!("CARGO_BIN_EXE_mncs"))
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "experiment",
            "run",
            &workspace(source),
            "--backend",
            backend,
            "--corpus",
            &workspace(corpus),
        ])
        .output()
        .expect("run json probe corpus");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{backend} {source}: exit; {stderr}"
    );
    let result: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{backend} {source} JSON ({stderr}): {error}"));
    let cases = result["cases"]
        .as_array()
        .unwrap_or_else(|| panic!("{backend} {source}: missing cases; {result:#}"));
    assert_eq!(
        cases.len(),
        expected_cases,
        "{backend} {source}: corpus drift"
    );
    for case in cases {
        let id = case["case_id"].as_str().unwrap_or("?");
        assert_eq!(
            case["status"], "returned",
            "{backend} {source} {id}: {case:#}"
        );
        assert_eq!(
            case["expectation_met"], true,
            "{backend} {source} {id}: logical value mismatch; returned={:#}",
            case["returned"]
        );
    }
}

#[test]
fn json_scanner_agrees_on_every_executable_backend() {
    for backend in EXECUTABLE_BACKENDS {
        run_probe_corpus(
            "examples/source/json-probe.mncs",
            "examples/execution/json-probe-corpus.json",
            backend,
            10,
        );
    }
}

#[test]
fn json_stream_envelope_agrees_on_every_executable_backend() {
    for backend in EXECUTABLE_BACKENDS {
        run_probe_corpus(
            "examples/source/json-stream-probe.mncs",
            "examples/execution/json-stream-corpus.json",
            backend,
            10,
        );
    }
}

#[test]
fn json_projection_agrees_on_every_executable_backend() {
    for backend in EXECUTABLE_BACKENDS {
        run_probe_corpus(
            "examples/source/json-projection-probe.mncs",
            "examples/execution/json-projection-corpus.json",
            backend,
            1,
        );
    }
}
