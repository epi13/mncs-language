//! RFC 0007 differential suite: the MNCS-native proof kernel executed on
//! every backend must agree exactly with the independent Rust reference
//! checker on the curated corpus and on the adversarial fuzz corpus.
//!
//! Agreement is empirical evidence of checker consistency, never a proof of
//! checker correctness. Any divergence is a release-blocking defect in one
//! of the two checkers.

use std::process::Command;

use mncs_model::{
    parse_proof_corpus, reference_check, ProofArtifact, ProofVerdict, SemanticId, PROOF_KERNEL_ID,
};
use serde_json::Value;

fn library(name: &str) -> String {
    format!("{}/../../library/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn run_kernel(source: &str, backend: &str, corpus: &str) -> Value {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library(""))
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
        .expect("run proof kernel experiment");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{backend}: experiment exit; {stderr}"
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{backend}: experiment JSON ({stderr}): {error}"))
}

fn verdict_of_code(code: i64) -> ProofVerdict {
    match code {
        0 => ProofVerdict::Pass,
        1 => ProofVerdict::Fail,
        2 => ProofVerdict::Unknown,
        other => panic!("unknown verdict code {other}"),
    }
}

fn check_corpus_agreement(corpus_name: &str, backends: &[&str]) {
    let text = std::fs::read_to_string(example(corpus_name)).expect("read corpus");
    let cases = parse_proof_corpus(&text).expect("parse proof corpus");
    assert!(!cases.is_empty(), "{corpus_name}: empty corpus");
    for backend in backends {
        let result = run_kernel(
            &library("core/proof_check.mncs"),
            backend,
            &example(corpus_name),
        );
        let returned: std::collections::BTreeMap<String, (String, i64)> = result["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{backend}: missing cases"))
            .iter()
            .map(|case_| {
                let id = case_["case_id"].as_str().unwrap_or("?").to_owned();
                let status = case_["status"].as_str().unwrap_or("?").to_owned();
                let code = case_["returned"][0]["integer"]["value"]
                    .as_i64()
                    .unwrap_or(-99);
                (id, (status, code))
            })
            .collect();
        for case in &cases {
            let (status, code) = returned
                .get(&case.id)
                .unwrap_or_else(|| panic!("{backend} {}: case missing", case.id));
            assert_eq!(
                status, "returned",
                "{backend} {}: kernel trapped instead of failing closed",
                case.id
            );
            let artifact = ProofArtifact::new(
                PROOF_KERNEL_ID,
                SemanticId("mncs:test:obligation".to_owned()),
                Vec::new(),
                Vec::new(),
                case.cells.clone(),
                case.count,
                case.proof,
                case.proposition,
            );
            let reference = reference_check(&artifact);
            assert_eq!(
                verdict_of_code(*code),
                reference,
                "{backend} {}: MNCS kernel disagrees with the reference checker",
                case.id
            );
            if let Some(expected) = case.expected {
                assert_eq!(
                    verdict_of_code(*code),
                    expected,
                    "{backend} {}: verdict changed against the checked-in expectation",
                    case.id
                );
            }
        }
    }
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

#[test]
fn curated_kernel_corpus_agrees_on_every_backend() {
    check_corpus_agreement("execution/proof-kernel-corpus.json", &EXECUTABLE_BACKENDS);
}

#[test]
fn fuzz_kernel_corpus_agrees_on_every_backend() {
    check_corpus_agreement(
        "execution/proof-kernel-fuzz-corpus.json",
        &EXECUTABLE_BACKENDS,
    );
}
