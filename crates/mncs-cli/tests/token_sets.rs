//! Fabric P-007: deterministic bounded token-set algebra as a stdlib
//! abstraction — no first-class set/map syntax. Storage plus logical
//! count, read-only folds, explicit overflow verdicts, lowest-index ties.

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

/// Membership, require-all, forbid-any, preference hits, dedup counting,
/// insertion decisions, and deterministic ranking agree on every
/// executable backend.
#[test]
fn token_set_algebra_agrees_on_every_executable_backend() {
    let source = format!("{}/std/token_set.mncs", library_dir());
    let corpus = example("execution/token-set-corpus.json");
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let result = run(&source, &corpus, backend);
        assert_all_met(&result, backend, 23);
    }
}

/// The Fabric-shaped fleet verdicts (required/forbidden/preferred tokens,
/// deterministic best-worker ranking) agree on every executable backend.
#[test]
fn fleet_token_verdicts_agree_on_every_executable_backend() {
    let source = example("source/fleet-tokens.mncs");
    let corpus = example("execution/fleet-tokens-corpus.json");
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let result = run(&source, &corpus, backend);
        assert_all_met(&result, backend, 13);
    }
}

/// The token-set operations agree across the layered reference executors.
#[test]
fn token_sets_agree_across_layers() {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "check-backend-execution",
            &format!("{}/std/token_set.mncs", library_dir()),
            &example("execution/token-set-corpus.json"),
        ])
        .output()
        .expect("run layered token-set check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("layered JSON");
    assert_eq!(result["status"], "consistent_over_corpus");
    assert_eq!(result["mismatching_cases"], 0);
}
