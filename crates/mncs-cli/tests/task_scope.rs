//! Index PRESS-001/008/012: deterministic structured task-scope contracts.
//!
//! `mncs.std.scope.v1` owns the source-semantics half of structured
//! concurrency — spawn/join ownership, deterministic id-ordered merge,
//! failure-vs-cancellation distinction, cancellation blocking later
//! success, and close-rejects-escape — as total bounded functions. These
//! tests prove the contracts agree on every executable backend and across
//! the layered reference executors, and that repeated runs converge to
//! identical witnesses (schedule independence is structural: the fold
//! walks slot indices, never completion order).

use std::process::Command;

use serde_json::Value;

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

fn run(source: &str, corpus: &str, backend: &str) -> Value {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "experiment",
            "run",
            source,
            "--backend",
            backend,
            "--corpus",
            corpus,
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

fn assert_all_met(result: &Value, backend: &str, expected_cases: usize) {
    let cases = result["cases"].as_array().unwrap();
    assert_eq!(cases.len(), expected_cases, "{backend}: corpus drift");
    for case in cases {
        assert_eq!(case["status"], "returned", "{backend}: {case}");
        assert_eq!(case["expectation_met"], true, "{backend}: {case}");
    }
}

/// Task, failure, cancellation, and ownership witnesses agree on every
/// executable backend.
#[test]
fn scope_contracts_agree_on_every_executable_backend() {
    let source = format!("{}/std/scope.mncs", library_dir());
    let corpus = example("execution/scope-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let result = run(&source, &corpus, backend);
        assert_all_met(&result, backend, 5);
    }
}

/// The scope contracts agree across the layered reference executors.
#[test]
fn scope_contracts_agree_across_layers() {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "check-backend-execution",
            &format!("{}/std/scope.mncs", library_dir()),
            &example("execution/scope-corpus.json"),
        ])
        .output()
        .expect("run layered scope check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("layered JSON");
    assert_eq!(result["status"], "consistent_over_corpus");
    assert_eq!(result["mismatching_cases"], 0);
}

/// Repeated runs converge to identical witnesses: merge order is a
/// function of task ids, never of execution order.
#[test]
fn scope_witnesses_are_deterministically_repeatable() {
    let source = format!("{}/std/scope.mncs", library_dir());
    let corpus = example("execution/scope-corpus.json");
    let first = run(&source, &corpus, "mncs-research-bytecode");
    let second = run(&source, &corpus, "mncs-research-bytecode");
    let witness = |result: &Value| {
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
    assert_eq!(witness(&first), witness(&second));
}
