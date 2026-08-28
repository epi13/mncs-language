//! Process-boundary ABI agreement: masks, unsigned 64-bit cells, and
//! sequence/view/vector exports. A case is met only when the backend
//! returned the expected logical value, not merely the expected status.

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

/// Require every case to return and to match the expected logical value.
/// Status agreement alone is not success.
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
fn mask_only_exports_agree_per_backend() {
    let source = library("core/mask.mncs");
    let corpus = example("execution/library-core-mask-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_value_agreement(backend, code, &result, &stderr, 13);
    }
}

#[test]
fn mask_only_module_without_cells_agrees_per_backend() {
    run_module_corpus(
        &example("source/abi-mask-only.mncs"),
        &example("execution/abi-mask-only-corpus.json"),
        2,
    );
}

fn run_module_corpus(source: &str, corpus: &str, expected_len: usize) {
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(source, backend, corpus);
        assert_value_agreement(backend, code, &result, &stderr, expected_len);
    }
}

#[test]
fn nested_composite_sequences_agree_per_backend() {
    run_module_corpus(
        &example("source/abi-nested-composites.mncs"),
        &example("execution/abi-nested-composites-corpus.json"),
        10,
    );
}

#[test]
fn boolean_sequences_agree_per_backend() {
    run_module_corpus(
        &example("source/abi-bool-sequences.mncs"),
        &example("execution/abi-bool-sequences-corpus.json"),
        3,
    );
}

#[test]
fn unsigned_sequence_and_view_cells_agree_per_backend() {
    run_module_corpus(
        &library("core/sequences.mncs"),
        &example("execution/library-core-unsigned-sequences-corpus.json"),
        4,
    );
}

#[test]
fn unsigned_encoding_roundtrips_agree_per_backend() {
    run_module_corpus(
        &library("std/encoding.mncs"),
        &example("execution/library-core-unsigned-encoding-corpus.json"),
        6,
    );
}

#[test]
fn unsigned_vector_cells_agree_per_backend() {
    run_module_corpus(
        &library("core/vector.mncs"),
        &example("execution/library-core-vector-u64-corpus.json"),
        3,
    );
}

#[test]
fn unsigned_record_fields_agree_per_backend() {
    run_module_corpus(
        &example("source/abi-unsigned-records.mncs"),
        &example("execution/abi-unsigned-records-corpus.json"),
        2,
    );
}

#[test]
fn wrong_expected_nested_record_value_is_rejected() {
    let (code, result, stderr) = run_experiment(
        &example("source/abi-nested-composites.mncs"),
        "mncs-research-bytecode",
        &example("execution/abi-nested-composites-mutant-corpus.json"),
    );
    assert_eq!(code, Some(1), "mutant must fail the experiment; {stderr}");
    assert_eq!(result["status"], "FAIL", "{result:#}");
    let case = &result["cases"][0];
    assert_eq!(case["status"], "returned");
    assert_eq!(
        case["expectation_met"], false,
        "wrong nested-record logical value must not count as met"
    );
}

#[test]
fn wrong_expected_u64_value_is_rejected() {
    let (code, result, stderr) = run_experiment(
        &library("core/sequences.mncs"),
        "mncs-research-bytecode",
        &example("execution/library-core-unsigned-mutant-corpus.json"),
    );
    assert_eq!(code, Some(1), "mutant must fail the experiment; {stderr}");
    assert_eq!(result["status"], "FAIL", "{result:#}");
    let case = &result["cases"][0];
    assert_eq!(case["status"], "returned");
    assert_eq!(
        case["expectation_met"], false,
        "wrong logical value must not count as met"
    );
}

#[test]
fn sequence_extra_helpers_agree_per_backend() {
    run_module_corpus(
        &library("core/sequences.mncs"),
        &example("execution/library-core-sequences-extra-corpus.json"),
        10,
    );
}

#[test]
fn geometry_alignment_and_relation_helpers_agree_per_backend() {
    run_module_corpus(
        &library("core/geometry.mncs"),
        &example("execution/library-core-geometry-extra-corpus.json"),
        11,
    );
}

#[test]
fn over_capacity_view_is_refused() {
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(
            &library("core/sequences.mncs"),
            backend,
            &example("execution/library-core-view-over-capacity-corpus.json"),
        );
        let case = &result["cases"][0];
        assert_eq!(
            case["status"], "invalid_request",
            "{backend}: over-capacity view must be refused; code={code:?} stderr={stderr} case={case:#}"
        );
        assert_eq!(
            case["status_met"], true,
            "{backend}: expected invalid_request was not matched: {case:#}"
        );
        assert_ne!(
            case["status"], "returned",
            "{backend}: over-capacity view must not execute as success"
        );
    }
}
