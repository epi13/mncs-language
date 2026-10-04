//! ENG-PRESSURE-0022: `select` evaluates both arms.
//!
//! Strictness is semantic: `select(true, 7, boom(0))` must trap with the
//! discarded arm's failure on every backend rather than returning 7. A
//! backend that silently went lazy would return the kept arm and break the
//! total-arms-guards idiom the engine relies on. The corpus pins the trap
//! in both arm positions plus a non-trapping control on every executable
//! backend.

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

#[test]
fn select_discarded_arm_trap_is_observed_per_backend() {
    let source = example("source/pressure-select-strict.mncs");
    let corpus = example("execution/pressure-select-strict-corpus.json");
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
        assert_eq!(cases.len(), 3, "{backend}: case count");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
            let want_status = match id {
                "control" => "returned",
                _ => "runtime_failure",
            };
            assert_eq!(
                case["status"], want_status,
                "{backend} {id}: status {case:#}"
            );
            if id == "control" {
                assert_eq!(
                    case["expectation_met"], true,
                    "{backend} {id}: logical value mismatch; returned={:#}",
                    case["returned"]
                );
            }
        }
    }
}
