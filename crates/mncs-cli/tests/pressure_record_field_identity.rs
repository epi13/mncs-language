//! WEB-P-011: nominal record (and finite payload) corpus values resolve
//! fields BY NAME on every executable backend.
//!
//! The interpreter used to accept record fields in any order while compiled
//! backends demanded canonical (name-sorted) order positionally, and nested
//! records inside sequences bypassed validation entirely and were bound
//! positionally — silently reading one field's value as another's. The
//! language-owned rule is now: external fields resolve by name at every
//! nesting level, accepted values normalize into canonical order, and
//! malformed values (missing/unknown/duplicate/wrong-type) are rejected
//! with an `MNCS_VALUE_CONTRACT` diagnostic naming expected-vs-received
//! fields. This battery pins declaration-order, canonical-order, shuffled,
//! nested, doubly-nested, sequence-carried, and finite-payload inputs plus
//! every malformed shape, on all five backends.

use std::process::Command;

use serde_json::Value;

/// Stage F: the `mncs-stdlib` checkout backing these tests: explicit
/// `MNCS_STDLIB_ROOT` wins, else the `mncs-stdlib` sibling checkout.
/// Fails closed with a clear message when absent.
fn stdlib_checkout_dir() -> String {
    // Test inputs need a real checkout: an explicitly empty variable
    // (the CLI's hermetic spelling) falls through to the sibling here.
    let explicit = std::env::var("MNCS_STDLIB_ROOT")
        .ok()
        .filter(|root| !root.trim().is_empty());
    let checkout =
        explicit.unwrap_or_else(|| format!("{}/../../../mncs-stdlib", env!("CARGO_MANIFEST_DIR")));
    assert!(
        std::path::Path::new(&checkout).is_dir(),
        "mncs-stdlib checkout missing at {checkout}; set MNCS_STDLIB_ROOT"
    );
    checkout
}

/// Stage F: standard-library sources now live in `mncs-stdlib/library/`.
fn stdlib_library_dir() -> String {
    let checkout = stdlib_checkout_dir();
    let dir = format!("{checkout}/library");
    assert!(
        std::path::Path::new(&dir).is_dir(),
        "mncs-stdlib checkout missing at {checkout}; set MNCS_STDLIB_ROOT"
    );
    dir
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn library(name: &str) -> String {
    format!("{}/{name}", stdlib_library_dir())
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

/// Well-formed inputs in any field order return identical values on every
/// backend; malformed inputs are rejected with the stable contract
/// diagnostic instead of being silently misbound.
#[test]
fn record_fields_resolve_by_name_on_every_backend() {
    let source = example("source/pressure-record-field-identity.mncs");
    let corpus = example("execution/pressure-record-field-identity-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        // No exit-code assertion: the corpus deliberately contains
        // invalid_request cases, which mark the experiment FAIL by design.
        // Every case outcome is pinned individually below.
        let (_code, result, stderr) = run_experiment(&source, backend, &corpus);
        let cases = result["cases"].as_array().unwrap_or_else(|| {
            panic!("{backend}: missing cases; stderr={stderr} result={result:#}")
        });
        assert_eq!(cases.len(), 29, "{backend}: case count changed; {result:#}");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
            assert_eq!(
                case["status_met"], true,
                "{backend} {id}: status not met; case={case:#}"
            );
            let status = case["status"].as_str().unwrap_or("?");
            if status == "invalid_request" {
                let reason = case["failure_reason"].as_str().unwrap_or("");
                assert!(
                    reason.contains("MNCS_VALUE_CONTRACT"),
                    "{backend} {id}: rejection lacks the stable contract diagnostic; reason={reason:#}"
                );
            }
        }
    }
}

/// The historically silent shape — a nested record inside a sequence with
/// declaration-order fields — must be interpreted correctly (not rejected,
/// not misbound) on every backend.
#[test]
fn nested_declaration_order_reads_the_named_field_on_every_backend() {
    let source = example("source/pressure-record-field-identity.mncs");
    let corpus = example("execution/pressure-record-field-identity-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (_code, result, _stderr) = run_experiment(&source, backend, &corpus);
        let cases = result["cases"].as_array().expect("cases");
        let case = cases
            .iter()
            .find(|case| case["case_id"] == "box-first-alpha-decl")
            .unwrap_or_else(|| panic!("{backend}: missing nested case"));
        assert_eq!(case["status"], "returned", "{backend}: {case:#}");
        assert_eq!(
            case["returned"],
            serde_json::json!([{"integer": {"value": 11, "type": {"bits": 64, "signed": false}}}]),
            "{backend}: nested declaration-order fields misbound; {case:#}"
        );
    }
}
