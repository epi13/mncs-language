//! ENG-PRESSURE-0008: checked-division obligations discharge statically.
//!
//! Division by a nonzero literal used to report opaque `integer-overflow`
//! obligation hashes with no divisor information. Division now carries two
//! explicit digest-bound obligations — `divisor-nonzero` and (signed only)
//! `signed-division-overflow` — and literal divisors discharge them through
//! the existing obligation ledger (same regime as wrapping-intent totality:
//! Rust routes the elaborated literal fact, proof authority stays in the
//! MNCS kernel path, runtime guards are retained regardless).
//!
//! The execution corpus pins values on every backend; the study assertions
//! below pin the discharge/refusal boundary: literal nonzero divisors leave
//! no open division fact, while unknown, zero, and `x / -1` divisors keep
//! precise open obligations.

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
fn checked_division_values_agree_per_backend() {
    let source = example("source/pressure-division.mncs");
    let corpus = example("execution/pressure-division-corpus.json");
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
                "{backend} {id}: logical value mismatch; returned={:#}",
                case["returned"]
            );
        }
    }
}

/// The discharge/refusal boundary at the study surface: no open division
/// fact for literal nonzero divisors; precise open facts otherwise; never
/// an opaque `integer-overflow` hash for a division operation.
#[test]
fn division_obligations_discharge_and_refuse_precisely() {
    let output = binary()
        .args(["source-study", &example("source/pressure-division.mncs")])
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
    // Exactly the refusal set: `probe_unk` (divisor + overflow) and
    // `probe_neg1` (overflow only). Every literal case discharged.
    assert_eq!(unresolved.len(), 3, "refusal set: {unresolved:?}");
    assert_eq!(
        unresolved
            .iter()
            .filter(|identity| identity.contains("divisor-nonzero"))
            .count(),
        1,
        "one open divisor-nonzero: {unresolved:?}"
    );
    assert_eq!(
        unresolved
            .iter()
            .filter(|identity| identity.contains("signed-division-overflow"))
            .count(),
        2,
        "two open signed-division-overflow: {unresolved:?}"
    );
    assert!(
        unresolved.iter().all(|identity| identity
            .starts_with("mncs:0.2:obligation:body:divisor-nonzero:")
            || identity.starts_with("mncs:0.2:obligation:body:signed-division-overflow:")),
        "canonical explicit kinds only: {unresolved:?}"
    );
}
