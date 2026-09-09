//! Index PRESS-013/016/017/018: durable-state transition contracts as
//! language-owned semantics (RFC 0026 substrate).
//!
//! `mncs.std.store.v1` pins compare-and-transition, snapshot-handle
//! isolation, retention policy, and durability receipts as total bounded
//! functions. The OS realization (files, locks, fsync) and its crash
//! evidence are future work behind these contracts; what these tests
//! prove is that the contracts agree on every executable backend, agree
//! across the layered reference executors, and converge on repetition.

use std::process::Command;

use serde_json::Value;

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn example(name: &str) -> String {
    let raw = format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"));
    let mut parts: Vec<&str> = Vec::new();
    for segment in raw.split('/') {
        if segment == ".." {
            parts.pop();
        } else {
            parts.push(segment);
        }
    }
    parts.join("/")
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

fn source() -> String {
    format!("{}/std/store.mncs", library_dir())
}

fn corpus() -> String {
    example("execution/store-corpus.json")
}

fn run(backend: &str) -> Value {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "experiment",
            "run",
            &source(),
            "--backend",
            backend,
            "--corpus",
            &corpus(),
        ])
        .output()
        .expect("run experiment");
    assert!(
        output.status.success(),
        "{backend}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("result JSON")
}

/// CAS race, snapshot isolation, retention scans, and the durability
/// receipt agree on every executable backend.
#[test]
fn store_contracts_agree_on_every_executable_backend() {
    for backend in EXECUTABLE_BACKENDS {
        let result = run(backend);
        let cases = result["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 4, "{backend}: corpus drift");
        for case in cases {
            assert_eq!(case["status"], "returned", "{backend}: {case}");
            assert_eq!(case["expectation_met"], true, "{backend}: {case}");
        }
    }
}

/// The store contracts agree across the layered reference executors.
#[test]
fn store_contracts_agree_across_layers() {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(["check-backend-execution", &source(), &corpus()])
        .output()
        .expect("run layered check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("layered JSON");
    assert_eq!(result["status"], "consistent_over_corpus");
    assert_eq!(result["mismatching_cases"], 0);
}

/// Repeated runs converge to identical witnesses.
#[test]
fn store_witnesses_are_deterministically_repeatable() {
    let first = run("mncs-research-bytecode");
    let second = run("mncs-research-bytecode");
    assert_eq!(first["cases"], second["cases"]);
}
