//! Index PRESS-002: deterministic bounded-channel contracts.
//!
//! `mncs.std.channel.v1` owns the source-semantics half of
//! producer/consumer communication — bounded capacity with backpressure
//! as data, a close-once contract gated on producer completion, drained
//! close observation without sentinel values, and deterministic FIFO
//! shutdown — as total bounded functions. These tests prove the contracts
//! agree on every executable backend and across the layered reference
//! executors, and that repeated runs converge.

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

/// Bounded flow, backpressure, close-once, and drained-close witnesses
/// agree on every executable backend.
#[test]
fn channel_contracts_agree_on_every_executable_backend() {
    let source = format!("{}/std/channel.mncs", library_dir());
    let corpus = example("execution/channel-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let result = run(&source, &corpus, backend);
        assert_all_met(&result, backend, 3);
    }
}

/// The channel contracts agree across the layered reference executors.
#[test]
fn channel_contracts_agree_across_layers() {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "check-backend-execution",
            &format!("{}/std/channel.mncs", library_dir()),
            &example("execution/channel-corpus.json"),
        ])
        .output()
        .expect("run layered channel check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("layered JSON");
    assert_eq!(result["status"], "consistent_over_corpus");
    assert_eq!(result["mismatching_cases"], 0);
}

/// Repeated runs converge to identical witnesses: delivery order is a
/// function of the FIFO, never of producer timing.
#[test]
fn channel_witnesses_are_deterministically_repeatable() {
    let source = format!("{}/std/channel.mncs", library_dir());
    let corpus = example("execution/channel-corpus.json");
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
